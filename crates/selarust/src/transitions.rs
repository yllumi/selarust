//! Page-transition direction: which animation a navigation deserves.
//!
//! A navigation is classified by how the *depth* of the route changes, not by
//! browser history direction: going deeper slides left (the new page enters from
//! the right), going back up to a parent slides right, and moving sideways to a
//! page at the same depth just fades. History direction alone reads every
//! backward move as "up" and every sibling move as "forward", which is what makes
//! the animation ambiguous.
//!
//! [`MobileLayout`](crate::MobileLayout) reports the depth of the page it renders
//! through [`note_route_depth`]. That component is rebuilt *inside* the view
//! transition's update callback — the router swaps the route tree there — so the
//! class is on `<html>` before the browser builds the animation layers, and no
//! guessing about the target URL is needed (the router pushes the new URL only
//! after the transition has started). Two shells can briefly be alive while the
//! route swaps, so a repeat report for the *same* path is ignored: it is not a
//! navigation and must not overwrite the direction that was just decided.
//!
//! The depth rule itself is Rust ([`route_depth`]) so it stays unit-testable; the
//! Wasm side only keeps a running "previous depth" and the class name.

/// Class set on `<html>` while moving into a deeper route (slide left).
pub const TRANSITION_INTO_CHILD: &str = "sr-nav-child";
/// Class set on `<html>` while moving back to a shallower route (slide right).
pub const TRANSITION_INTO_PARENT: &str = "sr-nav-parent";
/// Class set on `<html>` while moving to a route at the same depth (fade).
pub const TRANSITION_SAME_LEVEL: &str = "sr-nav-level";

/// Every class the helper manages; exactly one is set after a navigation.
pub const TRANSITION_CLASSES: [&str; 3] = [
    TRANSITION_INTO_CHILD,
    TRANSITION_INTO_PARENT,
    TRANSITION_SAME_LEVEL,
];

/// Number of path segments, i.e. how deep a route sits in the tree.
///
/// `/` is depth 0, `/todos` is 1, `/todos/42` is 2. Trailing slashes, empty
/// segments and query/fragment suffixes are ignored, so `/todos/` and
/// `/todos?filter=open` are both level 1.
pub fn route_depth(path: &str) -> usize {
    let without_query = path.split(['?', '#']).next().unwrap_or(path);
    without_query
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

/// Reports the depth of the page that just became visible.
///
/// Called by [`MobileLayout`](crate::MobileLayout) on every render, which happens
/// once per navigation, including the initial load (ignored, there is no
/// transition to give a direction to). No-op on the server.
pub fn note_route_depth(path: &str) {
    #[cfg(feature = "hydrate")]
    {
        note_route_depth_js(path, route_depth(path) as f64);
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = path;
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const root = document.documentElement;
// Tanpa View Transitions API, halaman lama sudah dilepas router; CSS memakai
// kelas ini untuk tetap menganimasikan halaman baru sesuai arahnya.
if (typeof document.startViewTransition !== "function") {
  root.classList.add("sr-no-vt");
}

let previous = null;
let timer = null;

export function __selarust_note_route_depth(path, depth) {
  const clear = () => root.classList.remove("sr-nav-child", "sr-nav-parent", "sr-nav-level");

  if (previous === null) {
    // Pemuatan pertama: tidak ada halaman sebelumnya, jadi tidak ada arah.
    previous = { path, depth };
    clear();
    return;
  }

  if (previous.path === path) {
    // Rute yang sama dilaporkan lebih dari sekali (dua shell sempat hidup
    // bersamaan saat rute diganti); bukan navigasi baru, jadi kelas arah yang
    // sudah dipasang tidak boleh ditimpa.
    return;
  }

  const from = previous.depth;
  previous = { path, depth };

  clear();
  root.classList.add(depth > from ? "sr-nav-child" : depth < from ? "sr-nav-parent" : "sr-nav-level");

  // Kelas hanya perlu hidup sampai view transition membentuk lapisan animasinya
  // (animasi 0.26 s); sesudah itu dibersihkan supaya tidak terbawa navigasi lain.
  if (timer !== null) {
    clearTimeout(timer);
  }
  timer = setTimeout(() => {
    timer = null;
    clear();
  }, 600);
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_note_route_depth)]
    fn note_route_depth_js(path: &str, depth: f64);
}

#[cfg(test)]
mod tests {
    use super::{route_depth, TRANSITION_CLASSES};

    #[test]
    fn depth_counts_path_segments() {
        assert_eq!(route_depth("/"), 0);
        assert_eq!(route_depth(""), 0);
        assert_eq!(route_depth("/todos"), 1);
        assert_eq!(route_depth("/todos/42"), 2);
        assert_eq!(route_depth("/files/a/b/c"), 4);
    }

    #[test]
    fn trailing_slashes_and_queries_do_not_change_the_level() {
        assert_eq!(route_depth("/todos/"), 1);
        assert_eq!(route_depth("/todos//"), 1);
        assert_eq!(route_depth("/todos?filter=open"), 1);
        assert_eq!(route_depth("/todos/42#catatan"), 2);
    }

    #[test]
    fn sibling_moves_stay_at_the_same_level() {
        assert_eq!(route_depth("/todos"), route_depth("/dashboard"));
        assert!(route_depth("/todos/42") > route_depth("/todos"));
        assert!(route_depth("/todos") > route_depth("/"));
        assert_eq!(TRANSITION_CLASSES.len(), 3);
    }
}
