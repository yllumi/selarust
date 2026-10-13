//! The "Auto-PWA Generator": manifest, service worker and icons.
//!
//! Generated from `Selarust.toml` on every build, so the PWA surface can never
//! drift away from the app configuration.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::{fingerprint, png, write_file, write_file_bytes, Config, Error, Result};

/// Public prefix the vendored Workbox runtime is served from.
const WORKBOX_PREFIX: &str = "/workbox/";

/// Vendored Workbox 7.4.1 runtime, copied next to the generated assets.
///
/// See `vendor/workbox/NOTICE.md` for provenance and the update procedure.
const WORKBOX_FILES: &[(&str, &str)] = &[
    ("workbox-sw.js", include_str!("../vendor/workbox/workbox-sw.js")),
    (
        "workbox-core.prod.js",
        include_str!("../vendor/workbox/workbox-core.prod.js"),
    ),
    (
        "workbox-routing.prod.js",
        include_str!("../vendor/workbox/workbox-routing.prod.js"),
    ),
    (
        "workbox-strategies.prod.js",
        include_str!("../vendor/workbox/workbox-strategies.prod.js"),
    ),
    (
        "workbox-precaching.prod.js",
        include_str!("../vendor/workbox/workbox-precaching.prod.js"),
    ),
    (
        "workbox-expiration.prod.js",
        include_str!("../vendor/workbox/workbox-expiration.prod.js"),
    ),
    (
        "workbox-cacheable-response.prod.js",
        include_str!("../vendor/workbox/workbox-cacheable-response.prod.js"),
    ),
];

/// Copies the vendored Workbox runtime into `<assets_dir>/workbox/`.
///
/// Vendored rather than loaded from a CDN so the worker can install while the
/// device is offline and never depends on a third-party origin at runtime.
pub(crate) fn write_workbox(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let directory = assets_dir.join("workbox");
    if !config.pwa.service_worker {
        if directory.exists() {
            std::fs::remove_dir_all(&directory)?;
        }
        return Ok(Vec::new());
    }

    std::fs::create_dir_all(&directory)?;
    let mut written = Vec::with_capacity(WORKBOX_FILES.len());
    for (name, contents) in WORKBOX_FILES {
        let path = directory.join(name);
        write_file(&path, contents)?;
        written.push(path);
    }
    Ok(written)
}

/// Writes `manifest.webmanifest`.
pub(crate) fn write_manifest(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let app = &config.app;
    let mut manifest = Map::new();

    manifest.insert("name".to_string(), Value::from(app.name.clone()));
    manifest.insert("short_name".to_string(), Value::from(app.short_name.clone()));
    manifest.insert(
        "description".to_string(),
        Value::from(app.description.clone()),
    );
    manifest.insert("lang".to_string(), Value::from(app.lang.clone()));
    manifest.insert("start_url".to_string(), Value::from(app.start_url.clone()));
    manifest.insert("scope".to_string(), Value::from(app.scope.clone()));
    manifest.insert("display".to_string(), Value::from(app.display.clone()));
    manifest.insert(
        "theme_color".to_string(),
        Value::from(app.theme_color.clone()),
    );
    manifest.insert(
        "background_color".to_string(),
        Value::from(app.background_color.clone()),
    );
    if let Some(orientation) = &app.orientation {
        manifest.insert("orientation".to_string(), Value::from(orientation.clone()));
    }

    let mut icons = Vec::with_capacity(config.pwa.icons.len() * 2);
    for size in &config.pwa.icons {
        validate_icon_size(*size)?;
        for (suffix, purpose) in [("", "any"), ("-maskable", "maskable")] {
            let mut icon = Map::new();
            icon.insert(
                "src".to_string(),
                Value::from(format!("/icons/icon-{size}{suffix}.png")),
            );
            icon.insert("sizes".to_string(), Value::from(format!("{size}x{size}")));
            icon.insert("type".to_string(), Value::from("image/png"));
            icon.insert("purpose".to_string(), Value::from(purpose));
            icons.push(Value::Object(icon));
        }
    }
    manifest.insert("icons".to_string(), Value::Array(icons));

    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(manifest))?
    );
    Ok(vec![write_file(
        &assets_dir.join("manifest.webmanifest"),
        &text,
    )?])
}

/// Writes `sw.js`, or removes a previously generated one when disabled.
///
/// Needs `manifest_dir` because the cache version is derived from the app
/// sources — see [`source_fingerprint`].
pub(crate) fn write_service_worker(
    manifest_dir: &Path,
    assets_dir: &Path,
    config: &Config,
) -> Result<Vec<PathBuf>> {
    let path = assets_dir.join("sw.js");

    if !config.pwa.service_worker {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        return Ok(Vec::new());
    }

    let pwa = &config.pwa;
    let version = cache_version(manifest_dir, config);
    let precache = serde_json::to_string(&precache_entries(&pwa.precache, &version))?;
    let shell_assets = serde_json::to_string(&shell_assets(manifest_dir))?;
    let offline = match &pwa.offline_fallback {
        Some(url) => serde_json::to_string(url)?,
        None => "null".to_string(),
    };

    let script = include_str!("templates/sw.js")
        .replace("__VERSION__", &version)
        .replace(
            "__WORKBOX_LOADER__",
            &format!("{WORKBOX_PREFIX}workbox-sw.js"),
        )
        .replace("__WORKBOX_PREFIX__", WORKBOX_PREFIX)
        .replace("__PRECACHE__", &precache)
        .replace("__SHELL_ASSETS__", &shell_assets)
        .replace("__OFFLINE_URL__", &offline)
        .replace("__NETWORK_FIRST__", &regex_list(&pwa.network_first))
        .replace(
            "__STALE_WHILE_REVALIDATE__",
            &regex_list(&pwa.stale_while_revalidate),
        )
        .replace("__MAX_ENTRIES__", &pwa.runtime_max_entries.to_string())
        .replace(
            "__MAX_AGE_SECONDS__",
            &pwa.runtime_max_age_seconds.to_string(),
        );

    write_file_bytes(&path, script.as_bytes())?;
    Ok(vec![path])
}

