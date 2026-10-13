//! Wiring PWA di sisi klien: memasang service worker dan memantau konektivitas.

use crate::config;

/// Dipanggil sekali dari `hydrate()`.
///
/// Service worker hanya dipasang pada build release: di mode debug aset disajikan
/// ulang setiap rebuild, jadi cache halaman bisa menyajikan HTML lama dan membuat
/// hot reload membingungkan. Di mode debug, worker/cache sisa build release
/// sebelumnya justru dibersihkan.
pub fn install() {
    if cfg!(debug_assertions) {
        selarust::reset_service_worker();
    } else if config::SERVICE_WORKER_ENABLED {
        selarust::register_service_worker(config::SERVICE_WORKER_PATH);
    }
    selarust::watch_connectivity();
}
