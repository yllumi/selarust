//! Komponen khusus aplikasi.
//!
//! Primitif framework (`MobileLayout`, `HeaderBar`, `BottomNav`,
//! `PullToRefresh`) disediakan crate `sayagi`; folder ini untuk komponen milik
//! aplikasi sendiri.

use leptos::prelude::*;

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
