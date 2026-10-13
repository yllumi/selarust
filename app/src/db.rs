//! SQLite tertanam: pool koneksi, WAL mode, dan tabel `todos`.
//!
//! Hanya dikompilasi di sisi server (`ssr`) — klien Wasm tidak pernah menyentuh
//! database secara langsung, semua akses lewat server functions di `api.rs`.

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::api::{Todo, TodoStats};

/// Tipe pool yang dibagikan lewat context Leptos ke server functions.
pub type Pool = SqlitePool;

/// Kunci context tempat pool disimpan (`provide_context` di `main.rs`).
pub fn pool() -> Result<Pool, leptos::prelude::ServerFnError> {
    leptos::prelude::use_context::<Pool>()
        .ok_or_else(|| leptos::prelude::ServerFnError::new("pool database belum disediakan"))
}

/// Mengubah `sqlite://sayagi.db` yang relatif menjadi absolut, berbasis direktori
/// biner dijalankan.
///
/// Tanpa ini, berkas database muncul di mana pun proses diluncurkan — misalnya di
/// root workspace saat binary dijalankan dari sana.
pub fn resolve_url(url: &str) -> String {
    let Some(file) = url.strip_prefix("sqlite://") else {
        return url.to_string();
    };
    if file.is_empty() || file == ":memory:" || file.starts_with('/') {
        return url.to_string();
    }

    let base = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok());

    match base {
        Some(base) => format!("sqlite://{}", base.join(file).display()),
        None => url.to_string(),
    }
}

/// Membuka database, menjalankan migrasi, dan mengisi data contoh bila kosong.
pub async fn connect(url: &str, max_connections: u32, seed: bool) -> Result<Pool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections.max(1))
        .connect_with(options)
        .await?;

    migrate(&pool).await?;
    if seed {
        seed_todos(&pool).await?;
    }
    Ok(pool)
}

/// Membuat skema bila belum ada (idempoten, aman dipanggil setiap boot).
pub async fn migrate(pool: &Pool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS todos (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            notes      TEXT NOT NULL DEFAULT '',
            completed  INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS todos_completed_idx ON todos (completed)")
        .execute(pool)
        .await?;
    Ok(())
}

/// Mengisi data contoh pada boot pertama saja.
pub async fn seed_todos(pool: &Pool) -> Result<(), sqlx::Error> {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM todos")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        return Ok(());
    }

    let rows = [
        ("Coba form tambah todo", "Todo baru muncul di paling atas.", false),
        ("Ketuk judul untuk membuka detail", "Di sana judul dan catatan bisa diubah.", false),
        ("Tandai selesai lewat tombol centang", "Perubahannya langsung tersimpan ke SQLite.", true),
    ];
    for (title, notes, completed) in rows {
        sqlx::query("INSERT INTO todos (title, notes, completed) VALUES (?1, ?2, ?3)")
            .bind(title)
            .bind(notes)
            .bind(if completed { 1_i64 } else { 0_i64 })
            .execute(pool)
            .await?;
    }
    Ok(())
}

const COLUMNS: &str = "id, title, notes, completed, created_at";

/// Semua todo, terbaru lebih dulu.
pub async fn list_todos(pool: &Pool) -> Result<Vec<Todo>, sqlx::Error> {
    let sql = format!("SELECT {COLUMNS} FROM todos ORDER BY id DESC");
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    Ok(rows.iter().map(map_todo).collect())
}

/// Satu todo, bila ada.
pub async fn find_todo(pool: &Pool, id: i64) -> Result<Option<Todo>, sqlx::Error> {
    let sql = format!("SELECT {COLUMNS} FROM todos WHERE id = ?1");
    let row = sqlx::query(&sql).bind(id).fetch_optional(pool).await?;
    Ok(row.as_ref().map(map_todo))
}

/// Menambah todo baru dan mengembalikan barisnya.
pub async fn create_todo(pool: &Pool, title: &str, notes: &str) -> Result<Todo, sqlx::Error> {
    let result = sqlx::query("INSERT INTO todos (title, notes) VALUES (?1, ?2)")
        .bind(title)
        .bind(notes)
        .execute(pool)
        .await?;

    let id = result.last_insert_rowid();
    find_todo(pool, id)
        .await?
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

/// Mengubah judul dan catatan. Mengembalikan `None` bila id tidak ada.
pub async fn update_todo(
    pool: &Pool,
    id: i64,
    title: &str,
    notes: &str,
) -> Result<Option<Todo>, sqlx::Error> {
    let result = sqlx::query("UPDATE todos SET title = ?1, notes = ?2 WHERE id = ?3")
        .bind(title)
        .bind(notes)
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }
    find_todo(pool, id).await
}

/// Menandai selesai/belum selesai. `false` bila id tidak ada.
pub async fn set_completed(pool: &Pool, id: i64, completed: bool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE todos SET completed = ?1 WHERE id = ?2")
        .bind(if completed { 1_i64 } else { 0_i64 })
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Menghapus satu todo. `false` bila id tidak ada.
pub async fn delete_todo(pool: &Pool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM todos WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Agregat untuk halaman status.
pub async fn todo_stats(pool: &Pool) -> Result<TodoStats, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*)                        AS total,
            COALESCE(SUM(completed = 0), 0) AS open,
            COALESCE(SUM(completed = 1), 0) AS done
        FROM todos
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(TodoStats {
        total: row.try_get("total")?,
        open: row.try_get("open")?,
        done: row.try_get("done")?,
    })
}

fn map_todo(row: &sqlx::sqlite::SqliteRow) -> Todo {
    Todo {
        id: row.get("id"),
        title: row.get("title"),
        notes: row.get("notes"),
        completed: row.get::<i64, _>("completed") != 0,
        created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_url, COLUMNS};

    #[test]
    fn relative_sqlite_urls_are_absolutised() {
        let resolved = resolve_url("sqlite://sayagi.db");
        assert!(
            resolved.starts_with("sqlite:///"),
            "expected an absolute path, got {resolved}"
        );
        assert!(resolved.ends_with("/sayagi.db"), "got {resolved}");
        assert_ne!(resolved, "sqlite://sayagi.db");
    }

    #[test]
    fn absolute_memory_and_other_urls_are_untouched() {
        assert_eq!(resolve_url("sqlite:///var/lib/app.db"), "sqlite:///var/lib/app.db");
        assert_eq!(resolve_url("sqlite::memory:"), "sqlite::memory:");
        assert_eq!(resolve_url("sqlite://:memory:"), "sqlite://:memory:");
        assert_eq!(resolve_url("postgres://localhost/app"), "postgres://localhost/app");
    }

    #[test]
    fn selected_columns_match_the_row_mapper() {
        for column in ["id", "title", "notes", "completed", "created_at"] {
            assert!(COLUMNS.contains(column), "`{column}` tidak ada di COLUMNS");
        }
    }
}
