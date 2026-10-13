//! `/hello` — contoh kecil yang membuktikan hidrasi Wasm berjalan.

use leptos::prelude::*;
use selarust::MobileLayout;

/// Komponen halaman.
#[component]
pub fn Page() -> impl IntoView {
    let count = RwSignal::new(0);

    view! {
        <MobileLayout title="Hello" show_back=true>
            <section class="sr-page">
                <h2 class="sr-title">"Hidrasi berjalan"</h2>
                <p class="sr-muted">
                    "HTML halaman ini dirender di server oleh Axum. Setelah bundle Wasm dimuat, \
                     tombol di bawah menjadi interaktif tanpa memuat ulang halaman."
                </p>
                <button class="sr-btn" on:click=move |_| *count.write() += 1>
                    "Klik: " {count}
                </button>
            </section>
        </MobileLayout>
    }
}
