//! The mobile UI shell: layout, header, bottom navigation, pull-to-refresh,
//! edge swipe-back and the drag-to-dismiss bottom sheet.
//!
//! Styling lives in the app stylesheet; the components only emit stable class
//! names (`sr-*`) so themes stay a pure CSS concern. [`MobileLayout`] needs to
//! live inside a `<Router>`: besides the navigation entries it reports the depth
//! of the current route, which is what gives page transitions a direction.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::ev;
use leptos::portal::Portal;
use leptos::prelude::*;
use leptos::web_sys::wasm_bindgen::JsCast;
use leptos_router::components::A;

use crate::transitions::note_route_depth;

/// Pull distance (in CSS pixels) that arms the refresh gesture.
pub const PULL_THRESHOLD: f64 = 72.0;

/// Distance from the left edge (in CSS pixels) where a back-swipe may start.
pub const SWIPE_BACK_EDGE: f64 = 28.0;

/// Horizontal travel (in CSS pixels) that triggers the back-swipe.
pub const SWIPE_BACK_TRIGGER: f64 = 72.0;

/// Fraction of the sheet height that dismisses it when dragged down.
pub const SHEET_DISMISS_RATIO: f64 = 0.28;

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

/// Bottom-navigation entries declared once by the application.
///
/// Provided as a context in the app root, so pages only write
/// `<MobileLayout title="…">` and the shell picks the entries up:
///
/// ```ignore
/// provide_context(NavItems::new([
///     NavItem::new("Beranda", "/", "home"),
///     NavItem::new("Todo", "/todos", "check"),
/// ]));
/// ```
#[derive(Clone, Debug, Default)]
pub struct NavItems(pub Vec<NavItem>);

impl NavItems {
    /// Builds the context value.
    pub fn new(items: impl IntoIterator<Item = NavItem>) -> Self {
        Self(items.into_iter().collect())
    }
}

