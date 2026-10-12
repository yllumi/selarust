//! `/hello` — contoh kecil yang membuktikan hidrasi Wasm berjalan.

use leptos::prelude::*;
use sayagi::MobileLayout;

/// Komponen halaman.
#[component]
pub fn Page() -> impl IntoView {
    let count = RwSignal::new(0);

    view! {
        <MobileLayout title="Hello" show_back=true>
            <section class="sy-page">
                <h2 class="sy-title">"Hidrasi berjalan"</h2>
                <p class="sy-muted">
                    "HTML halaman ini dirender di server oleh Axum. Setelah bundle Wasm dimuat, \
                     tombol di bawah menjadi interaktif tanpa memuat ulang halaman."
                </p>
                <button class="sy-btn" on:click=move |_| *count.write() += 1>
                    "Klik: " {count}
                </button>
            </section>
        </MobileLayout>
    }
}
