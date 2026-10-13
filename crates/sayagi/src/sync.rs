//! Realtime sync client for the Wasm side: one WebSocket, automatic reconnect.
//!
//! The wire format belongs to the application — this client only keeps a socket
//! alive, reports its state, and hands every inbound message to a callback. That
//! keeps the framework free of application types while the app decides what a
//! payload means.
//!
//! Reconnection uses capped exponential backoff (0.5s → 30s) and is also
//! triggered by the browser's `online` event, so a phone coming out of a tunnel
//! resyncs without user action. A `subscribe` message is sent on every (re)open,
//! which is what makes the server reply with a fresh snapshot.

use leptos::prelude::*;

/// Connection state, ready to be rendered by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Socket is being opened (or reopened).
    Connecting,
    /// Socket is open; events flow.
    Online,
    /// Socket is closed; a retry is scheduled.
    Offline,
}

impl ConnectionState {
    /// Parses the wire value sent by the JS side.
    pub fn from_wire(value: &str) -> Self {
        match value {
            "online" => Self::Online,
            "offline" => Self::Offline,
            _ => Self::Connecting,
        }
    }

    /// Human-readable label (Indonesian, matching the app copy).
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Menyambungkan…",
            Self::Online => "Tersinkron",
            Self::Offline => "Terputus",
        }
    }
}

/// WebSocket URL for `path` on the current origin (`ws:` / `wss:` aware).
///
/// Returns an empty string outside the browser, so callers can skip connecting.
pub fn websocket_url(path: &str) -> String {
    let Some(window) = leptos::web_sys::window() else {
        return String::new();
    };
    let location = window.location();
    let secure = location
        .protocol()
        .map(|protocol| protocol == "https:")
        .unwrap_or(false);
    let scheme = if secure { "wss" } else { "ws" };
    format!("{scheme}://{}{path}", location.host().unwrap_or_default())
}

/// Handle of the app-wide sync connection.
#[derive(Clone, Copy)]
pub struct SyncClient {
    state: RwSignal<ConnectionState>,
}

/// Connects to `url` (e.g. `ws://127.0.0.1:3000/api/ws`).
///
/// One connection per app: a second call replaces the first. `on_message`
/// receives every inbound payload verbatim.
pub fn connect(url: &str, on_message: Callback<String>) -> SyncClient {
    let state = RwSignal::new(ConnectionState::Connecting);

    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        let on_state = Closure::<dyn FnMut(String)>::new(move |value: String| {
            state.set(ConnectionState::from_wire(&value));
        });
        let on_event = Closure::<dyn FnMut(String)>::new(move |payload: String| {
            on_message.run(payload);
        });

        connect_js(
            url,
            on_state.as_ref().unchecked_ref(),
            on_event.as_ref().unchecked_ref(),
        );

        // The socket outlives this call, so its callbacks must too; they are
        // released when the page goes away.
        on_state.into_js_value();
        on_event.into_js_value();
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (url, on_message);
    }

    SyncClient { state }
}

impl SyncClient {
    /// Reactive connection state.
    pub fn state(&self) -> Signal<ConnectionState> {
        self.state.into()
    }

    /// Sends a raw payload, e.g. `{"type":"subscribe"}` to force a snapshot.
    pub fn send(&self, payload: &str) {
        #[cfg(feature = "hydrate")]
        send_js(payload);

        #[cfg(not(feature = "hydrate"))]
        let _ = payload;
    }

    /// Closes the socket and stops reconnecting.
    pub fn close(&self) {
        #[cfg(feature = "hydrate")]
        close_js();
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
let current = null;

function create(url, onState, onMessage) {
  let socket = null;
  let attempt = 0;
  let ping = null;
  let retry = null;
  let closed = false;

  const clearTimers = () => {
    if (ping !== null) { clearInterval(ping); ping = null; }
    if (retry !== null) { clearTimeout(retry); retry = null; }
  };

  const scheduleRetry = () => {
    if (closed || retry !== null) return;
    const delay = Math.min(30000, 500 * 2 ** attempt++);
    retry = setTimeout(() => { retry = null; open(); }, delay);
  };

  function open() {
    if (closed) return;
    onState("connecting");
    try {
      socket = new WebSocket(url);
    } catch (error) {
      onState("offline");
      scheduleRetry();
      return;
    }
    socket.onopen = () => {
      attempt = 0;
      onState("online");
      // Selalu minta snapshot: setelah reconnect, keadaan lokal bisa tertinggal.
      socket.send(JSON.stringify({ type: "subscribe" }));
      ping = setInterval(() => {
        try { socket.send(JSON.stringify({ type: "ping" })); } catch (error) { /* ditutup */ }
      }, 25000);
    };
    socket.onmessage = (event) => {
      if (typeof event.data === "string") onMessage(event.data);
    };
    socket.onerror = () => {
      try { socket.close(); } catch (error) { /* diabaikan */ }
    };
    socket.onclose = () => {
      clearTimers();
      onState("offline");
      scheduleRetry();
    };
  }

  window.addEventListener("online", () => {
    if (closed) return;
    if (!socket || socket.readyState > WebSocket.OPEN) open();
  });
  window.addEventListener("offline", () => onState("offline"));

  open();

  return {
    send(payload) {
      if (socket && socket.readyState === WebSocket.OPEN) socket.send(payload);
    },
    close() {
      closed = true;
      clearTimers();
      if (socket) { try { socket.close(); } catch (error) { /* diabaikan */ } }
      socket = null;
    },
  };
}

export function __sayagi_sync_connect(url, onState, onMessage) {
  if (current) current.close();
  current = create(url, onState, onMessage);
}

export function __sayagi_sync_send(payload) {
  if (current) current.send(payload);
}

export function __sayagi_sync_close() {
  if (current) { current.close(); current = null; }
}
"#)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __sayagi_sync_connect)]
    fn connect_js(url: &str, on_state: &js_sys::Function, on_message: &js_sys::Function);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __sayagi_sync_send)]
    fn send_js(payload: &str);

    #[wasm_bindgen::prelude::wasm_bindgen(js_name = __sayagi_sync_close)]
    fn close_js();
}
