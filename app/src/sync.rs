//! Kanal sinkronisasi realtime: satu broadcast channel, satu endpoint WebSocket.
//!
//! Protokolnya sengaja kecil dan JSON-based supaya bisa dipakai klien Wasm
//! (Rust) sekaligus diuji dengan `websocat`:
//!
//! ```text
//! klien -> server : teks apa pun            => dibalas {"type":"echo", ...}
//! klien -> server : {"type":"ping"}         => {"type":"pong"}
//! klien -> server : {"type":"subscribe"}    => {"type":"snapshot","todos":[…]}
//! server -> klien : {"type":"todo","event":{…}} saat ada mutasi
//! ```
//!
//! Setiap klien yang terhubung menerima semua mutasi todo, sehingga dua tab
//! (atau dua perangkat) tetap sinkron tanpa polling.

use serde::Serialize;
use serde_json::{json, Value};

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::broadcast;

use crate::api::{Todo, TodoEvent};
use crate::db;

/// Pesan yang dikirim server ke klien.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Dikirim sekali saat klien terhubung.
    Hello {
        /// Versi biner server, berguna saat debugging lintas build.
        version: String,
    },
    /// Keadaan seluruh todo saat ini (dikirim saat `subscribe`).
    Snapshot { todos: Vec<Todo> },
    /// Satu mutasi todo.
    Todo { event: TodoEvent },
    /// Jawaban `ping`.
    Pong,
    /// Jawaban teks bebas (perilaku lama `/api/ws`).
    Echo { echo: String },
}

/// Menyiarkan perubahan ke semua klien WebSocket.
#[derive(Clone)]
pub struct SyncHub {
    sender: broadcast::Sender<TodoEvent>,
}

impl SyncHub {
    /// Buffer 128 event: cukup untuk burst singkat tanpa menahan pengirim.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(128);
        Self { sender }
    }

    /// Tidak gagal walau belum ada klien yang mendengarkan.
    pub fn publish(&self, event: TodoEvent) {
        let _ = self.sender.send(event);
    }

    fn subscribe(&self) -> broadcast::Receiver<TodoEvent> {
        self.sender.subscribe()
    }
}

impl Default for SyncHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Mengelola satu koneksi WebSocket: snapshot awal, lalu event dan pesan masuk.
pub async fn handle_socket(mut socket: WebSocket, hub: SyncHub, pool: db::Pool) {
    let mut events = hub.subscribe();

    if send(&mut socket, &ServerMessage::Hello { version: env!("CARGO_PKG_VERSION").to_string() })
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if handle_client_message(&mut socket, &pool, text.as_str()).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if socket.send(Message::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if send(&mut socket, &ServerMessage::Todo { event }).await.is_err() {
                            break;
                        }
                    }
                    // Lagging only skips missed events: the client asks for a
                    // fresh snapshot when it reconnects.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn handle_client_message(
    socket: &mut WebSocket,
    pool: &db::Pool,
    text: &str,
) -> Result<(), axum::Error> {
    let parsed = serde_json::from_str::<Value>(text).ok();
    let kind = parsed
        .as_ref()
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str);

    match kind {
        Some("ping") => send(socket, &ServerMessage::Pong).await,
        Some("subscribe") => {
            let todos = db::list_todos(pool).await.unwrap_or_default();
            send(socket, &ServerMessage::Snapshot { todos }).await
        }
        _ => send(socket, &ServerMessage::Echo { echo: text.to_string() }).await,
    }
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let payload = json!(message).to_string();
    socket.send(Message::Text(payload.into())).await
}
