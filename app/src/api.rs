//! Server functions: dipanggil dari Wasm layaknya fungsi lokal, dieksekusi di Axum.
//!
//! Setiap fungsi beranotasi `#[server]` otomatis mendapat endpoint HTTP di bawah
//! `/api/` — namanya `<nama_fungsi>` ditambah hash kecil dari lokasi sumber, mis.
//! `/api/list_todos4286897385061139490`. Itulah sebabnya `Sayagi.toml` menaruh
//! `/api/` di `network_first` untuk service worker, bukan daftar URL spesifik
//! yang harus diperbarui setiap kali ada fungsi baru.

use leptos::prelude::*;
use leptos::server_fn::codec::GetUrl;
use serde::{Deserialize, Serialize};

/// Batas panjang judul yang diterima semua endpoint tulis.
pub const MAX_TITLE: usize = 120;

/// Satu baris tabel `todos`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    /// Primary key.
    pub id: i64,
    /// Judul singkat.
    pub title: String,
    /// Catatan bebas (boleh kosong).
    pub notes: String,
    /// Sudah selesai atau belum.
    pub completed: bool,
    /// Timestamp SQLite (`datetime('now')`, UTC).
    pub created_at: String,
}

/// Perubahan todo yang disiarkan ke semua klien yang terhubung.
///
/// Klien memakai event ini untuk menyamakan daftarnya tanpa memuat ulang dari
/// server, dan `apply` adalah satu-satunya tempat aturan itu ditulis — dipakai
/// oleh sinkronisasi WebSocket maupun saat memutar ulang antrean offline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TodoEvent {
    /// Todo baru tersimpan.
    Created { todo: Todo },
    /// Judul/catatan berubah.
    Updated { todo: Todo },
    /// Status selesai berubah.
    Completed { id: i64, completed: bool },
    /// Todo dihapus.
    Deleted { id: i64 },
}

impl TodoEvent {
    /// Applies the event to a client-side list, newest first.
    pub fn apply(&self, todos: &mut Vec<Todo>) {
        match self {
            TodoEvent::Created { todo } => upsert(todos, todo.clone()),
            TodoEvent::Updated { todo } => {
                // A row can be unknown locally (e.g. created on another device).
                if todos.iter().any(|existing| existing.id == todo.id) {
                    upsert(todos, todo.clone());
                }
            }
            TodoEvent::Completed { id, completed } => {
                if let Some(todo) = todos.iter_mut().find(|todo| todo.id == *id) {
                    todo.completed = *completed;
                }
            }
            TodoEvent::Deleted { id } => todos.retain(|todo| todo.id != *id),
        }
    }
}

/// Inserts or replaces a todo, keeping the newest-first ordering.
fn upsert(todos: &mut Vec<Todo>, todo: Todo) {
    match todos.iter_mut().find(|existing| existing.id == todo.id) {
        Some(existing) => *existing = todo,
        None => {
            todos.push(todo);
            todos.sort_by(|left, right| right.id.cmp(&left.id));
        }
    }
}

/// Ringkasan daftar todo untuk halaman status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoStats {
    /// Jumlah seluruh todo.
    pub total: i64,
    /// Yang belum selesai.
    pub open: i64,
    /// Yang sudah selesai.
    pub done: i64,
}

/// Menyiarkan perubahan ke klien lain yang sedang terhubung.
///
/// Diam-diam tidak melakukan apa-apa bila hub belum disediakan (mis. saat
/// server function dipanggil dari test), supaya mutasi tetap berhasil.
#[cfg(feature = "ssr")]
fn publish(event: TodoEvent) {
    if let Some(hub) = use_context::<crate::sync::SyncHub>() {
        hub.publish(event);
    }
}

/// Merapikan judul dan menolak masukan kosong/terlalu panjang.
///
/// Hanya dipakai badan server function, jadi tidak ikut dikompilasi ke Wasm.
#[cfg(feature = "ssr")]
fn clean_title(title: &str) -> Result<String, ServerFnError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ServerFnError::new("Judul todo tidak boleh kosong"));
    }
    if title.chars().count() > MAX_TITLE {
        return Err(ServerFnError::new(format!(
            "Judul todo maksimal {MAX_TITLE} karakter"
        )));
    }
    Ok(title.to_string())
}