/// Inline SVG markup for the built-in icon set
/// (`home`, `grid`, `chart`, `check`, `plus`, `filter`, `sync`, `back`).
pub fn icon_svg(name: &str) -> String {
    let path = match name {
        "home" => "M4 11.2 12 4l8 7.2V20a1 1 0 0 1-1 1h-4.5v-5.5h-5V21H5a1 1 0 0 1-1-1z",
        "grid" => "M4 4h6.5v6.5H4zm9.5 0H20v6.5h-6.5zM4 13.5h6.5V20H4zm9.5 0H20V20h-6.5z",
        "chart" => "M4 20v-9h3.5v9zm6.2 0V4h3.5v16zm6.3 0v-6h3.5v6z",
        "check" => "M9.6 16.2 5.4 12l-1.4 1.4 5.6 5.6L20.4 8.2 19 6.8z",
        "plus" => "M11 4h2v7h7v2h-7v7h-2v-7H4v-2h7z",
        "filter" => "M4 5h16l-6.2 7.3V19l-3.6 2v-8.7z",
        "sync" => "M12 4V1L7.5 5.5 12 10V7a5 5 0 0 1 5 5h3a8 8 0 0 0-8-8zm-5 5H4a8 8 0 0 0 8 8v3l4.5-4.5L12 11v3a5 5 0 0 1-5-5z",
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
    /// Override the navigation entries for this page. Defaults to the
    /// [`NavItems`] provided at the app root.
    #[prop(optional)]
    nav: Option<Vec<NavItem>>,
    /// Trailing header actions (icon buttons, links).
    #[prop(optional)]
    actions: Option<AnyView>,
    children: Children,
) -> impl IntoView {
    let items = nav
        .or_else(|| use_context::<NavItems>().map(|items| items.0))
        .unwrap_or_default();
    let show_nav = show_nav && !items.is_empty();
    let title = title.unwrap_or_default();
    let has_title = !title.is_empty();

    // Arah animasi transisi halaman: komponen ini dibangun ulang setiap kali
    // rute berganti, dan pergantian itu terjadi di dalam update callback view
    // transition — jadi kedalaman halaman baru sudah dilaporkan sebelum browser
    // menyusun lapisan animasinya.
    let location = leptos_router::hooks::use_location();
    Effect::new(move |_| note_route_depth(&location.pathname.get()));

    // Edge swipe-back: only pages with a back button inherit the gesture, and it
    // only engages when the touch starts inside [`SWIPE_BACK_EDGE`] and moves
    // mostly horizontally — otherwise it would fight vertical scrolling and the
    // pull-to-refresh gesture that lives inside `<main>`.
    let drag_x = RwSignal::new(0.0f64);
    let gesture = RwSignal::new(None::<Gesture>);
    let sliding = Signal::derive(move || drag_x.get() > 0.0);

    let on_touch_start = move |event: ev::TouchEvent| {
        if !show_back {
            return;
        }
        let Some(touch) = event.touches().item(0) else {
            return;
        };
        let x = touch.client_x() as f64;
        gesture.set(
            (x <= SWIPE_BACK_EDGE).then(|| Gesture {
                x,
                y: touch.client_y() as f64,
                armed: false,
            }),
        );
    };
    let on_touch_move = move |event: ev::TouchEvent| {
        let Some(mut current) = gesture.get() else {
            return;
        };
        let Some(touch) = event.touches().item(0) else {
            return;
        };
        let dx = touch.client_x() as f64 - current.x;
        let dy = (touch.client_y() as f64 - current.y).abs();
        current.armed |= dx > 12.0 && dx > dy * 1.5;
        gesture.set(Some(current));
        drag_x.set(if current.armed {
            dx.max(0.0).min(viewport_width())
        } else {
            0.0
        });
    };
    let finish_gesture = move |_| {
        // `touchend` dan `touchcancel` bisa datang berurutan (mis. setelah gestur
        // sebelumnya dianggap masih aktif oleh browser). Gestur yang sudah selesai
        // tidak boleh memicu `history.back()` dua kali.
        let Some(_) = gesture.get_untracked() else {
            drag_x.set(0.0);
            return;
        };
        let travelled = drag_x.get_untracked();
        gesture.set(None);
        drag_x.set(0.0);
        if travelled >= SWIPE_BACK_TRIGGER {
            go_back();
        }
    };

    view! {
        <div
            class="sr-app"
            class:is-swiping-back=sliding
            style:transform=move || {
                let offset = drag_x.get();
                if offset > 0.0 {
                    format!("translateX({offset}px)")
                } else {
                    String::new()
                }
            }
            on:touchstart=on_touch_start
            on:touchmove=on_touch_move
            on:touchend=finish_gesture
            on:touchcancel=finish_gesture
        >
            {has_title
                .then(|| {
                    view! {
                        <HeaderBar title=title.clone() show_back=show_back>{actions}</HeaderBar>
                    }
                })}
            <OfflineBanner/>
            <main class="sr-main">{children()}</main>
            {show_nav.then(|| view! { <BottomNav items=items/> })}
        </div>
    }
}

/// In-flight edge swipe-back gesture.
#[derive(Clone, Copy)]
struct Gesture {
    x: f64,
    y: f64,
    armed: bool,
}

/// Pops the history stack.
///
/// `use_navigate` can only travel to a path — `"-1"` would be resolved as a
/// route — so back navigation goes through the History API; the Leptos router
/// follows along through its `popstate` listener.
pub fn go_back() {
    if let Some(window) = leptos::web_sys::window() {
        if let Ok(history) = window.history() {
            let _ = history.back();
        }
    }
}

/// Width of the layout viewport, used to clamp the swipe-back offset.
fn viewport_width() -> f64 {
    leptos::web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .unwrap_or(320.0)
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
    let title = title.unwrap_or_default();

    view! {
        <header class="sr-header">
            {show_back
                .then(|| {
                    view! {
                        <button
                            class="sr-header__back"
                            type="button"
                            aria-label="Kembali"
                            on:click=|_| go_back()
                            inner_html=icon_svg("back")
                        ></button>
                    }
                })}
            <h1 class="sr-header__title">{title}</h1>
            <div class="sr-header__actions">{children.map(|children| children())}</div>
        </header>
    }
}

/// Bottom navigation bar.
///
/// The active entry is marked by `aria-current="page"`, which
/// [`A`](leptos_router::components::A) sets on its own and CSS styles with
/// `.sr-nav a[aria-current="page"]`. Routing stays client-side because the links
/// are real router links.
#[component]
pub fn BottomNav(
    /// Navigation entries to render.
    #[prop(optional)]
    items: Option<Vec<NavItem>>,
) -> impl IntoView {
    let items = items.unwrap_or_default();

    let links = items
        .into_iter()
        .map(|item| {
            view! {
                <A href=item.href>
                    <span class="sr-nav__icon" inner_html=icon_svg(item.icon)></span>
                    <span class="sr-nav__label">{item.label}</span>
                </A>
            }
        })
        .collect_view();

    view! { <nav class="sr-nav" aria-label="Navigasi utama">{links}</nav> }
}

/// A button-styled router link.
///
/// Leptos' `<A>` component does not accept extra attributes (classes included), so
/// the button chrome lives on a wrapping element and CSS styles `.sr-btn > a`.
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
        Some("ghost") => "sr-btn sr-btn--ghost",
        _ => "sr-btn",
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
            class="sr-pull"
            class:is-armed=armed
            class:is-pulling=pulling
            on:touchstart=on_touch_start
            on:touchmove=on_touch_move
            on:touchend=finish
            on:touchcancel=finish
        >
            <div class="sr-pull__indicator" aria-hidden="true">
                <span class="sr-pull__spinner"></span>
                <span class="sr-pull__label">{label}</span>
            </div>
            <div
                class="sr-pull__content"
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

/// A drag-to-dismiss bottom sheet.
///
/// `open` is two-way: the sheet writes `false` when it dismisses itself (drag past
/// [`SHEET_DISMISS_RATIO`], backdrop tap, or `Escape`). It stays mounted while
/// closed so form state inside survives, but it is `visibility: hidden` — which
/// also keeps it out of the tab order and the accessibility tree.
///
/// The markup is teleported to `document.body`, so the overlay cannot be trapped
/// by a stacking context created further up the page — `view-transition-name` on
/// the page container does exactly that, and a fixed sheet nested inside it would
/// paint below the header and the bottom nav.
#[component]
pub fn BottomSheet(
    /// Visibility of the sheet.
    open: RwSignal<bool>,
    /// Heading shown above the content.
    #[prop(into, optional)]
    title: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let panel = NodeRef::<leptos::html::Div>::new();
    // Elemen pemicu disimpan di luar sistem sinyal: `HtmlElement` bukan `Send`,
    // dan sinyal lokal (`new_local`) akan panik bila owner-nya dibersihkan dari
    // thread lain — yang persis terjadi pada render SSR di dalam runtime Axum.
    let restore_focus: Rc<RefCell<Option<leptos::web_sys::HtmlElement>>> =
        Rc::new(RefCell::new(None));
    // Fokus hanya dipindahkan sekali per pembukaan; node portal bisa muncul belakangan.
    let focused = RwSignal::new(false);
    let drag = RwSignal::new(0.0f64);
    let touch_start = RwSignal::new(None::<f64>);
    let dragging = Signal::derive(move || touch_start.get().is_some());
    let title = title.unwrap_or_default();
    let has_title = !title.is_empty();

    let on_touch_start = move |event: ev::TouchEvent| {
        let Some(touch) = event.touches().item(0) else {
            return;
        };
        touch_start.set(Some(touch.client_y() as f64));
    };
    let on_touch_move = move |event: ev::TouchEvent| {
        let Some(start) = touch_start.get() else {
            return;
        };
        let Some(touch) = event.touches().item(0) else {
            return;
        };
        let delta = touch.client_y() as f64 - start;
        // Downward travel follows the finger; upward gets resistance so the sheet
        // does not detach from the top edge when a scroll overshoots.
        drag.set(if delta > 0.0 { delta } else { delta * 0.2 });
    };
    let settle = move |_| {
        // Idempoten: `touchend` bisa disusul `touchcancel`.
        let Some(start) = touch_start.get_untracked() else {
            drag.set(0.0);
            return;
        };
        let _ = start;
        let travelled = drag.get_untracked();
        let height = panel
            .get_untracked()
            .map(|panel| panel.client_height() as f64)
            .unwrap_or(0.0);
        touch_start.set(None);
        drag.set(0.0);
        if travelled > 0.0 && travelled >= height * SHEET_DISMISS_RATIO {
            open.set(false);
        }
    };

    // While open: block background scrolling and move focus into the sheet; on
    // close, hand focus back to whatever opened it. The panel node is read as a
    // dependency because the portal mounts the markup *after* the first render,
    // so `open` can turn true while the node does not exist yet.
    Effect::new(move |_| {
        let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let node = panel.get();
        if open.get() {
            set_body_scroll(&document, false);
            if !focused.get_untracked() {
                if let Some(node) = node {
                    *restore_focus.borrow_mut() =
                        document.active_element().and_then(|element| element.dyn_into().ok());
                    let _ = node.focus();
                    focused.set(true);
                }
            }
        } else {
            set_body_scroll(&document, true);
            if focused.get_untracked() {
                focused.set(false);
                if let Some(element) = restore_focus.borrow().as_ref() {
                    let _ = element.focus();
                }
            }
        }
    });

    let on_keydown = move |event: ev::KeyboardEvent| {
        if event.key() == "Escape" {
            open.set(false);
        }
    };
    let close = move |_| open.set(false);

    // Teleport ke `document.body`: lihat catatan pada [`BottomSheet`].
    let sheet = move || {
        view! {
            <div
                class="sr-sheet"
                class:is-open=move || open.get()
                aria-hidden=move || (!open.get()).then_some("true")
            >
                <div class="sr-sheet__backdrop" on:click=close></div>
                <div
                    class="sr-sheet__panel"
                    class:is-dragging=dragging
                    node_ref=panel
                    tabindex="-1"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby=has_title.then_some("sr-sheet-title")
                    style:transform=move || {
                        let offset = drag.get();
                        (offset > 0.0).then(|| format!("translateY({offset}px)")).unwrap_or_default()
                    }
                    on:touchstart=on_touch_start
                    on:touchmove=on_touch_move
                    on:touchend=settle
                    on:touchcancel=settle
                    on:keydown=on_keydown
                >
                    <div class="sr-sheet__handle" aria-hidden="true"></div>
                    {has_title
                        .then(|| view! { <h2 class="sr-sheet__title" id="sr-sheet-title">{title.clone()}</h2> })}
                    <div class="sr-sheet__body">{children()}</div>
                </div>
            </div>
        }
    };

    view! { <Portal>{sheet()}</Portal> }
}

/// Locks or releases background scrolling while a sheet is open.
fn set_body_scroll(document: &leptos::web_sys::Document, enabled: bool) {
    let Some(body) = document.body() else {
        return;
    };
    let style = body.style();
    if enabled {
        let _ = style.remove_property("overflow");
    } else {
        let _ = style.set_property("overflow", "hidden");
    }
}

/// Banner that CSS reveals while `navigator.onLine` is `false`.
#[component]
fn OfflineBanner() -> impl IntoView {
    view! {
        <div class="sr-offline-banner" role="status">
            <span class="sr-offline-banner__dot"></span>
            "Mode offline — data diambil dari cache."
        </div>
    }
}
