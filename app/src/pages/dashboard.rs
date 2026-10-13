//! `/dashboard` — statistik todo dan tabel rute hasil generate.

use leptos::prelude::*;
use selarust::MobileLayout;

use crate::api::{get_todo_stats, TodoStats};
use crate::components::StatTile;
use crate::pages::ROUTES;

/// Komponen halaman status.
#[component]
pub fn Page() -> impl IntoView {
    let stats = Resource::new(|| (), |_| async { get_todo_stats().await });

    view! {
        <MobileLayout title="Status">
            <section class="sr-section">
                <h3 class="sr-section__title">"Todo"</h3>
                <Suspense fallback=|| view! { <p class="sr-muted">"Memuat statistik…"</p> }>
                    {move || {
                        stats
                            .get()
                            .map(|result| match result {
                                Ok(stats) => view! { <StatRow stats=stats/> }.into_any(),
                                Err(error) => {
                                    view! { <p class="sr-error">{error.to_string()}</p> }.into_any()
                                }
                            })
                    }}
                </Suspense>
            </section>

            <section class="sr-section">
                <h3 class="sr-section__title">"Rute yang digenerate"</h3>
                <p class="sr-muted sr-small">
                    "Dipindai dari " <code class="sr-code">"src/pages"</code>
                    " saat build — tidak ada daftar rute manual."
                </p>
                <ul class="sr-list sr-list--tight">
                    {ROUTES
                        .iter()
                        .map(|route| {
                            view! {
                                <li class="sr-list__item sr-list__item--column">
                                    <span class="sr-list__head">
                                        <code class="sr-code">{route.path}</code>
                                        <span class="sr-chip">{route.label()}</span>
                                    </span>
                                    <span class="sr-muted sr-small">{route.file}</span>
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
fn StatRow(stats: TodoStats) -> impl IntoView {
    view! {
        <div class="sr-stats">
            <StatTile label="Total" value=stats.total.to_string()/>
            <StatTile label="Terbuka" value=stats.open.to_string()/>
            <StatTile label="Selesai" value=stats.done.to_string() tone="ok"/>
        </div>
    }
}
