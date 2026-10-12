//! Crate aplikasi.
//!
//! Dikompilasi dua kali dari satu sumber:
//!
//! * `--features ssr` — binary Axum yang merender HTML di server.
//! * `--features hydrate` — Wasm yang menempelkan interaktivitas ke HTML itu.

pub mod app;
pub mod components;
pub mod service_worker;

/// Rute yang digenerate dari `src/pages` oleh `sayagi-build`.
///
/// Setiap file halaman dideklarasikan lewat `#[path]` absolut, sehingga nama
/// berkas seperti `[id].rs` tetap bisa dipakai sebagai modul Rust.
pub mod pages {
    include!(concat!(env!("OUT_DIR"), "/routes.rs"));
}

/// Konstanta yang dipanggang dari `Sayagi.toml` saat build.
pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

/// Titik masuk sisi klien.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    service_worker::install();
    leptos::mount::hydrate_body(app::App);
}
