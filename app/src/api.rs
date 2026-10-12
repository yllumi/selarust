//! Server functions: dipanggil dari Wasm layaknya fungsi lokal, dieksekusi di Axum.
//!
//! Setiap fungsi beranotasi `#[server]` otomatis mendapat endpoint HTTP di bawah
//! `/api/` — namanya `<nama_fungsi>` ditambah hash kecil dari lokasi sumber, mis.
//! `/api/get_containers4286897385061139490`. Itulah sebabnya `Sayagi.toml`
//! menaruh `/api/` di `network_first` untuk service worker, bukan daftar URL
//! spesifik yang harus diperbarui setiap kali ada fungsi baru.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Satu baris tabel `containers`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Container {
    /// Primary key.
    pub id: i64,
    /// Nama container.
    pub name: String,
    /// `running`, `idle`, atau `stopped`.
    pub status: String,
}

/// Ringkasan status seluruh container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    /// Total container terdaftar.
    pub total: i64,
    /// Container dengan status `running`.
    pub running: i64,
    /// Container yang tidak berjalan.
    pub stopped: i64,
}

/// Mengambil seluruh container, terbaru lebih dulu.
///
/// Bentuk `#[server(Nama, "/api")]` mengikuti ARCHITECTURE.md §3B: argumen kedua
/// menetapkan prefix URL, sehingga endpoint-nya berada di bawah `/api/`.
#[server(GetContainers, "/api")]
pub async fn get_containers() -> Result<Vec<Container>, ServerFnError> {
    let pool = crate::db::pool()?;
    crate::db::list_containers(&pool)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Mengambil satu container berdasarkan `id` dari segmen rute.
#[server(GetContainer, "/api")]
pub async fn get_container(id: String) -> Result<Option<Container>, ServerFnError> {
    let id: i64 = id
        .parse()
        .map_err(|_| ServerFnError::new(format!("id `{id}` bukan angka")))?;
    let pool = crate::db::pool()?;
    crate::db::find_container(&pool, id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Statistik agregat untuk halaman status.
#[server(GetStats, "/api")]
pub async fn get_stats() -> Result<Stats, ServerFnError> {
    let pool = crate::db::pool()?;
    crate::db::stats(&pool)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
