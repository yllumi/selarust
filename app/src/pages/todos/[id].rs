//! `/todos/:id` — detail, ubah, dan hapus satu todo.
//!
//! Nama berkas `[id].rs` dipetakan ke segmen dinamis `:id`; halaman membaca
//! parameternya lewat `sayagi::param`. Bila server tidak terjangkau, baris
//! diambil dari snapshot lokal supaya halaman tetap bisa dibuka offline.

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use sayagi::{param, LinkButton, MobileLayout};

use crate::api::{get_todo, Todo, MAX_TITLE};
use crate::components::EmptyState;
use crate::store::{TodoMutation, TodoStore};

/// Komponen halaman detail todo.
#[component]
pub fn Page() -> impl IntoView {
    let store = expect_context::<TodoStore>();
    let id = param("id");
    let navigate = use_navigate();

    // Kolom form hidup di level halaman supaya editan tidak tertimpa ketika
    // store berubah (mis. event sinkronisasi dari tab lain).
    let title = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let seeded = RwSignal::new(None::<i64>);

    let revision = RwSignal::new(0u32);
    let loaded = Resource::new(
        move || (id.get(), revision.get()),
        |(id, _)| async move { get_todo(id).await },
    );

    // Salinan lokal dipakai ketika permintaan ke server gagal (mode offline).
    let offline_copy = Signal::derive(move || {
        let wanted: Option<i64> = id.get().parse().ok();
        wanted.and_then(|wanted| {
            store
                .items()
                .get()
                .into_iter()
                .find(|todo| todo.id == wanted)
        })
    });

    // Isi kolom form sekali per id; sesudah itu editan pengguna yang menang.
    Effect::new(move |_| {
        let item = loaded
            .get()
            .and_then(|result| result.ok().flatten())
            .or_else(|| offline_copy.get());
        if let Some(item) = item {
            if seeded.get_untracked() != Some(item.id) {
                title.set(item.title.clone());
                notes.set(item.notes.clone());
                seeded.set(Some(item.id));
            }
        }
    });

    view! {
        <MobileLayout title="Detail todo" show_back=true>
            <Transition fallback=|| view! { <p class="sy-muted">"Memuat detail…"</p> }>
                {move || {
                    loaded
                        .get()
                        .map(|result| match result {
                            Ok(Some(item)) => {
                                view! {
                                    <Detail
                                        item
                                        title
                                        notes
                                        store
                                        navigate=navigate.clone()
                                        on_saved=Callback::new(move |_| {
                                            revision.update(|value| *value += 1)
                                        })
                                    />
                                }
                                    .into_any()
                            }
                            Ok(None) => {
                                view! {
                                    <EmptyState
                                        title="Todo tidak ditemukan"
                                        hint="Baris ini mungkin sudah dihapus dari database."
                                    />
                                }
                                    .into_any()
                            }
                            // Gagal menghubungi server: pakai salinan lokal bila ada.
                            Err(_) => match offline_copy.get() {
                                Some(item) => {
                                    view! {
                                        <Detail
                                            item
                                            title
                                            notes
                                            store
                                            navigate=navigate.clone()
                                            on_saved=Callback::new(move |_| {
                                                revision.update(|value| *value += 1)
                                            })
                                        />
                                    }
                                        .into_any()
                                }
                                None => {
                                    view! {
                                        <p class="sy-error">
                                            "Tidak bisa memuat data dan belum ada salinan offline."
                                        </p>
                                    }
                                        .into_any()
                                }
                            },
                        })
                }}
            </Transition>
        </MobileLayout>
    }
}

/// Halaman detail: form ubah, tombol status, dan tombol hapus.
#[component]
fn Detail(
    item: Todo,
    title: RwSignal<String>,
    notes: RwSignal<String>,
    store: TodoStore,
    navigate: impl Fn(&str, NavigateOptions) + Clone + 'static,
    on_saved: Callback<()>,
) -> impl IntoView {
    let id = item.id;
    let completed = item.completed;
    let status = if completed { "selesai" } else { "terbuka" };

    let save = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let new_title = title.get_untracked().trim().to_string();
        if new_title.is_empty() {
            return;
        }
        let new_notes = notes.get_untracked().trim().to_string();
        leptos::task::spawn_local(async move {
            store
                .enqueue(TodoMutation::Update {
                    id,
                    title: new_title,
                    notes: new_notes,
                })
                .await;
            store.flush().await;
        });
        on_saved.run(());
    };

    let toggle = move |_| {
        leptos::task::spawn_local(async move {
            store
                .enqueue(TodoMutation::Complete {
                    id,
                    completed: !completed,
                })
                .await;
            store.flush().await;
        });
        on_saved.run(());
    };

    let remove = {
        let navigate = navigate.clone();
        move |_| {
            let navigate = navigate.clone();
            leptos::task::spawn_local(async move {
                store.enqueue(TodoMutation::Delete { id }).await;
                store.flush().await;
            });
            navigate("/todos", Default::default());
        }
    };

    view! {
        <section class="sy-page">
            <p class="sy-overline">{format!("todo #{id}")}</p>

            <form class="sy-form" on:submit=save>
                <label class="sy-field">
                    <span class="sy-field__label">"Judul"</span>
                    <input
                        class="sy-input"
                        type="text"
                        maxlength=MAX_TITLE
                        prop:value=move || title.get()
                        on:input=move |event| title.set(event_target_value(&event))
                    />
                </label>
                <label class="sy-field">
                    <span class="sy-field__label">"Catatan"</span>
                    <textarea
                        class="sy-input"
                        rows="4"
                        prop:value=move || notes.get()
                        on:input=move |event| notes.set(event_target_value(&event))
                    ></textarea>
                </label>
                <button class="sy-btn-solid" type="submit">"Simpan"</button>
            </form>

            <dl class="sy-detail">
                <div class="sy-detail__row">
                    <dt>"Status"</dt>
                    <dd>
                        <span class=if completed { "sy-badge sy-badge--ok" } else { "sy-badge" }>
                            {status}
                        </span>
                    </dd>
                </div>
                <div class="sy-detail__row">
                    <dt>"Dibuat"</dt>
                    <dd>{item.created_at.clone()}</dd>
                </div>
            </dl>

            <div class="sy-actions">
                <button class="sy-btn-solid sy-btn-solid--ghost" type="button" on:click=toggle>
                    {if completed { "Tandai terbuka" } else { "Tandai selesai" }}
                </button>
                <button class="sy-btn-solid sy-btn-solid--danger" type="button" on:click=remove>
                    "Hapus todo"
                </button>
            </div>

            <LinkButton href="/todos" variant="ghost">
                "Kembali ke daftar"
            </LinkButton>
        </section>
    }
}
