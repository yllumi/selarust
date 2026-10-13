//! Klien realtime: menyambung ke `/api/ws`, menerapkan pesan ke [`TodoStore`].
//!
//! Pesan yang mungkin diterima (lihat [`crate::sync`] di sisi server):
//!
//! * `hello` — versi server, hanya untuk debugging.
//! * `snapshot` — keadaan penuh; menggantikan daftar lokal.
//! * `todo` — satu mutasi; diterapkan tanpa refetch.
//! * `pong` — jawaban ping, tidak mengubah apa pun.

use serde::Deserialize;

use crate::api::{Todo, TodoEvent};
use crate::store::TodoStore;

#[cfg(feature = "hydrate")]
use leptos::prelude::*;
#[cfg(feature = "hydrate")]
use selarust::sync;

/// Pesan dari server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Sapaan awal saat koneksi terbuka.
    Hello {
        /// Versi biner server.
        version: String,
    },
    /// Keadaan seluruh todo saat `subscribe`.
    Snapshot {
        /// Daftar todo terbaru.
        todos: Vec<Todo>,
    },
    /// Satu mutasi todo.
    Todo {
        /// Perubahan yang harus diterapkan.
        event: TodoEvent,
    },
    /// Jawaban `ping`.
    Pong,
    /// Jawaban teks bebas.
    Echo {
        /// Teks yang dikirim klien.
        echo: String,
    },
}

/// Menyambungkan kanal sinkronisasi dan memasang efek-efeknya.
///
/// Mengembalikan `None` di server (tidak ada WebSocket saat SSR).
pub fn start(store: TodoStore) -> Option<selarust::SyncClient> {
    #[cfg(feature = "hydrate")]
    {
        // Snapshot lokal dipakai lebih dulu supaya daftar langsung terlihat,
        // lalu server menyusul dan antrean offline diputar ulang.
        leptos::task::spawn_local(async move {
            store.load().await;
            store.refresh().await;
            store.flush().await;
        });

        let url = sync::websocket_url("/api/ws");
        if url.is_empty() {
            return None;
        }

        let client = sync::connect(
            &url,
            Callback::new(move |payload: String| handle(store, &payload)),
        );

        // Begitu socket tersambung lagi, kirim antrean yang tertinggal.
        Effect::new(move |_| {
            if client.state().get() == sync::ConnectionState::Online {
                leptos::task::spawn_local(async move { store.flush().await });
            }
        });

        Some(client)
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = store;
        None
    }
}

/// Menerapkan satu payload JSON ke store.
pub fn handle(store: TodoStore, payload: &str) {
    match serde_json::from_str::<ServerMessage>(payload) {
        Ok(ServerMessage::Snapshot { todos }) => store.replace(todos),
        Ok(ServerMessage::Todo { event }) => store.apply(&event),
        Ok(ServerMessage::Hello { version }) => {
            leptos::logging::log!("[selarust] sync tersambung ke server {version}");
        }
        Ok(ServerMessage::Pong) | Ok(ServerMessage::Echo { .. }) => {}
        Err(error) => leptos::logging::warn!("[selarust] pesan sync tidak dikenal: {error}"),
    }
}
