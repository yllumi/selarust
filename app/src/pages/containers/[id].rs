//! `/containers/:id` — detail container.
//!
//! Nama berkas `[id].rs` dipetakan ke segmen dinamis `:id`; halaman membaca
//! parameternya lewat `sayagi::param` tanpa perlu mendeklarasikan tipe rute.

use leptos::prelude::*;
use sayagi::{LinkButton, MobileLayout, param};

use crate::api::{get_container, Container};
use crate::components::EmptyState;

/// Komponen halaman detail container.
#[component]
pub fn Page() -> impl IntoView {
    let id = param("id");
    let container = Resource::new(move || id.get(), |id| async move { get_container(id).await });

    view! {
        <MobileLayout title="Detail" show_back=true>
            <Suspense fallback=|| view! { <p class="sy-muted">"Memuat detail…"</p> }>
                {move || {
                    container
                        .get()
                        .map(|result| match result {
                            Ok(Some(container)) => {
                                view! { <Detail container=container/> }.into_any()
                            }
                            Ok(None) => {
                                view! {
                                    <EmptyState
                                        title="Container tidak ditemukan"
                                        hint="Baris ini mungkin sudah dihapus dari database."
                                    />
                                }
                                    .into_any()
                            }
                            Err(error) => {
                                view! { <p class="sy-error">{error.to_string()}</p> }.into_any()
                            }
                        })
                }}
            </Suspense>
        </MobileLayout>
    }
}

#[component]
fn Detail(container: Container) -> impl IntoView {
    let badge_class = match container.status.as_str() {
        "running" => "sy-badge sy-badge--ok",
        "stopped" => "sy-badge sy-badge--off",
        _ => "sy-badge sy-badge--idle",
    };
    let id = container.id;
    let name = container.name.clone();
    let status = container.status.clone();

    view! {
        <section class="sy-page">
            <p class="sy-overline">{format!("container #{id}")}</p>
            <h2 class="sy-title">{name.clone()}</h2>
            <span class=badge_class>{status.clone()}</span>

            <dl class="sy-detail">
                <div class="sy-detail__row">
                    <dt>"ID"</dt>
                    <dd>{id.to_string()}</dd>
                </div>
                <div class="sy-detail__row">
                    <dt>"Status"</dt>
                    <dd>{status.clone()}</dd>
                </div>
            </dl>

            <LinkButton href="/containers" variant="ghost">
                "Kembali ke daftar"
            </LinkButton>
        </section>
    }
}
