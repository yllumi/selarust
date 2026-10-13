//! End-to-end checks for the generator: does `generate` write what an app needs?

use std::path::{Path, PathBuf};

/// Creates a throwaway app. `pwa` is appended to the `[pwa]` section.
fn scaffold(label: &str, pwa: &str, pages: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("selarust-build-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/pages")).expect("create pages dir");

    let config = format!(
        r#"
[app]
name = "Generator Test"
short_name = "GenTest"

[pwa]
precache = ["/", "/offline", "/manifest.webmanifest"]
network_first = ["/api/", "/pkg/"]
stale_while_revalidate = ["/icons/", "/assets/"]
{pwa}

[database]
url = "sqlite::memory:"
"#
    );
    std::fs::write(root.join("Selarust.toml"), config).expect("write Selarust.toml");

    // Metadata Leptos dipakai worker untuk menemukan berkas bundle shell.
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"gen-test\"\nversion = \"0.1.0\"\n\n[package.metadata.leptos]\noutput-name = \"gen-test\"\nsite-pkg-dir = \"pkg\"\n",
    )
    .expect("write Cargo.toml");

    for (path, source) in pages {
        let file = root.join("src/pages").join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).expect("create page dir");
        }
        std::fs::write(&file, source).expect("write page");
    }
    root
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn sw(root: &Path) -> String {
    read(&root.join("public/sw.js"))
}

fn sw_version(sw: &str) -> String {
    sw.lines()
        .find_map(|line| line.strip_prefix("const VERSION = \""))
        .and_then(|rest| rest.strip_suffix("\";"))
        .map(str::to_string)
        .expect("sw.js must declare a VERSION constant")
}

fn generate(root: &Path) -> Vec<PathBuf> {
    let out = root.join("out");
    std::fs::create_dir_all(&out).expect("create out dir");
    let report = selarust_build::generate(root, &out).expect("generation succeeds");
    report.files
}

#[test]
fn writes_routes_manifest_icons_and_a_workbox_service_worker() {
    let root = scaffold(
        "full",
        "",
        &[
            ("index.rs", "// home"),
            ("todos/index.rs", "// list"),
            ("todos/[id].rs", "// detail"),
            ("offline.rs", "// offline"),
        ],
    );
    generate(&root);

    let routes = read(&root.join("out/routes.rs"));
    assert!(routes.contains("todos/[id].rs"), "dynamic page module:\n{routes}");
    assert!(routes.contains("\"/todos/:id\""), "dynamic route path:\n{routes}");

    let sw = sw(&root);
    assert!(
        sw.contains(r#"importScripts("/workbox/workbox-sw.js")"#),
        "the worker loads the vendored Workbox runtime"
    );
    assert!(
        sw.contains(r#"setConfig({ modulePathPrefix: "/workbox/" })"#),
        "the module prefix must match where the runtime is copied"
    );
    for expected in [
        "workbox.precaching.precacheAndRoute",
        "workbox.routing",
        "workbox.strategies",
        "NetworkFirst",
        "StaleWhileRevalidate",
        "ExpirationPlugin",
        "setCatchHandler",
    ] {
        assert!(sw.contains(expected), "`{expected}` missing from sw.js");
    }
    for url in ["/", "/offline", "/manifest.webmanifest"] {
        assert!(
            sw.contains(&format!(r#""url":"{url}""#)),
            "precache entry for {url} is missing"
        );
    }
    assert!(
        sw.contains(r#""revision":""#),
        "unhashed precache URLs need an explicit revision"
    );
    assert!(
        sw.contains(r#"const SHELL_ASSETS = ["/pkg/gen-test.js","/pkg/gen-test.wasm","/pkg/gen-test.css"]"#),
        "the worker must know which bundle files to warm:\n{sw}"
    );
    assert!(
        sw.contains("warmShell"),
        "the shell bundle is warmed on install, otherwise offline loads have HTML but no Wasm"
    );
    assert!(
        !sw.contains("__"),
        "every template placeholder must be substituted:\n{sw}"
    );

    for file in [
        "workbox/workbox-sw.js",
        "workbox/workbox-core.prod.js",
        "workbox/workbox-routing.prod.js",
        "workbox/workbox-strategies.prod.js",
        "workbox/workbox-precaching.prod.js",
        "workbox/workbox-expiration.prod.js",
        "workbox/workbox-cacheable-response.prod.js",
    ] {
        let path = root.join("public").join(file);
        let size = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("missing vendored runtime file {file}: {error}"))
            .len();
        assert!(size > 100, "{file} looks empty ({size} bytes)");
    }

    let manifest = read(&root.join("public/manifest.webmanifest"));
    assert!(manifest.contains("Generator Test"));
    assert!(root.join("public/icons/icon-192.png").is_file());
    assert!(root.join("out/config.rs").is_file());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cache_version_changes_when_app_sources_change() {
    let root = scaffold("version", "", &[("index.rs", "// first revision")]);
    generate(&root);
    let before = sw_version(&sw(&root));

    std::fs::write(root.join("src/pages/index.rs"), "// second revision").expect("edit page");
    generate(&root);
    let after = sw_version(&sw(&root));

    assert_ne!(
        before, after,
        "a code-only change must invalidate the caches, otherwise a returning visitor \
         can get the previous build's HTML paired with the new Wasm"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn disabled_service_worker_removes_worker_and_runtime() {
    let root = scaffold("disabled", "", &[("index.rs", "// home")]);
    generate(&root);
    assert!(root.join("public/sw.js").is_file(), "worker written while enabled");
    assert!(root.join("public/workbox").is_dir(), "runtime copied while enabled");

    let config = read(&root.join("Selarust.toml")).replace("[pwa]\n", "[pwa]\nservice_worker = false\n");
    std::fs::write(root.join("Selarust.toml"), config).expect("disable worker");
    generate(&root);

    assert!(
        !root.join("public/sw.js").exists(),
        "disabling the worker must delete the stale sw.js"
    );
    assert!(
        !root.join("public/workbox").exists(),
        "the vendored runtime is only kept for an enabled worker"
    );
    assert!(
        root.join("public/manifest.webmanifest").is_file(),
        "the manifest is independent of the worker"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn shell_assets_are_empty_without_leptos_metadata() {
    let root = scaffold("no-meta", "", &[("index.rs", "// home")]);
    std::fs::remove_file(root.join("Cargo.toml")).expect("drop metadata");
    generate(&root);

    let sw = sw(&root);
    assert!(
        sw.contains("const SHELL_ASSETS = []"),
        "without metadata the worker has nothing to warm and must not guess paths:\n{sw}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
