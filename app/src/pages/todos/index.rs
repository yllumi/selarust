//! `/todos` — daftar todo dari store klien, filter + tambah lewat bottom sheet.
//!
//! Semua mutasi masuk ke antrean offline lebih dulu (`TodoMutation`), lalu
//! diputar ulang ke server: satu jalur kode untuk online maupun offline.

use leptos::prelude::*;
use sayagi::{icon_svg, BottomSheet, MobileLayout, PullToRefresh};

use crate::api::{list_todos, Todo, MAX_TITLE};
use crate::components::{EmptyState, TodoRow};
use crate::store::{TodoMutation, TodoStore};

/// Penyaring daftar yang berlaku di sisi klien.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Open,
    Done,
}

impl Filter {
    const ALL: [Filter; 3] = [Filter::All, Filter::Open, Filter::Done];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "Semua",
            Filter::Open => "Terbuka",
            Filter::Done => "Selesai",
        }
    }

    fn matches(self, todo: &Todo) -> bool {
        match self {
            Filter::All => true,
            Filter::Open => !todo.completed,
            Filter::Done => todo.completed,
        }
    }
}

/// Komponen halaman daftar todo.
#[component]
pub fn Page() -> impl IntoView {
    let store = expect_context::<TodoStore>();

    let add_open = RwSignal::new(false);
    let filter_open = RwSignal::new(false);
    let filter = RwSignal::new(Filter::All);
    let title = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let revision = RwSignal::new(0u32);

    // Data awal dari server. Nilainya dirender di dalam batas `<Transition>` —
    // sama seperti halaman lain — supaya markup awal klien (fallback) identik
    // dengan markup SSR saat hidrasi.
    let server = Resource::new(move || revision.get(), |_| async { list_todos().await });

    // Klien: simpan hasil server sebagai snapshot lokal (dan bahan baku store).
    Effect::new(move |_| {
        if let Some(Ok(items)) = server.get() {
            store.remember(items);
        }
    });

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        let text = title.get_untracked().trim().to_string();
        if text.is_empty() {
            return;
        }
        let note = notes.get_untracked().trim().to_string();
        title.set(String::new());
        notes.set(String::new());
        add_open.set(false);
        leptos::task::spawn_local(async move {
            store
                .enqueue(TodoMutation::Create {
                    temp_id: store.next_temp_id(),
                    title: text,
                    notes: note,
                })
                .await;
            store.flush().await;
        });
    };

    let on_toggle = Callback::new(move |(id, completed): (i64, bool)| {
        leptos::task::spawn_local(async move {
            store.enqueue(TodoMutation::Complete { id, completed }).await;
            store.flush().await;
        });
    });
    let on_delete = Callback::new(move |id: i64| {
        leptos::task::spawn_local(async move {
            store.enqueue(TodoMutation::Delete { id }).await;
            store.flush().await;
        });
    });
    let on_refresh = Callback::new(move |_| {
        revision.update(|value| *value += 1);
        leptos::task::spawn_local(async move {
            store.refresh().await;
            store.flush().await;
        });
    });

    let actions = view! {
        <button
            class="sy-icon-btn"
            type="button"
            aria-label="Filter"
            on:click=move |_| filter_open.set(true)
            inner_html=icon_svg("filter")
        ></button>
        <button
            class="sy-icon-btn"
            type="button"
            aria-label="Tambah todo"
            on:click=move |_| add_open.set(true)
            inner_html=icon_svg("plus")
        ></button>
    }
    .into_any();

    // Daftar dirender dari satu tempat supaya cabang "server tidak terjangkau"
    // bisa memakai sumber yang sama (store) ketika Resource gagal diisi.
    let list_section = move |base: Vec<Todo>| {
        view! {
            <section class="sy-section">
                <h3 class="sy-section__title">
                    {move || format!("Daftar · {}", filter.get().label())}
                </h3>
                // Setelah hidrasi daftar mengikuti store: snapshot lokal + mutasi
                // yang belum tersinkron. Sebelum itu nilainya sama dengan `base`.
                {move || {
                    let base = if store.ready().get() { store.items().get() } else { base.clone() };
                    let active = filter.get();
                    let visible: Vec<Todo> = base
                        .into_iter()
                        .filter(|todo| active.matches(todo))
                        .collect();
                    view! { <TodoList items=visible filter=active on_toggle on_delete/> }
                }}
            </section>
        }
    };

    view! {
        <MobileLayout title="Todo" actions=actions>
            <PullToRefresh label="Tarik untuk memuat ulang" on_refresh=on_refresh>
                <Transition fallback=|| view! { <p class="sy-muted">"Memuat todo…"</p> }>
                    {move || match server.get() {
                        Some(Ok(items)) => list_section(items).into_any(),
                        // Offline: server tidak terjangkau, jadi tampilkan salinan
                        // lokal beserta antrean perubahan yang belum terkirim.
                        Some(Err(error)) => {
                            view! {
                                <p class="sy-muted sy-small">
                                    "Server tidak terjangkau — menampilkan salinan lokal."
                                </p>
                                {list_section(Vec::new())}
                                <p class="sy-error">{error.to_string()}</p>
                            }
                                .into_any()
                        }
                        None => ().into_any(),
                    }}
                </Transition>
            </PullToRefresh>

            <BottomSheet open=add_open title="Tambah todo">
                <form class="sy-form" on:submit=submit>
                    <label class="sy-field">
                        <span class="sy-field__label">"Judul"</span>
                        <input
                            class="sy-input"
                            type="text"
                            placeholder="Mau dikerjakan apa?"
                            autocomplete="off"
                            maxlength=MAX_TITLE
                            prop:value=move || title.get()
                            on:input=move |event| title.set(event_target_value(&event))
                        />
                    </label>
                    <label class="sy-field">
                        <span class="sy-field__label">"Catatan (opsional)"</span>
                        <textarea
                            class="sy-input"
                            rows="3"
                            placeholder="Detail singkat"
                            prop:value=move || notes.get()
                            on:input=move |event| notes.set(event_target_value(&event))
                        ></textarea>
                    </label>
                    <button class="sy-btn-solid" type="submit">"Simpan todo"</button>
                </form>
            </BottomSheet>

            <BottomSheet open=filter_open title="Tampilkan">
                <div class="sy-choices">
                    {Filter::ALL
                        .into_iter()
                        .map(|option| {
                            view! {
                                <button
                                    class="sy-choice"
                                    type="button"
                                    aria-pressed=move || (filter.get() == option).to_string()
                                    on:click=move |_| {
                                        filter.set(option);
                                        filter_open.set(false);
                                    }
                                >
                                    <span>{option.label()}</span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </BottomSheet>
        </MobileLayout>
    }
}

/// Daftar baris todo, atau keadaan kosong bila tidak ada yang cocok.
#[component]
fn TodoList(
    items: Vec<Todo>,
    filter: Filter,
    on_toggle: Callback<(i64, bool)>,
    on_delete: Callback<i64>,
) -> impl IntoView {
    if items.is_empty() {
        let hint = match filter {
            Filter::All => "Tambahkan lewat tombol + di kanan atas — datanya disimpan di SQLite.",
            Filter::Open => "Semua todo sudah selesai.",
            Filter::Done => "Belum ada todo yang ditandai selesai.",
        };
        return view! { <EmptyState title="Tidak ada todo" hint=hint/> }.into_any();
    }

    view! {
        <ul class="sy-todos">
            {items
                .into_iter()
                .map(|todo| view! { <TodoRow todo on_toggle on_delete/> })
                .collect_view()}
        </ul>
    }
    .into_any()
}
