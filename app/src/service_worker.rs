//! Wiring PWA di sisi klien.
//!
//! Skrip worker-nya sendiri (`/sw.js`) digenerate saat build oleh `sayagi-build`.
//! Service worker wajib berupa berkas JS klasik, jadi sisi Rust hanya memasang dan
//! menjaga kesegarannya, sementara *strategi* caching-nya tetap digenerate dari
//! `Sayagi.toml` (lihat `crates/sayagi-build/src/templates/sw.js`).

use crate::config;

/// Memasang service worker (jika diaktifkan) dan mulai melacak status konektivitas.
///
/// Pada build debug service worker sengaja tidak dipasang: `cargo leptos watch`
/// menyajikan `/pkg/*` tanpa hash nama berkas, sehingga cache
/// *stale-while-revalidate* bisa menyajikan campuran JS/Wasm dari build lama dan
/// merusak hot reload. Di build release nama berkas sudah di-hash dan worker
/// justru yang membuat aplikasi bisa dipakai offline.
pub fn install() {
    if config::SERVICE_WORKER_ENABLED && !cfg!(debug_assertions) {
        sayagi::register_service_worker(config::SERVICE_WORKER_PATH);
    }
    sayagi::watch_connectivity();
}
