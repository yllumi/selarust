//! The mobile UI shell: layout, header, bottom navigation and pull-to-refresh.
//!
//! Styling lives in the app stylesheet; the components only emit stable class
//! names (`sy-*`) so themes stay a pure CSS concern.

use leptos::ev;
use leptos::prelude::*;
use leptos_router::components::A;

/// Pull distance (in CSS pixels) that arms the refresh gesture.
pub const PULL_THRESHOLD: f64 = 72.0;

/// One entry of the [`BottomNav`] bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavItem {
    /// Visible label.
    pub label: &'static str,
    /// Router href.
    pub href: &'static str,
    /// Icon name understood by [`icon_svg`].
    pub icon: &'static str,
}

impl NavItem {
    /// Builds a navigation entry.
    pub const fn new(label: &'static str, href: &'static str, icon: &'static str) -> Self {
        Self { label, href, icon }
    }
}

/// Navigation used by [`BottomNav`] when no custom list is supplied.
pub fn default_nav_items() -> Vec<NavItem> {
    vec![
        NavItem::new("Beranda", "/", "home"),
        NavItem::new("Container", "/containers", "grid"),
        NavItem::new("Status", "/dashboard", "chart"),
    ]
}

/// Inline SVG markup for the built-in icon set (`home`, `grid`, `chart`, `back`).
pub fn icon_svg(name: &str) -> String {
    let path = match name {
        "home" => "M4 11.2 12 4l8 7.2V20a1 1 0 0 1-1 1h-4.5v-5.5h-5V21H5a1 1 0 0 1-1-1z",
        "grid" => "M4 4h6.5v6.5H4zm9.5 0H20v6.5h-6.5zM4 13.5h6.5V20H4zm9.5 0H20V20h-6.5z",
        "chart" => "M4 20v-9h3.5v9zm6.2 0V4h3.5v16zm6.3 0v-6h3.5v6z",
        "back" => "M14.7 5.3 8 12l6.7 6.7-1.4 1.4L5.2 12l8.1-8.1z",
        _ => "M12 5a7 7 0 1 0 0 14 7 7 0 0 0 0-14z",
    };
    format!(r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="{path}"/></svg>"#)
}

/// Mobile-first app shell: sticky header, scrollable content, bottom navigation.
#[component]
pub fn MobileLayout(
    /// Header title. The header is omitted when empty.
    #[prop(into, optional)]
    title: Option<String>,
    /// Render the bottom navigation bar.
    #[prop(default = true)]
    show_nav: bool,
    /// Show a back button in the header.
    #[prop(default = false)]
    show_back: bool,
    children: Children,
) -> impl IntoView {
    let title = title.unwrap_or_default();
    let has_title = !title.is_empty();

    view! {
        <div class="sy-app">
            {has_title.then(|| view! { <HeaderBar title=title.clone() show_back=show_back/> })}
            <OfflineBanner/>
            <main class="sy-main">{children()}</main>
            {show_nav.then(|| view! { <BottomNav/> })}
        </div>
    }
}

/// Sticky top bar with an optional back button and trailing actions.
#[component]
pub fn HeaderBar(
    /// Title text.
    #[prop(into, optional)]
    title: Option<String>,
    /// Show a back button that pops the history stack.
    #[prop(optional)]
    show_back: bool,
    /// Trailing action buttons.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    // `use_navigate` hanya bisa menuju sebuah path — string "-1" akan dianggap
    // rute biasa — jadi tombol kembali memakai History API langsung. Router
    // Leptos tetap ikut menyesuaikan lewat event `popstate`.
    let go_back = move |_| {
        if let Some(window) = leptos::web_sys::window() {
            if let Ok(history) = window.history() {
                let _ = history.back();
            }
        }
    };
    let title = title.unwrap_or_default();

    view! {
        <header class="sy-header">
            {show_back
                .then(|| {
                    view! {
                        <button
                            class="sy-header__back"
                            type="button"
                            aria-label="Kembali"
                            on:click=go_back
                            inner_html=icon_svg("back")
                        ></button>
                    }
                })}
            <h1 class="sy-header__title">{title}</h1>
            <div class="sy-header__actions">{children.map(|children| children())}</div>
        </header>
    }
}

/// Bottom navigation bar.
///
/// The active entry is marked by `aria-current="page"`, which
/// [`A`](leptos_router::components::A) sets on its own and CSS styles with
/// `.sy-nav a[aria-current="page"]`. Routing stays client-side because the links
/// are real router links.
#[component]
pub fn BottomNav(
    /// Custom entries. Defaults to [`default_nav_items`].
    #[prop(optional)]
    items: Option<Vec<NavItem>>,
) -> impl IntoView {
    let items = items.unwrap_or_else(default_nav_items);

    let links = items
        .into_iter()
        .map(|item| {
            view! {
                <A href=item.href>
                    <span class="sy-nav__icon" inner_html=icon_svg(item.icon)></span>
                    <span class="sy-nav__label">{item.label}</span>
                </A>
            }
        })
        .collect_view();

    view! { <nav class="sy-nav" aria-label="Navigasi utama">{links}</nav> }
}

/// A button-styled router link.
///
/// Leptos' `<A>` component does not accept extra attributes (classes included), so
/// the button chrome lives on a wrapping element and CSS styles `.sy-btn > a`.
/// That keeps client-side routing while still looking like a button.
#[component]
pub fn LinkButton(
    /// Target route.
    #[prop(into)]
    href: String,
    /// `ghost` for the outlined variant.
    #[prop(optional, into)]
    variant: Option<String>,
    children: Children,
) -> impl IntoView {
    let class = match variant.as_deref() {
        Some("ghost") => "sy-btn sy-btn--ghost",
        _ => "sy-btn",
    };

    view! {
        <span class=class>
            <A href=href>{children()}</A>
        </span>
    }
}

/// Wraps scrollable content in a pull-to-refresh gesture.
///
/// The gesture is driven by touch events (a pull is a touch gesture, and the
/// browser cancels pointer sequences as soon as it starts a scroll gesture). It
/// only engages while the window is scrolled to the very top, and once the pull
/// passes [`PULL_THRESHOLD`], `on_refresh` fires and the content springs back.
#[component]
pub fn PullToRefresh(
    /// Hint shown above the content while pulling.
    #[prop(into, optional)]
    label: Option<String>,
    /// Called once per completed pull gesture.
    #[prop(optional)]
    on_refresh: Option<Callback<()>>,
    children: Children,
) -> impl IntoView {
    let label = label.unwrap_or_else(|| "Tarik untuk menyegarkan".to_string());
    let start_y = RwSignal::new(None::<f64>);
    let distance = RwSignal::new(0.0f64);
    let armed = Signal::derive(move || distance.get() >= PULL_THRESHOLD);
    let pulling = Signal::derive(move || distance.get() > 4.0);

    let on_touch_start = move |event: ev::TouchEvent| {
        let touch_y = event.touches().item(0).map(|touch| touch.client_y() as f64);
        start_y.set(if at_top() { touch_y } else { None });
    };
    let on_touch_move = move |event: ev::TouchEvent| {
        let Some(start) = start_y.get() else {
            return;
        };
        let Some(current) = event.touches().item(0).map(|touch| touch.client_y() as f64) else {
            return;
        };
        let delta = current - start;
        // Rubber-band the pull so it feels progressive instead of linear.
        distance.set(if delta > 0.0 {
            (delta * 0.55).min(140.0)
        } else {
            0.0
        });
    };
    let finish = move |_| {
        let pulled = distance.get_untracked() >= PULL_THRESHOLD;
        start_y.set(None);
        distance.set(0.0);
        if pulled {
            if let Some(callback) = on_refresh {
                callback.run(());
            }
        }
    };

    view! {
        <div
            class="sy-pull"
            class:is-armed=armed
            class:is-pulling=pulling
            on:touchstart=on_touch_start
            on:touchmove=on_touch_move
            on:touchend=finish
            on:touchcancel=finish
        >
            <div class="sy-pull__indicator" aria-hidden="true">
                <span class="sy-pull__spinner"></span>
                <span class="sy-pull__label">{label}</span>
            </div>
            <div
                class="sy-pull__content"
                style:transform=move || format!("translateY({}px)", distance.get())
            >
                {children()}
            </div>
        </div>
    }
}

/// Whether the window is scrolled all the way up, i.e. a downward pull is not a scroll.
fn at_top() -> bool {
    leptos::web_sys::window()
        .and_then(|window| window.scroll_y().ok())
        .map(|offset| offset <= 0.0)
        .unwrap_or(false)
}

/// Banner that CSS reveals while `navigator.onLine` is `false`.
#[component]
fn OfflineBanner() -> impl IntoView {
    view! {
        <div class="sy-offline-banner" role="status">
            <span class="sy-offline-banner__dot"></span>
            "Mode offline — data diambil dari cache."
        </div>
    }
}
