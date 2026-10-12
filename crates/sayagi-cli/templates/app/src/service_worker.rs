//! Wiring PWA di sisi klien: memasang service worker dan memantau konektivitas.

use crate::config;

/// Dipanggil sekali dari `hydrate()`.
///
/// Service worker hanya dipasang pada build release: di mode debug `cargo leptos
/// watch` menyajikan berkas `/pkg/*` tanpa hash, sehingga cache aplikasi bisa
/// menyajikan bundle lama dan membuat hot reload membingungkan.
pub fn install() {
    if config::SERVICE_WORKER_ENABLED && !cfg!(debug_assertions) {
        sayagi::register_service_worker(config::SERVICE_WORKER_PATH);
    }
    sayagi::watch_connectivity();
}