/// URLs of the application shell bundle (`/pkg/…`), read from the Leptos metadata.
///
/// The service worker warms these into the runtime cache on install: the very
/// first page load happens *before* the worker controls the page, so those
/// requests never pass through it. Without this step an offline visit can serve
/// the cached HTML but not the Wasm/CSS it needs, and the app never hydrates.
/// Returns an empty list when the metadata is absent (e.g. a non-Leptos crate or
/// a test fixture), in which case the worker only precaches `[pwa] precache`.
fn shell_assets(manifest_dir: &Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(manifest_dir.join("Cargo.toml")) else {
        return Vec::new();
    };
    let Ok(manifest) = raw.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(leptos) = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("leptos"))
    else {
        return Vec::new();
    };
    let Some(output_name) = leptos.get("output-name").and_then(|value| value.as_str()) else {
        return Vec::new();
    };
    let pkg_dir = leptos
        .get("site-pkg-dir")
        .and_then(|value| value.as_str())
        .unwrap_or("pkg")
        .trim_matches('/');

    ["js", "wasm", "css"]
        .iter()
        .map(|extension| format!("/{pkg_dir}/{output_name}.{extension}"))
        .collect()
}

/// Precached URLs paired with a revision, the shape Workbox expects.
///
/// Unhashed URLs need an explicit revision, otherwise Workbox keeps the first
/// copy it ever saw.
fn precache_entries(urls: &[String], revision: &str) -> Vec<Value> {
    urls.iter()
        .map(|url| json!({ "url": url, "revision": revision }))
        .collect()
}

/// Cache version shared by every cache the generated worker owns.
fn cache_version(manifest_dir: &Path, config: &Config) -> String {
    let pwa = &config.pwa;
    let sources = source_fingerprint(manifest_dir, config);
    format!(
        "selarust-{}",
        fingerprint(&[
            config.app.name.as_str(),
            config.app.short_name.as_str(),
            &pwa.precache.join(","),
            &pwa.network_first.join(","),
            &pwa.stale_while_revalidate.join(","),
            pwa.offline_fallback.as_deref().unwrap_or(""),
            &sources,
        ])
    )
}

/// Fingerprint of the app sources that end up inside the build.
///
/// Cargo does not hand a build script a build id, so the generator hashes the app
/// tree itself. Without this a code-only deploy would keep the same cache version
/// (the config did not change), and a returning visitor could be served the
/// previous build's HTML next to the new Wasm.
fn source_fingerprint(manifest_dir: &Path, config: &Config) -> String {
    // Build output and generated assets are excluded; hashing the generated
    // worker would make the fingerprint depend on its own output.
    let skip = [
        manifest_dir.join("target"),
        config.assets_dir(manifest_dir),
        manifest_dir.join(".git"),
        manifest_dir.join("node_modules"),
    ];

    let mut files = Vec::new();
    collect_sources(manifest_dir, manifest_dir, &skip, &mut files);
    files.sort();

    let parts: Vec<String> = files
        .into_iter()
        .flat_map(|(path, contents)| [path, contents])
        .collect();
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    fingerprint(&refs)
}

fn collect_sources(
    root: &Path,
    directory: &Path,
    skip: &[PathBuf],
    out: &mut Vec<(String, String)>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();

    for path in paths {
        if skip.iter().any(|blocked| path == *blocked) {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_sources(root, &path, skip, out);
            continue;
        }
        // Binary files (icons, databases) are not part of the fingerprint.
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        out.push((relative, contents));
    }
}

/// Writes `icons/icon-<size>.png` and `icons/icon-<size>-maskable.png`.
pub(crate) fn write_icons(assets_dir: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    let theme = png::parse_hex_color(&config.app.theme_color)?;
    let directory = assets_dir.join("icons");
    std::fs::create_dir_all(&directory)?;

    let mut written = Vec::with_capacity(config.pwa.icons.len() * 2);
    for size in &config.pwa.icons {
        validate_icon_size(*size)?;
        for (suffix, maskable) in [("", false), ("-maskable", true)] {
            let path = directory.join(format!("icon-{size}{suffix}.png"));
            write_file_bytes(&path, &png::icon_png(*size, theme, maskable))?;
            written.push(path);
        }
    }
    Ok(written)
}

fn validate_icon_size(size: u32) -> Result<()> {
    if (16..=2048).contains(&size) {
        Ok(())
    } else {
        Err(Error::new(format!(
            "[pwa] icons contains {size}: sizes must be between 16 and 2048"
        )))
    }
}

fn regex_list(prefixes: &[String]) -> String {
    let items = prefixes
        .iter()
        .map(|prefix| format!("/{}/", escape_regex(prefix)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn escape_regex(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '/' | '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}
