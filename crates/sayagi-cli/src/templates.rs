//! Proyek yang ditulis oleh `sayagi new`.
//!
//! Template disimpan sebagai berkas nyata di `crates/sayagi-cli/templates/app`
//! dan disematkan ke binary lewat `include_str!`, sehingga CLI tetap mandiri
//! tanpa perlu mengunduh apa pun.

use std::path::{Path, PathBuf};

/// Satu berkas template. `path` relatif terhadap root template.
struct TemplateFile {
    path: &'static str,
    contents: &'static str,
}

const FILES: &[TemplateFile] = &[
    TemplateFile { path: "Cargo.toml.tmpl", contents: include_str!("../templates/app/Cargo.toml.tmpl") },
    TemplateFile { path: "Sayagi.toml.tmpl", contents: include_str!("../templates/app/Sayagi.toml.tmpl") },
    TemplateFile { path: "build.rs", contents: include_str!("../templates/app/build.rs") },
    TemplateFile { path: "README.md.tmpl", contents: include_str!("../templates/app/README.md.tmpl") },
    TemplateFile { path: ".gitignore", contents: include_str!("../templates/app/.gitignore") },
    TemplateFile { path: "public/robots.txt", contents: include_str!("../templates/app/public/robots.txt") },
    TemplateFile { path: "style/main.css", contents: include_str!("../templates/app/style/main.css") },
    TemplateFile { path: "src/lib.rs", contents: include_str!("../templates/app/src/lib.rs") },
    TemplateFile { path: "src/main.rs.tmpl", contents: include_str!("../templates/app/src/main.rs.tmpl") },
    TemplateFile { path: "src/app.rs", contents: include_str!("../templates/app/src/app.rs") },
    TemplateFile { path: "src/service_worker.rs", contents: include_str!("../templates/app/src/service_worker.rs") },
    TemplateFile { path: "src/components/mod.rs", contents: include_str!("../templates/app/src/components/mod.rs") },
    TemplateFile { path: "src/pages/index.rs.tmpl", contents: include_str!("../templates/app/src/pages/index.rs.tmpl") },
    TemplateFile { path: "src/pages/hello.rs", contents: include_str!("../templates/app/src/pages/hello.rs") },
    TemplateFile { path: "src/pages/offline.rs", contents: include_str!("../templates/app/src/pages/offline.rs") },
];

/// Root workspace Sayagi, dipakai untuk menulis dependency `path = …` di scaffold.
pub fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest_dir.to_path_buf())
}

/// Menulis seluruh template ke `target` dan mengembalikan berkas yang dibuat.
///
/// Nama berkas berakhiran `.tmpl` ditulis tanpa sufiks itu.
pub fn scaffold(target: &Path, vars: &[(&str, &str)]) -> Result<Vec<PathBuf>, String> {
    let mut written = Vec::with_capacity(FILES.len());

    for file in FILES {
        let relative = file.path.strip_suffix(".tmpl").unwrap_or(file.path);
        let destination = target.join(relative);

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("gagal membuat folder {}: {error}", parent.display())
            })?;
        }

        let contents = render(file.contents, vars);
        std::fs::write(&destination, contents).map_err(|error| {
            format!("gagal menulis {}: {error}", destination.display())
        })?;
        written.push(destination);
    }

    Ok(written)
}

/// Mengganti placeholder `{{nama}}` dengan nilai yang diberikan.
pub fn render(contents: &str, vars: &[(&str, &str)]) -> String {
    let mut rendered = contents.to_string();
    for (key, value) in vars {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_every_placeholder() {
        let rendered = render("name = \"{{project_name}}\"", &[("project_name", "demo")]);
        assert_eq!(rendered, "name = \"demo\"");
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn templates_only_use_documented_placeholders() {
        let documented = [
            "project_name",
            "crate_name",
            "project_title",
            "sayagi_root",
            "site_addr",
        ];

        for file in FILES {
            let mut rest = file.contents;
            while let Some(start) = rest.find("{{") {
                let after = &rest[start + 2..];
                let end = after
                    .find("}}")
                    .unwrap_or_else(|| panic!("placeholder tidak ditutup di {}", file.path));
                let name = &after[..end];
                assert!(
                    documented.contains(&name),
                    "placeholder `{{{{{name}}}}}` di {} tidak terdaftar di dokumentasi",
                    file.path
                );
                rest = &after[end + 2..];
            }
        }
    }
}
