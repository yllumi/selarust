//! PWA wiring for the Wasm client: service worker registration and connectivity state.
//!
//! Both helpers are no-ops on the server, so app code can call them unconditionally.

/// Registers the generated service worker at `path` (usually `/sw.js`).
///
/// The worker itself is produced by `sayagi-build` from the settings in `Sayagi.toml`.
pub fn register_service_worker(path: &str) {
    #[cfg(feature = "hydrate")]
    {
        register_service_worker_js(path);
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = path;
    }
}

/// Mirrors `navigator.onLine` onto `<html data-sayagi-online="…">` and keeps it updated.
///
/// CSS can react to it directly, so an offline banner needs no Rust state:
///
/// ```css
/// html[data-sayagi-online="false"] .sy-offline-banner { display: flex; }
/// ```
pub fn watch_connectivity() {
    #[cfg(feature = "hydrate")]
    {
        watch_connectivity_js();
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function __sayagi_register_service_worker(path) {
  if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
    return;
  }
  const register = () => {
    navigator.serviceWorker.register(path, { scope: "/" }).catch((error) => {
      console.warn("[sayagi] registrasi service worker gagal:", error);
    });
  };
  if (document.readyState === "complete") {
    register();
  } else {
    window.addEventListener("load", register, { once: true });
  }
}

export function __sayagi_watch_connectivity() {
  if (typeof navigator === "undefined") {
    return;
  }
  const root = document.documentElement;
  const sync = () => {
    root.dataset.sayagiOnline = navigator.onLine ? "true" : "false";
    root.dispatchEvent(
      new CustomEvent("sayagi:connectivity", { detail: { online: navigator.onLine } })
    );
  };
  window.addEventListener("online", sync);
  window.addEventListener("offline", sync);
  sync();
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __sayagi_register_service_worker)]
    fn register_service_worker_js(path: &str);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __sayagi_watch_connectivity)]
    fn watch_connectivity_js();
}
