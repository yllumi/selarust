//! `/offline` — halaman cadangan yang di-precache service worker.
//!
//! Rute ini disebut di `Sayagi.toml` sebagai `offline_fallback`, sehingga
//! navigasi yang gagal saat perangkat tanpa jaringan tetap menampilkan layar
//! yang ramah. Karena store lokal sudah memuat snapshot IndexedDB, halaman ini
//! juga bisa memperlihatkan data terakhir yang diketahui perangkat.

use leptos::prelude::*;
use sayagi::MobileLayout;

use crate::store::TodoStore;

/// Komponen halaman mode offline.
#[component]
pub fn Page() -> impl IntoView {
    let store = expect_context::<TodoStore>();
    let pending = store.pending_count();
    // Sebelum hidrasi store masih kosong, sama seperti saat SSR: tidak ada
    // perbedaan markup yang perlu dihidrasi ulang.
    let items = Signal::derive(move || {
        if store.ready().get() {
            store.items().get()
        } else {
            Vec::new()
        }
    });

    let reload = move |_| {
        if let Some(window) = leptos::web_sys::window() {
            let _ = window.location().reload();
        }
    };

    view! {
        <MobileLayout title="Offline">
            <section class="sy-page sy-empty">
                <p class="sy-empty__code">"offline"</p>
                <h2 class="sy-empty__title">"Koneksi terputus"</h2>
                <p class="sy-muted">
                    "Halaman ini disajikan dari cache service worker. Sambungkan kembali \
                     perangkat untuk memuat data terbaru."
                </p>
                <button class="sy-btn-solid" type="button" on:click=reload>
                    "Coba lagi"
                </button>
            </section>

            <section class="sy-section">
                <h3 class="sy-section__title">"Salinan lokal"</h3>
                <p class="sy-muted sy-small">
                    {move || {
                        let waiting = pending.get();
                        if waiting == 0 {
                            "Semua perubahan sudah tersinkron.".to_string()
                        } else {
                            format!("{waiting} perubahan menunggu koneksi.")
                        }
                    }}
                </p>
                {move || {
                    let items = items.get();
                    if items.is_empty() {
                        view! {
                            <p class="sy-muted sy-small">
                                "Belum ada salinan data di perangkat ini."
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <ul class="sy-todos">
                                {items
                                    .into_iter()
                                    .map(|todo| {
                                        view! {
                                            <li class="sy-todo" class:sy-todo--done=todo.completed>
                                                <p class="sy-todo__title">{todo.title.clone()}</p>
                                                <span class="sy-todo__state">
                                                    {if todo.completed { "selesai" } else { "terbuka" }}
                                                </span>
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                            .into_any()
                    }
                }}
            </section>
        </MobileLayout>
    }
}
