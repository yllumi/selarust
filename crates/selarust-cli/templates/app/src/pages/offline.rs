//! `/offline` — halaman cadangan yang di-precache service worker.

use leptos::prelude::*;
use selarust::{LinkButton, MobileLayout};

use crate::components::EmptyState;

/// Komponen halaman.
#[component]
pub fn Page() -> impl IntoView {
    view! {
        <MobileLayout title="Offline">
            <EmptyState
                title="Koneksi terputus"
                hint="Halaman ini disajikan dari cache service worker. Sambungkan kembali perangkat untuk memuat data terbaru."
            />
            <LinkButton href="/">
                "Coba beranda"
            </LinkButton>
        </MobileLayout>
    }
}
