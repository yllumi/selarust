//! Memastikan aset PWA yang digenerate benar-benar sampai ke site root Leptos.
//!
//! Ini kasus yang tidak terlihat di mesin pengembangan yang sudah pernah build:
//! `cargo leptos` menyalin `assets-dir` ke site root **sebelum** `cargo build`,
//! jadi pada checkout baru (`app/public` masih kosong) manifest dan service
//! worker tidak pernah ikut tersalin. Uji ini menirukan posisi build script —
//! `LEPTOS_SITE_ROOT` di environment, `assets_dir` masih kosong — dan memeriksa
//! isi site root setelah `generate`.
//!
//! Berkas uji ini sengaja hanya memuat satu test: `LEPTOS_SITE_ROOT` adalah state
//! global milik proses, dan tiap berkas uji dijalankan sebagai binary sendiri.

use std::path::{Path, PathBuf};

/// Membuat aplikasi minimal di dalam root workspace palsu.
fn scaffold(label: &str) -> (PathBuf, PathBuf) {
    let workspace = std::env::temp_dir().join(format!("selarust-publish-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workspace);
    let root = workspace.join("app");
    std::fs::create_dir_all(root.join("src/pages")).expect("create pages dir");

    std::fs::write(
        workspace.join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\"]\n",
    )
    .expect("write workspace manifest");

    std::fs::write(
        root.join("Selarust.toml"),
        format!(
            r#"
[app]
name = "Publish Test {label}"
short_name = "PubTest"

[routes]
dir = "src/pages"

[pwa]
precache = ["/", "/offline", "/manifest.webmanifest"]
"#
        ),
    )
    .expect("write Selarust.toml");

    std::fs::write(root.join("src/pages/index.rs"), "// page\n").expect("write page");
    (workspace, root)
}

fn assert_present(path: &Path) {
    assert!(path.is_file(), "{} harus ada di site root", path.display());
}

#[test]
fn assets_generated_during_the_build_reach_the_leptos_site_root() {
    let (workspace, app) = scaffold("root");
    let site_root = workspace.join("target/site");
    // Relatif, persis seperti metadata `site-root = "target/site"`: generator
    // harus menyelesaikannya terhadap root workspace, bukan direktori paket.
    std::env::set_var("LEPTOS_SITE_ROOT", "target/site");

    let out = app.join("out");
    std::fs::create_dir_all(&out).expect("create out dir");
    let assets = app.join("public");
    assert!(
        !assets.exists(),
        "prasyarat: assets dir harus masih kosong seperti checkout baru"
    );

    let report = selarust_build::generate(&app, &out).expect("generation succeeds");

    assert_eq!(report.site_root.as_deref(), Some(site_root.as_path()));
    assert!(
        report.published.len() >= 11,
        "semua aset harus dipublikasikan, hanya {} berkas",
        report.published.len()
    );
    assert_present(&site_root.join("manifest.webmanifest"));
    assert_present(&site_root.join("sw.js"));
    assert_present(&site_root.join("icons/icon-192.png"));
    assert_present(&site_root.join("workbox/workbox-sw.js"));

    // Isinya harus sama dengan sumbernya di assets dir.
    let generated = std::fs::read(app.join("public/sw.js")).expect("read generated sw.js");
    let published = std::fs::read(site_root.join("sw.js")).expect("read published sw.js");
    assert_eq!(generated, published, "sw.js di site root harus identik");

    // Sekaligus memastikan ringkasan menyebutkan tujuan publikasinya.
    assert!(
        report.summary().contains("published"),
        "ringkasan: {}",
        report.summary()
    );
}
