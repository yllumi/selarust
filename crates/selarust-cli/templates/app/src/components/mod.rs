//! Komponen khusus aplikasi.
//!
//! Primitif framework (`MobileLayout`, `HeaderBar`, `BottomNav`,
//! `PullToRefresh`) disediakan crate `selarust`; folder ini untuk komponen milik
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
        <section class="sr-page sr-empty">
            <h2 class="sr-empty__title">{title}</h2>
            {hint.map(|hint| view! { <p class="sr-muted">{hint}</p> })}
        </section>
    }
}
