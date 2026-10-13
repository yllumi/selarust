//! Access to the `[param]` segments of the current route.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

/// Reads a route parameter as a reactive signal.
///
/// Returns an empty string when the parameter is absent, which keeps page code
/// free of `unwrap()` noise:
///
/// ```ignore
/// let id = param("id");
/// view! { <h1>{move || format!("Container {}", id.get())}</h1> }
/// ```
///
/// # Panics
///
/// Like every `leptos_router` hook, this must be called while a [`Router`] is
/// mounted above the component.
///
/// [`Router`]: leptos_router::components::Router
pub fn param(name: &'static str) -> Signal<String> {
    let params = use_params_map();
    Signal::derive(move || params.with(|map| map.get(name).unwrap_or_default()))
}

/// Like [`param`], but falls back to `fallback` when the segment is missing or empty.
pub fn param_or(name: &'static str, fallback: &'static str) -> Signal<String> {
    let params = use_params_map();
    Signal::derive(move || {
        params.with(|map| {
            map.get(name)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback.to_string())
        })
    })
}
