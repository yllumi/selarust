//! Wiring PWA di sisi klien.
//!
//! Skrip worker-nya sendiri (`/sw.js`) digenerate saat build oleh `sayagi-build`.
//! Service worker wajib berupa berkas JS klasik, jadi sisi Rust hanya memasang dan
//! menjaga kesegarannya, sementara *strategi* caching-nya tetap digenerate dari
//! `Sayagi.toml` (lihat `crates/sayagi-build/src/templates/sw.js`).

use crate::config;

/// Memasang service worker (jika diaktifkan) dan mulai melacak status konektivitas.
///
/// Pada build debug service worker tidak dipasang: aset dev disajikan ulang setiap
/// rebuild, sementara aturan navigasi worker meng-cache halaman sehingga HTML basi
/// bisa merusak hot reload. Sebagai gantinya worker dan cache yang mungkin
/// tertinggal dari build release sebelumnya dibersihkan. Di build release worker
/// justru yang membuat aplikasi bisa dipakai offline.
pub fn install() {
    if cfg!(debug_assertions) {
        sayagi::reset_service_worker();
    } else if config::SERVICE_WORKER_ENABLED {
        sayagi::register_service_worker(config::SERVICE_WORKER_PATH);
    }
    sayagi::watch_connectivity();
}
