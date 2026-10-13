//! Publishing generated assets into the Leptos site root.
//!
//! `cargo leptos` copies the package `assets-dir` into the site root *before* it
//! runs `cargo build`, so anything `build.rs` generates during that same build —
//! manifest, service worker, icons, Workbox runtime — would only reach the site
//! root on the *next* build. On a fresh checkout that means the first build
//! serves no manifest and no service worker at all, which silently breaks
//! installability until the app is built twice.
//!
//! `selarust-build` therefore publishes its own output to the site root as part of
//! [`generate`](crate::generate). The directory is taken from `LEPTOS_SITE_ROOT`,
//! which cargo-leptos exports to build scripts; without it (unit tests, `selarust
//! pwa` outside a Leptos build) publishing is simply skipped.

use std::path::{Path, PathBuf};

use crate::{write_file_bytes, Result};

/// Site root for a package, if a Leptos build is exporting one.
pub(crate) fn site_root(manifest_dir: &Path) -> Option<PathBuf> {
    let value = std::env::var("LEPTOS_SITE_ROOT").ok()?;
    resolve_root(&value, manifest_dir)
}

/// Turns the raw `LEPTOS_SITE_ROOT` value into an absolute path.
///
/// Split from [`site_root`] so the resolution rules can be tested without
/// touching the process environment (which is global to the test binary).
pub(crate) fn resolve_root(value: &str, manifest_dir: &Path) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    // cargo-leptos resolves the metadata `site-root` against the *workspace*
    // root, while build scripts run with their package directory as cwd.
    Some(workspace_root(manifest_dir).join(path))
}

/// Nearest ancestor of `manifest_dir` (including itself) whose `Cargo.toml`
/// declares a `[workspace]` table.
pub(crate) fn workspace_root(manifest_dir: &Path) -> PathBuf {
    let mut current = manifest_dir.to_path_buf();
    loop {
        let manifest = current.join("Cargo.toml");
        if let Ok(contents) = std::fs::read_to_string(&manifest) {
            if contents.lines().any(|line| line.trim() == "[workspace]") {
                return current;
            }
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return manifest_dir.to_path_buf(),
        }
    }
}

/// Copies every file of `assets_dir` into `site_root`, recursively.
///
/// Returns the destination paths. Identical files are left untouched (see
/// [`write_file_bytes`]), so repeated builds do not churn the site root.
pub(crate) fn publish(assets_dir: &Path, site_root: &Path) -> Result<Vec<PathBuf>> {
    let mut published = Vec::new();
    if !assets_dir.is_dir() || assets_dir == site_root {
        return Ok(published);
    }
    copy_tree(assets_dir, assets_dir, site_root, &mut published)?;
    Ok(published)
}

fn copy_tree(
    root: &Path,
    directory: &Path,
    site_root: &Path,
    published: &mut Vec<PathBuf>,
) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Never copy a directory into itself (a site root inside the assets
            // directory would otherwise recurse forever).
            if site_root.starts_with(&path) {
                continue;
            }
            copy_tree(root, &path, site_root, published)?;
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let contents = std::fs::read(&path)?;
        let destination = site_root.join(relative);
        write_file_bytes(&destination, &contents)?;
        published.push(destination);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{publish, resolve_root, workspace_root};
    use std::fs;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "selarust-site-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn absolute_site_roots_are_used_as_is() {
        let manifest = std::path::Path::new("/srv/app");
        assert_eq!(
            resolve_root("/srv/site", manifest),
            Some(std::path::PathBuf::from("/srv/site"))
        );
        assert_eq!(resolve_root("   ", manifest), None);
    }

    #[test]
    fn relative_site_roots_resolve_against_the_workspace_root() {
        let root = temp_dir("workspace");
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        let package = root.join("app");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();

        assert_eq!(workspace_root(&package), root);
        assert_eq!(
            resolve_root("target/site", &package),
            Some(root.join("target/site")),
            "site-root relatif harus mengikuti root workspace, bukan direktori paket"
        );
    }

    #[test]
    fn publishing_copies_nested_assets_and_skips_identical_files() {
        let assets = temp_dir("assets");
        let site = temp_dir("site");
        fs::write(assets.join("manifest.webmanifest"), "{}").unwrap();
        fs::create_dir_all(assets.join("icons")).unwrap();
        fs::write(assets.join("icons/icon-192.png"), b"\x89PNG").unwrap();

        let published = publish(&assets, &site).unwrap();
        assert_eq!(published.len(), 2, "dua berkas harus tersalin: {published:?}");
        assert_eq!(fs::read_to_string(site.join("manifest.webmanifest")).unwrap(), "{}");
        assert_eq!(fs::read(site.join("icons/icon-192.png")).unwrap(), b"\x89PNG");

        // Salinan kedua tidak menulis ulang berkas yang isinya sama.
        let before = fs::metadata(site.join("manifest.webmanifest")).unwrap().modified().unwrap();
        let again = publish(&assets, &site).unwrap();
        assert_eq!(again.len(), 2);
        let after = fs::metadata(site.join("manifest.webmanifest")).unwrap().modified().unwrap();
        assert_eq!(before, after, "berkas identik tidak boleh ditulis ulang");
    }

    #[test]
    fn publishing_stops_when_the_site_root_is_the_assets_dir() {
        let assets = temp_dir("self");
        fs::write(assets.join("sw.js"), "// sw").unwrap();
        assert!(publish(&assets, &assets).unwrap().is_empty());
    }
}
