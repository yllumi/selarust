//! Offline storage: an IndexedDB key/value store plus a FIFO mutation queue.
//!
//! The framework stores opaque strings — the application decides what a snapshot
//! or a queued mutation contains, and owns the replay rules. IndexedDB is the
//! right home for this instead of `localStorage`: it is asynchronous (so it never
//! blocks the UI thread), it stores structured data, and quota is measured in
//! hundreds of MB rather than 5 MB.
//!
//! On the server every function is a no-op, so app code can call it without
//! `cfg` gymnastics.

use serde::{Deserialize, Serialize};

/// One queued mutation, in the order it was performed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueuedMutation<T> {
    /// Queue key assigned by IndexedDB, used to delete the entry after replay.
    pub key: f64,
    /// Application payload.
    pub payload: T,
}

/// Reads a snapshot by key. `None` when nothing was stored yet.
pub async fn snapshot(key: &str) -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        let value = kv_get_js(key).await.ok()?;
        if value.is_undefined() || value.is_null() {
            return None;
        }
        value.as_string()
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = key;
        None
    }
}

/// Stores a snapshot by key.
pub async fn store_snapshot(key: &str, value: &str) -> Result<(), String> {
    #[cfg(feature = "hydrate")]
    {
        kv_put_js(key, value).await.map_err(describe)?;
        Ok(())
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (key, value);
        Ok(())
    }
}

/// Appends a mutation to the queue, returning its queue key.
pub async fn queue_push<T: Serialize>(payload: &T) -> Result<f64, String> {
    #[cfg(feature = "hydrate")]
    {
        let encoded =
            serde_json::to_string(payload).map_err(|error| format!("serialize: {error}"))?;
        let key = queue_add_js(&encoded).await.map_err(describe)?;
        Ok(key.as_f64().unwrap_or_default())
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = payload;
        Err("antrean offline hanya tersedia di klien".to_string())
    }
}

/// Every queued mutation, oldest first.
pub async fn queue_all<T: for<'de> Deserialize<'de>>() -> Result<Vec<QueuedMutation<T>>, String> {
    #[cfg(feature = "hydrate")]
    {
        let entries = queue_all_js().await.map_err(describe)?;
        let mut out = Vec::new();
        let mut index = 0;
        while let Some(pair) = js_sys::Reflect::get_u32(&entries, index)
            .ok()
            .map(wasm_bindgen::JsValue::from)
            .and_then(|pair| pair.dyn_into::<js_sys::Array>().ok())
        {
            let key = pair.get(0).as_f64().unwrap_or_default();
            let value = pair.get(1).as_string().unwrap_or_default();
            let payload = serde_json::from_str(&value)
                .map_err(|error| format!("antrean rusak pada key {key}: {error}"))?;
            out.push(QueuedMutation { key, payload });
            index += 1;
        }
        Ok(out)
    }

    #[cfg(not(feature = "hydrate"))]
    {
        Ok(Vec::new())
    }
}

/// Drops one queue entry (after a successful replay).
pub async fn queue_remove(key: f64) -> Result<(), String> {
    #[cfg(feature = "hydrate")]
    {
        queue_delete_js(key).await.map_err(describe)?;
        Ok(())
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = key;
        Ok(())
    }
}

/// Empties the queue.
pub async fn queue_clear() -> Result<(), String> {
    #[cfg(feature = "hydrate")]
    {
        queue_clear_js().await.map_err(describe)?;
        Ok(())
    }

    #[cfg(not(feature = "hydrate"))]
    {
        Ok(())
    }
}

#[cfg(feature = "hydrate")]
use wasm_bindgen::JsCast;

#[cfg(feature = "hydrate")]
fn describe(error: wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("{error:?}"))
}

#[cfg(feature = "hydrate")]
async fn kv_put_js(key: &str, value: &str) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_bindgen_futures::JsFuture::from(kv_put(key, value)).await
}

#[cfg(feature = "hydrate")]
async fn kv_get_js(key: &str) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_bindgen_futures::JsFuture::from(kv_get(key)).await
}

#[cfg(feature = "hydrate")]
async fn queue_add_js(value: &str) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_bindgen_futures::JsFuture::from(queue_add(value)).await
}

#[cfg(feature = "hydrate")]
async fn queue_all_js() -> Result<js_sys::Array, wasm_bindgen::JsValue> {
    let entries = wasm_bindgen_futures::JsFuture::from(queue_all_entries()).await?;
    entries
        .dyn_into::<js_sys::Array>()
        .map_err(|value| value)
}

#[cfg(feature = "hydrate")]
async fn queue_delete_js(key: f64) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_bindgen_futures::JsFuture::from(queue_delete(key)).await
}

#[cfg(feature = "hydrate")]
async fn queue_clear_js() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    wasm_bindgen_futures::JsFuture::from(queue_clear_all()).await
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const DB_NAME = "selarust-offline";
const DB_VERSION = 1;

function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("kv")) db.createObjectStore("kv");
      if (!db.objectStoreNames.contains("queue")) {
        db.createObjectStore("queue", { autoIncrement: true });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

// One transaction per call: IndexedDB auto-commits when the microtask queue
// drains, so keeping transactions short is what keeps writes cheap.
function run(store, mode, action) {
  return openDb().then((db) => new Promise((resolve, reject) => {
    const transaction = db.transaction(store, mode);
    let result;
    const finish = () => { db.close(); resolve(result); };
    transaction.oncomplete = finish;
    transaction.onerror = () => { db.close(); reject(transaction.error); };
    transaction.onabort = () => { db.close(); reject(transaction.error); };
    try {
      result = action(transaction.objectStore(store));
    } catch (error) {
      db.close();
      reject(error);
    }
  }));
}

export function __selarust_offline_kv_put(key, value) {
  return run("kv", "readwrite", (store) => { store.put(value, key); });
}

export function __selarust_offline_kv_get(key) {
  return run("kv", "readonly", (store) => store.get(key));
}

export function __selarust_offline_queue_add(value) {
  let key;
  return run("queue", "readwrite", (store) => {
    const request = store.add(value);
    request.onsuccess = () => { key = request.result; };
  }).then(() => key);
}

export function __selarust_offline_queue_all() {
  return new Promise((resolve, reject) => {
    openDb().then((db) => {
      const entries = [];
      const transaction = db.transaction("queue", "readonly");
      const cursor = transaction.objectStore("queue").openCursor();
      cursor.onsuccess = () => {
        const current = cursor.result;
        if (current) {
          entries.push([current.key, current.value]);
          current.continue();
        }
      };
      transaction.oncomplete = () => { db.close(); resolve(entries); };
      transaction.onerror = () => { db.close(); reject(transaction.error); };
    }).catch(reject);
  });
}

export function __selarust_offline_queue_delete(key) {
  return run("queue", "readwrite", (store) => { store.delete(key); });
}

export function __selarust_offline_queue_clear() {
  return run("queue", "readwrite", (store) => { store.clear(); });
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_kv_put)]
    fn kv_put(key: &str, value: &str) -> js_sys::Promise;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_kv_get)]
    fn kv_get(key: &str) -> js_sys::Promise;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_queue_add)]
    fn queue_add(value: &str) -> js_sys::Promise;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_queue_all)]
    fn queue_all_entries() -> js_sys::Promise;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_queue_delete)]
    fn queue_delete(key: f64) -> js_sys::Promise;

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __selarust_offline_queue_clear)]
    fn queue_clear_all() -> js_sys::Promise;
}
