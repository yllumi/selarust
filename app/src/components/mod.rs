//! Komponen khusus aplikasi.
//!
//! Primitif yang disediakan framework (`MobileLayout`, `HeaderBar`, `BottomNav`,
//! `PullToRefresh`) tinggal di crate `sayagi`; folder ini untuk komponen milik
//! aplikasi sendiri.

use leptos::prelude::*;
use leptos_router::components::A;

use crate::api::Container;

/// Kartu ringkas satu container; seluruh kartu bisa diketuk.
#[component]
pub fn ContainerCard(container: Container) -> impl IntoView {
    let href = format!("/containers/{}", container.id);
    let badge_class = match container.status.as_str() {
        "running" => "sy-badge sy-badge--ok",
        "stopped" => "sy-badge sy-badge--off",
        _ => "sy-badge sy-badge--idle",
    };
    let name = container.name.clone();
    let status = container.status.clone();
    let id = container.id;

    view! {
        <div class="sy-card">
            <A href=href>
                <div class="sy-card__body">
                    <p class="sy-card__title">{name}</p>
                    <p class="sy-card__meta">{format!("#{id}")}</p>
                </div>
                <span class=badge_class>{status}</span>
            </A>
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
