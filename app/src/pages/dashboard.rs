//! `/dashboard` — statistik database dan tabel rute hasil generate.

use leptos::prelude::*;
use sayagi::MobileLayout;

use crate::api::{get_stats, Stats};
use crate::components::StatTile;
use crate::pages::ROUTES;

/// Komponen halaman status.
#[component]
pub fn Page() -> impl IntoView {
    let stats = Resource::new(|| (), |_| async { get_stats().await });

    view! {
        <MobileLayout title="Status">
            <section class="sy-section">
                <h3 class="sy-section__title">"Container"</h3>
                <Suspense fallback=|| view! { <p class="sy-muted">"Memuat statistik…"</p> }>
                    {move || {
                        stats
                            .get()
                            .map(|result| match result {
                                Ok(stats) => view! { <StatRow stats=stats/> }.into_any(),
                                Err(error) => {
                                    view! { <p class="sy-error">{error.to_string()}</p> }.into_any()
                                }
                            })
                    }}
                </Suspense>
            </section>

            <section class="sy-section">
                <h3 class="sy-section__title">"Rute yang digenerate"</h3>
                <p class="sy-muted sy-small">
                    "Dipindai dari " <code class="sy-code">"src/pages"</code>
                    " saat build — tidak ada daftar rute manual."
                </p>
                <ul class="sy-list sy-list--tight">
                    {ROUTES
                        .iter()
                        .map(|route| {
                            view! {
                                <li class="sy-list__item sy-list__item--column">
                                    <span class="sy-list__head">
                                        <code class="sy-code">{route.path}</code>
                                        <span class="sy-chip">{route.label()}</span>
                                    </span>
                                    <span class="sy-muted sy-small">{route.file}</span>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </section>
        </MobileLayout>
    }
}

#[component]
fn StatRow(stats: Stats) -> impl IntoView {
    view! {
        <div class="sy-stats">
            <StatTile label="Total" value=stats.total.to_string()/>
            <StatTile label="Running" value=stats.running.to_string() tone="ok"/>
            <StatTile label="Berhenti" value=stats.stopped.to_string() tone="off"/>
        </div>
    }
}
