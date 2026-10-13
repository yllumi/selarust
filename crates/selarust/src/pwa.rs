//! PWA wiring for the Wasm client: service worker registration and connectivity state.
//!
//! Both helpers are no-ops on the server, so app code can call them unconditionally.

/// Registers the generated service worker at `path` (usually `/sw.js`).
///
/// The worker itself is produced by `selarust-build` from the settings in `Selarust.toml`.
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

/// Unregisters every service worker of this origin and drops all `selarust-*` caches.
///
/// Dev builds call this so a worker left behind by an earlier release build cannot
/// serve cached HTML to `cargo leptos watch`. It is a no-op on the server.
pub fn reset_service_worker() {
    #[cfg(feature = "hydrate")]
    {
        reset_service_worker_js();
    }
}

/// Whether the browser reports a connection right now.
///
/// `navigator.onLine` only says "there is a network interface", not "the server
/// answers", so callers treat `true` as "try it" rather than a guarantee.
pub fn is_online() -> bool {
    #[cfg(feature = "hydrate")]
    {
        leptos::web_sys::window()
            .and_then(|window| window.navigator().on_line().into())
            .unwrap_or(true)
    }

    #[cfg(not(feature = "hydrate"))]
    {
        true
    }
}

/// Mirrors `navigator.onLine` onto `<html data-selarust-online="…">` and keeps it updated.
///
/// CSS can react to it directly, so an offline banner needs no Rust state:
///
/// ```css
/// html[data-selarust-online="false"] .sr-offline-banner { display: flex; }
/// ```
pub fn watch_connectivity() {
    #[cfg(feature = "hydrate")]
    {
        watch_connectivity_js();
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function __selarust_register_service_worker(path) {
  if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
    return;
  }
  // Navigasi antar rute dilakukan router di sisi klien, jadi dokumen rute itu
  // tidak pernah diminta lewat jaringan dan tidak akan masuk cache runtime
  // dengan sendirinya. Tanpa langkah ini, reload offline pada rute yang sudah
  // pernah dibuka akan jatuh ke halaman offline.
  const warm = (registration) => {
    const worker = navigator.serviceWorker.controller || registration.active;
    if (!worker) {
      return;
    }
    worker.postMessage({ type: "selarust:warm-document", url: location.pathname + location.search });
  };

  const register = () => {
    navigator.serviceWorker
      .register(path, { scope: "/" })
      .then((registration) => {
        warm(registration);
        navigator.serviceWorker.addEventListener("controllerchange", () => warm(registration));
      })
      .catch((error) => {
        console.warn("[selarust] registrasi service worker gagal:", error);
      });
  };
  if (document.readyState === "complete") {
    register();
  } else {
    window.addEventListener("load", register, { once: true });
  }
}

export async function __selarust_reset_service_worker() {
  if (typeof navigator === "undefined" || !("serviceWorker" in navigator)) {
    return;
  }
  const registrations = await navigator.serviceWorker.getRegistrations();
  await Promise.all(registrations.map((registration) => registration.unregister()));
  if (typeof caches !== "undefined") {
    // "selarust-" covers our own caches, "workbox-" the ones Workbox manages.
    const owned = (key) => key.startsWith("selarust-") || key.startsWith("workbox-");
    const keys = await caches.keys();
    await Promise.all(keys.filter(owned).map((key) => caches.delete(key)));
  }
  if (registrations.length > 0) {
    console.info("[selarust] service worker dibersihkan untuk mode dev");
  }
}

export function __selarust_watch_connectivity() {
  if (typeof navigator === "undefined") {
    return;
  }
  const root = document.documentElement;
  const sync = () => {
    root.dataset.selarustOnline = navigator.onLine ? "true" : "false";
    root.dispatchEvent(
      new CustomEvent("selarust:connectivity", { detail: { online: navigator.onLine } })
    );
  };
  window.addEventListener("online", sync);
  window.addEventListener("offline", sync);
  sync();
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_register_service_worker)]
    fn register_service_worker_js(path: &str);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_reset_service_worker)]
    fn reset_service_worker_js();

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_watch_connectivity)]
    fn watch_connectivity_js();
}
