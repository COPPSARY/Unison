//! SQLite pool (spec §7): WAL mode, foreign keys on, migrations applied at startup.
//! Tokens and the Spotify Client ID never go in this database (keychain only).

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::error::AppResult;

/// File name inside the app data dir (`%APPDATA%\app.unison\unison.db`).
pub const DB_FILE: &str = "unison.db";

/// Migrations embedded at compile time from `src-tauri/migrations`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Opens (creating if needed) the database at `path` and applies pending migrations.
pub async fn open(path: &Path) -> AppResult<SqlitePool> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;
    tracing::info!(migrations = MIGRATOR.iter().count(), "database ready");
    Ok(pool)
}

/// Latest migration version applied to `pool`.
pub async fn schema_version(pool: &SqlitePool) -> AppResult<i64> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(pool)
            .await?;
    Ok(version.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let pool = open(&dir.path().join("nested").join(DB_FILE)).await.unwrap();
        (dir, pool)
    }

    #[tokio::test]
    async fn migrations_apply_on_an_empty_db() {
        let (dir, pool) = temp_db().await;
        assert!(dir.path().join("nested").join(DB_FILE).exists());
        assert_eq!(schema_version(&pool).await.unwrap(), 1);

        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type IN ('table') AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        for expected in [
            "accounts", "albums", "artists", "cache_kv", "library", "playlist_items", "playlists",
            "quota_usage", "settings", "signals", "sync_state", "track_sources", "tracks",
            "tracks_fts",
        ] {
            assert!(tables.iter().any(|t| t == expected), "missing table {expected}: {tables:?}");
        }
    }

    #[tokio::test]
    async fn wal_and_foreign_keys_are_on() {
        let (_dir, pool) = temp_db().await;
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode").fetch_one(&pool).await.unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(&pool).await.unwrap();
        assert_eq!(fk, 1);
    }

    #[tokio::test]
    async fn reopening_is_idempotent_and_data_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE);
        {
            let pool = open(&path).await.unwrap();
            sqlx::query("INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"')")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let pool = open(&path).await.unwrap();
        let value: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'theme'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "\"dark\"");
    }

    #[tokio::test]
    async fn cascades_and_fts5_work() {
        let (_dir, pool) = temp_db().await;
        sqlx::query(
            "INSERT INTO tracks (id, title, title_norm, artists_json, artist_norm, created_at)
             VALUES ('t1', 'Song', 'song', '[\"A\"]', 'a', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_sources (track_id, provider, item_id, url, fetched_at)
             VALUES ('t1', 'spotify', 'sp1', 'https://open.spotify.com/track/sp1', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM tracks WHERE id = 't1'").execute(&pool).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "track_sources should cascade");

        // FTS5 is compiled into the bundled SQLite.
        sqlx::query("INSERT INTO tracks_fts (rowid, title, artists, album) VALUES (1, 'Hello', 'X', 'Y')")
            .execute(&pool)
            .await
            .unwrap();
    }
}
