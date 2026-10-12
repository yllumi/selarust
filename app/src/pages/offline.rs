//! `/offline` — halaman cadangan yang di-precache service worker.
//!
//! Rute ini disebut di `Sayagi.toml` sebagai `offline_fallback`, sehingga
//! navigasi yang gagal saat perangkat tanpa jaringan tetap menampilkan layar
//! yang ramah, bukan halaman galat bawaan browser.

use leptos::prelude::*;
use sayagi::{LinkButton, MobileLayout};

/// Komponen halaman mode offline.
#[component]
pub fn Page() -> impl IntoView {
    view! {
        <MobileLayout title="Offline">
            <section class="sy-page sy-empty">
                <p class="sy-empty__code">"offline"</p>
                <h2 class="sy-empty__title">"Koneksi terputus"</h2>
                <p class="sy-muted">
                    "Halaman ini disajikan dari cache service worker. Sambungkan kembali \
                     perangkat untuk memuat data terbaru."
                </p>
                <LinkButton href="/">
                    "Coba beranda"
                </LinkButton>
            </section>
        </MobileLayout>
    }
}
