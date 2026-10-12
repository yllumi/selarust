//! SQLite tertanam: pool koneksi, WAL mode, dan tabel contoh.
//!
//! Hanya dikompilasi di sisi server (`ssr`) — klien Wasm tidak pernah menyentuh
//! database secara langsung, semua akses lewat server functions di `api.rs`.

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::api::{Container, Stats};

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
        seed_containers(&pool).await?;
    }
    Ok(pool)
}

/// Membuat skema bila belum ada (idempoten, aman dipanggil setiap boot).
pub async fn migrate(pool: &Pool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS containers (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            status     TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS containers_status_idx ON containers (status)")
        .execute(pool)
        .await?;
    Ok(())
}

/// Mengisi data contoh pada boot pertama saja.
pub async fn seed_containers(pool: &Pool) -> Result<(), sqlx::Error> {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM containers")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        return Ok(());
    }

    let rows = [
        ("api-gateway", "running"),
        ("worker-queue", "idle"),
        ("postgres-primary", "running"),
        ("redis-cache", "stopped"),
    ];
    for (name, status) in rows {
        sqlx::query("INSERT INTO containers (name, status) VALUES (?1, ?2)")
            .bind(name)
            .bind(status)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Semua container, terbaru lebih dulu.
pub async fn list_containers(pool: &Pool) -> Result<Vec<Container>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, name, status FROM containers ORDER BY id DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(map_container).collect())
}

/// Satu container, bila ada.
pub async fn find_container(pool: &Pool, id: i64) -> Result<Option<Container>, sqlx::Error> {
    let row = sqlx::query("SELECT id, name, status FROM containers WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_container))
}

/// Agregat status untuk halaman dashboard.
pub async fn stats(pool: &Pool) -> Result<Stats, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*)                                          AS total,
            COALESCE(SUM(status = 'running'), 0)              AS running,
            COALESCE(SUM(status <> 'running'), 0)              AS stopped
        FROM containers
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(Stats {
        total: row.try_get("total")?,
        running: row.try_get("running")?,
        stopped: row.try_get("stopped")?,
    })
}

fn map_container(row: &sqlx::sqlite::SqliteRow) -> Container {
    Container {
        id: row.get("id"),
        name: row.get("name"),
        status: row.get("status"),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_url;

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
}
