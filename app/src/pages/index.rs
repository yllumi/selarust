//! `/` — halaman perkenalan.

use leptos::prelude::*;
use selarust::{LinkButton, MobileLayout};

/// Komponen halaman beranda.
#[component]
pub fn Page() -> impl IntoView {
    view! {
        <MobileLayout title="Selarust">
            <section class="sr-hero">
                <p class="sr-hero__eyebrow">"Rust · Axum · Leptos · SQLite"</p>
                <h2 class="sr-hero__title">"Metaframework PWA mobile-first"</h2>
                <p class="sr-hero__lead">
                    "Backend Axum, frontend Leptos yang di-hydrate dari WebAssembly, dan SQLite \
                     tertanam — semuanya dari satu bahasa, satu toolchain."
                </p>
                <LinkButton href="/todos">"Buka todo list"</LinkButton>
            </section>

            <section class="sr-section">
                <h3 class="sr-section__title">"Sudah aktif di contoh ini"</h3>
                <ul class="sr-list">
                    <li class="sr-list__item">
                        <span class="sr-dot"></span>
                        "CRUD todo yang tersimpan di SQLite, lewat server functions"
                    </li>
                    <li class="sr-list__item">
                        <span class="sr-dot"></span>
                        "File-based routing dari " <code class="sr-code">"src/pages"</code>
                    </li>
                    <li class="sr-list__item">
                        <span class="sr-dot"></span>
                        "Manifest, service worker Workbox, dan ikon digenerate saat build"
                    </li>
                    <li class="sr-list__item">
                        <span class="sr-dot"></span>
                        "SQLite mode WAL dengan migrasi otomatis saat boot"
                    </li>
                </ul>
            </section>

            <section class="sr-section">
                <h3 class="sr-section__title">"Coba juga"</h3>
                <LinkButton href="/dashboard" variant="ghost">
                    "Lihat statistik dan tabel rute"
                </LinkButton>
            </section>
        </MobileLayout>
    }
}
