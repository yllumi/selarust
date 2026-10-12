//! `/` — halaman perkenalan.

use leptos::prelude::*;
use sayagi::{LinkButton, MobileLayout};

/// Komponen halaman beranda.
#[component]
pub fn Page() -> impl IntoView {
    view! {
        <MobileLayout title="Sayagi">
            <section class="sy-hero">
                <p class="sy-hero__eyebrow">"Rust · Axum · Leptos · SQLite"</p>
                <h2 class="sy-hero__title">"Metaframework PWA mobile-first"</h2>
                <p class="sy-hero__lead">
                    "Backend Axum, frontend Leptos yang di-hydrate dari WebAssembly, dan SQLite \
                     tertanam — semuanya dari satu bahasa, satu toolchain."
                </p>
                <LinkButton href="/containers">
                    "Buka daftar container"
                </LinkButton>
            </section>

            <section class="sy-section">
                <h3 class="sy-section__title">"Sudah aktif di contoh ini"</h3>
                <ul class="sy-list">
                    <li class="sy-list__item">
                        <span class="sy-dot"></span>
                        "File-based routing dari " <code class="sy-code">"src/pages"</code>
                    </li>
                    <li class="sy-list__item">
                        <span class="sy-dot"></span>
                        "Server functions tanpa menulis endpoint REST manual"
                    </li>
                    <li class="sy-list__item">
                        <span class="sy-dot"></span>
                        "Manifest, service worker, dan ikon digenerate saat build"
                    </li>
                    <li class="sy-list__item">
                        <span class="sy-dot"></span>
                        "SQLite mode WAL dengan migrasi otomatis saat boot"
                    </li>
                </ul>
            </section>

            <section class="sy-section">
                <h3 class="sy-section__title">"Coba juga"</h3>
                <LinkButton href="/dashboard" variant="ghost">
                    "Lihat tabel rute hasil generate"
                </LinkButton>
            </section>
        </MobileLayout>
    }
}