#[cfg(feature = "ssr")]
fn clean_notes(notes: Option<String>) -> String {
    notes.unwrap_or_default().trim().to_string()
}

#[cfg(feature = "ssr")]
fn parse_id(id: &str) -> Result<i64, ServerFnError> {
    id.trim()
        .parse()
        .map_err(|_| ServerFnError::new(format!("id `{id}` bukan angka")))
}

/// Seluruh todo, terbaru lebih dulu.
///
/// Bentuk `#[server(Nama, "/api")]` mengikuti pola di ARCHITECTURE.md §5.3:
/// argumen kedua menetapkan prefix URL, sehingga endpoint-nya berada di bawah
/// `/api/`.
///
/// `input = GetUrl` membuat fungsi baca memakai HTTP GET. Service worker hanya
/// merutekan GET (Cache API memang tidak bisa menyimpan request POST), jadi
/// inilah yang membuat daftar todo tetap terbaca saat perangkat offline.
/// Fungsi tulis di bawah tetap POST supaya tidak pernah di-cache.
#[server(ListTodos, "/api", input = GetUrl)]
pub async fn list_todos() -> Result<Vec<Todo>, ServerFnError> {
    let pool = crate::db::pool()?;
    crate::db::list_todos(&pool)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Satu todo berdasarkan `id` dari segmen rute.
#[server(GetTodo, "/api", input = GetUrl)]
pub async fn get_todo(id: String) -> Result<Option<Todo>, ServerFnError> {
    let id = parse_id(&id)?;
    let pool = crate::db::pool()?;
    crate::db::find_todo(&pool, id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Membuat todo baru. Contoh pemakaian `ActionForm` — tidak ada endpoint REST manual.
#[server(CreateTodo, "/api")]
pub async fn create_todo(title: String, notes: Option<String>) -> Result<Todo, ServerFnError> {
    let title = clean_title(&title)?;
    let pool = crate::db::pool()?;
    let todo = crate::db::create_todo(&pool, &title, &clean_notes(notes))
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

    publish(TodoEvent::Created { todo: todo.clone() });
    Ok(todo)
}

/// Mengubah judul dan catatan satu todo.
#[server(UpdateTodo, "/api")]
pub async fn update_todo(
    id: i64,
    title: String,
    notes: Option<String>,
) -> Result<Todo, ServerFnError> {
    let title = clean_title(&title)?;
    let pool = crate::db::pool()?;
    let todo = crate::db::update_todo(&pool, id, &title, &clean_notes(notes))
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?
        .ok_or_else(|| ServerFnError::new(format!("todo #{id} tidak ditemukan")))?;

    publish(TodoEvent::Updated { todo: todo.clone() });
    Ok(todo)
}

/// Menandai selesai/belum selesai dari daftar.
#[server(SetTodoCompleted, "/api")]
pub async fn set_todo_completed(id: i64, completed: bool) -> Result<(), ServerFnError> {
    let pool = crate::db::pool()?;
    let changed = crate::db::set_completed(&pool, id, completed)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

    if changed {
        publish(TodoEvent::Completed { id, completed });
        Ok(())
    } else {
        Err(ServerFnError::new(format!("todo #{id} tidak ditemukan")))
    }
}

/// Menghapus satu todo.
#[server(DeleteTodo, "/api")]
pub async fn delete_todo(id: i64) -> Result<(), ServerFnError> {
    let pool = crate::db::pool()?;
    let deleted = crate::db::delete_todo(&pool, id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))?;

    if deleted {
        publish(TodoEvent::Deleted { id });
        Ok(())
    } else {
        Err(ServerFnError::new(format!("todo #{id} tidak ditemukan")))
    }
}

/// Statistik agregat untuk halaman status.
#[server(GetTodoStats, "/api", input = GetUrl)]
pub async fn get_todo_stats() -> Result<TodoStats, ServerFnError> {
    let pool = crate::db::pool()?;
    crate::db::todo_stats(&pool)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
