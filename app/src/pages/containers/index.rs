//! `/containers` — daftar container dengan tarik-untuk-menyegarkan.

use leptos::prelude::*;
use sayagi::{MobileLayout, PullToRefresh};

use crate::api::get_containers;
use crate::components::{ContainerCard, EmptyState};

/// Komponen halaman daftar container.
#[component]
pub fn Page() -> impl IntoView {
    let containers = Resource::new(|| (), |_| async { get_containers().await });
    let on_refresh = Callback::new(move |_| containers.refetch());

    view! {
        <MobileLayout title="Container">
            <PullToRefresh label="Tarik untuk memuat ulang" on_refresh=on_refresh>
                <Suspense fallback=|| view! { <p class="sy-muted">"Memuat container…"</p> }>
                    {move || {
                        containers
                            .get()
                            .map(|result| match result {
                                Ok(items) if items.is_empty() => {
                                    view! {
                                        <EmptyState
                                            title="Belum ada container"
                                            hint="Tabel `containers` masih kosong."
                                        />
                                    }
                                        .into_any()
                                }
                                Ok(items) => {
                                    view! {
                                        <div class="sy-cards">
                                            {items
                                                .into_iter()
                                                .map(|container| view! { <ContainerCard container/> })
                                                .collect_view()}
                                        </div>
                                    }
                                        .into_any()
                                }
                                Err(error) => {
                                    view! { <p class="sy-error">{error.to_string()}</p> }.into_any()
                                }
                            })
                    }}
                </Suspense>
            </PullToRefresh>
        </MobileLayout>
    }
}
