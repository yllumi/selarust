//! Komponen khusus aplikasi.
//!
//! Primitif yang disediakan framework (`MobileLayout`, `HeaderBar`, `BottomNav`,
//! `PullToRefresh`) tinggal di crate `sayagi`; folder ini untuk komponen milik
//! aplikasi sendiri.

use leptos::prelude::*;
use leptos_router::components::A;
use sayagi::{ConnectionState, SyncClient};

use crate::api::Todo;
use crate::store::TodoStore;

/// Satu baris daftar todo: tombol centang, judul menuju detail, dan tombol hapus.
///
/// Status selesai disimpan di sinyal lokal supaya ketukan langsung terlihat
/// (optimistis). Bila server menolak, refetch daftar akan mengembalikan nilai
/// sebenarnya — `on_toggle` tidak perlu mengurus rollback sendiri.
#[component]
pub fn TodoRow(
    /// Data baris.
    todo: Todo,
    /// Dipanggil dengan `(id, selesai_baru)`.
    on_toggle: Callback<(i64, bool)>,
    /// Dipanggil dengan `id` saat tombol hapus ditekan.
    on_delete: Callback<i64>,
) -> impl IntoView {
    let id = todo.id;
    let done = RwSignal::new(todo.completed);
    let href = format!("/todos/{id}");
    let title = todo.title.clone();
    let notes = todo.notes.clone();

    let toggle = move |_| {
        let next = !done.get_untracked();
        done.set(next);
        on_toggle.run((id, next));
    };

    view! {
        <li class="sy-todo" class:sy-todo--done=done>
            <button
                type="button"
                class="sy-todo__check"
                role="checkbox"
                aria-checked=move || if done.get() { "true" } else { "false" }
                aria-label="Tandai selesai"
                on:click=toggle
            >
                <span class="sy-todo__mark" inner_html=sayagi::icon_svg("check")></span>
            </button>

            <A href=href>
                <p class="sy-todo__title">{title}</p>
                <p class="sy-todo__meta">
                    {if notes.is_empty() {
                        "tidak ada catatan".to_string()
                    } else {
                        notes.clone()
                    }}
                </p>
            </A>

            <span class="sy-todo__state">
                {move || if done.get() { "selesai" } else { "terbuka" }}
            </span>

            <button
                type="button"
                class="sy-todo__delete"
                aria-label="Hapus todo"
                on:click=move |_| on_delete.run(id)
            >
                "×"
            </button>
        </li>
    }
}


/// Strip status sinkronisasi: keadaan kanal realtime dan mutasi tertunda.
///
/// Selalu ada di DOM (supaya hidrasi tidak berbeda dari HTML SSR) tetapi hanya
/// terlihat setelah aplikasi ter-hidrasi, ketika memang ada yang perlu
/// dilaporkan: koneksi terputus atau antrean belum kosong.
#[component]
pub fn SyncStrip() -> impl IntoView {
    let store = expect_context::<TodoStore>();
    let client = use_context::<SyncClient>();
    let mounted = RwSignal::new(false);
    Effect::new(move |_| mounted.set(true));

    let state = Signal::derive(move || {
        client
            .map(|client| client.state().get())
            .unwrap_or(ConnectionState::Offline)
    });
    let online = Signal::derive(move || state.get() == ConnectionState::Online);
    let pending = store.pending_count();
    let visible = Signal::derive(move || {
        mounted.get() && (pending.get() > 0 || state.get() == ConnectionState::Offline)
    });

    let sync_now = move |_| {
        if let Some(client) = client {
            // Minta snapshot lagi: keadaan lokal bisa tertinggal saat terputus.
            client.send(r#"{"type":"subscribe"}"#);
        }
        leptos::task::spawn_local(async move { store.flush().await });
    };

    view! {
        <div class="sy-sync" class:is-visible=visible role="status" aria-live="polite">
            <span class="sy-sync__dot" class:is-on=online aria-hidden="true"></span>
            <span class="sy-sync__label">{move || state.get().label()}</span>
            {move || {
                (pending.get() > 0)
                    .then(|| {
                        view! {
                            <span class="sy-sync__count">
                                {format!("{} perubahan menunggu", pending.get())}
                            </span>
                        }
                    })
            }}
            {move || {
                store
                    .error()
                    .get()
                    .map(|error| view! { <span class="sy-sync__error">{error}</span> })
            }}
            <button
                class="sy-sync__action"
                type="button"
                disabled=move || store.syncing().get() || pending.get() == 0
                on:click=sync_now
            >
                "Sinkronkan"
            </button>
        </div>
    }
}

/// Kotak angka untuk halaman status.
#[component]
pub fn StatTile(
    /// Label di atas angka.
    #[prop(into)]
    label: String,
    /// Nilai yang ditonjolkan.
    #[prop(into)]
    value: String,
    /// Varian warna: `ok`, `off`, atau kosong.
    #[prop(into, optional)]
    tone: Option<String>,
) -> impl IntoView {
    let class = match tone.as_deref() {
        Some("ok") => "sy-stat sy-stat--ok",
        Some("off") => "sy-stat sy-stat--off",
        _ => "sy-stat",
    };

    view! {
        <div class=class>
            <p class="sy-stat__value">{value}</p>
            <p class="sy-stat__label">{label}</p>
        </div>
    }
}

/// Keadaan kosong atau pesan galat yang ramah di layar kecil.
#[component]
pub fn EmptyState(
    /// Judul singkat.
    #[prop(into)]
    title: String,
    /// Penjelasan tambahan.
    #[prop(into, optional)]
    hint: Option<String>,
) -> impl IntoView {
    view! {
        <section class="sy-page sy-empty">
            <h2 class="sy-empty__title">{title}</h2>
            {hint.map(|hint| view! { <p class="sy-muted">{hint}</p> })}
        </section>
    }
}
