//! Crate aplikasi `mobile-pwa`.
//!
//! Dikompilasi dua kali dari satu sumber:
//!
//! * `--features ssr` — dijalankan sebagai binary Axum (SSR + server functions).
//! * `--features hydrate` — dikompilasi ke `wasm32-unknown-unknown` dan
//!   menempelkan kembali interaktivitas ke HTML hasil SSR.

pub mod api;
pub mod app;
pub mod components;
/// Klien realtime (WebSocket) yang menerapkan event ke [`store::TodoStore`].
pub mod realtime;
pub mod service_worker;
/// State todo sisi klien: snapshot lokal, antrean offline, overlay optimistis.
pub mod store;

#[cfg(feature = "ssr")]
pub mod db;

/// Hub + endpoint WebSocket untuk sinkronisasi realtime (khusus server).
#[cfg(feature = "ssr")]
pub mod sync;

/// Rute yang digenerate dari `src/pages` oleh `sayagi-build`.
///
/// Setiap file halaman dideklarasikan di sini lewat `#[path]` absolut, sehingga
/// nama file seperti `[id].rs` bisa tetap dipakai sebagai modul Rust.
/// Modul ini juga mengekspor `ROUTES` dan komponen `FileRoutes`/`NotFound`.
pub mod pages {
    include!(concat!(env!("OUT_DIR"), "/routes.rs"));
}

/// Konstanta yang dipanggang dari `Sayagi.toml` saat build (`APP_NAME`, `THEME_COLOR`, …).
pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

/// Titik masuk sisi klien: hidrasi HTML hasil SSR lalu pasang lapisan PWA.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    service_worker::install();
    leptos::mount::hydrate_body(app::App);
}
