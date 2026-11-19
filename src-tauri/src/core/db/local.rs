//! Local files repository (addendum §4 steps 6–7, §6): `local_files`, the `local` track source
//! (item_id = fingerprint, url = absolute path) and `library(provider='local', kind='local')`.

use std::collections::HashMap;
use std::path::Path;

use sqlx::SqliteConnection;

use super::library::basic_norm;
use crate::core::local::tags::FileMeta;
use crate::core::matching::matcher::{MatchIndex, MatchKey};
use crate::error::AppResult;

pub const PROVIDER: &str = "local";
pub const KIND: &str = "local";

/// A file read from disk and ready to store.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalFileInput {
    pub fingerprint: String,
    pub path: String,
    pub size: u64,
    pub mtime: i64,
    pub meta: FileMeta,
    pub artwork_path: Option<String>,
    /// Webview URL of the saved cover (`artwork://…`).
    pub artwork_url: Option<String>,
}

/// What storing a file did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// New in the library.
    Imported,
    /// Known file that moved, or a path whose contents changed.
    Updated,
    /// Already imported (same path, or a second copy of the same file).
    Unchanged,
}

/// `path → (size, mtime)` for every imported file, to skip unchanged files without reading them.
pub async fn known_files(conn: &mut SqliteConnection) -> AppResult<HashMap<String, (i64, i64)>> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as("SELECT path, size, mtime FROM local_files")
        .fetch_all(&mut *conn)
        .await?;
    Ok(rows.into_iter().map(|(p, s, m)| (p, (s, m))).collect())
}

fn json_list(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_owned())
}

/// Removes one imported file's rows. Its track goes too when nothing else refers to it.
/// Returns the deleted track id, if any.
pub async fn remove(conn: &mut SqliteConnection, fingerprint: &str) -> AppResult<Option<String>> {
    let track_id: Option<String> =
        sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = ?1 AND item_id = ?2")
            .bind(PROVIDER)
            .bind(fingerprint)
            .fetch_optional(&mut *conn)
            .await?;
    sqlx::query("DELETE FROM local_files WHERE fingerprint = ?1").bind(fingerprint).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM track_sources WHERE provider = ?1 AND item_id = ?2")
        .bind(PROVIDER)
        .bind(fingerprint)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM library WHERE provider = ?1 AND kind = ?2 AND ref_id = ?3")
        .bind(PROVIDER)
        .bind(KIND)
        .bind(fingerprint)
        .execute(&mut *conn)
        .await?;
    let Some(track_id) = track_id else { return Ok(None) };
    let deleted = sqlx::query(
        "DELETE FROM tracks WHERE id = ?1
           AND NOT EXISTS (SELECT 1 FROM track_sources WHERE track_id = ?1)
           AND NOT EXISTS (SELECT 1 FROM library WHERE track_id = ?1)
           AND NOT EXISTS (SELECT 1 FROM playlist_items WHERE track_id = ?1)",
    )
    .bind(&track_id)
    .execute(&mut *conn)
    .await?;
    Ok((deleted.rows_affected() > 0).then_some(track_id))
}

/// Stores one file: dedupes by fingerprint, merges into an existing song through the matcher
/// (§8.2) or creates a new track, then writes the source and library rows.
pub async fn store(conn: &mut SqliteConnection, index: &mut MatchIndex, f: &LocalFileInput, now: i64) -> AppResult<Stored> {
    // Same path, different contents (re-tagged or replaced): drop the old entry first.
    let at_path: Option<String> = sqlx::query_scalar("SELECT fingerprint FROM local_files WHERE path = ?1")
        .bind(&f.path)
        .fetch_optional(&mut *conn)
        .await?;
    let mut replaced = false;
    if let Some(old) = at_path.filter(|old| *old != f.fingerprint) {
        if let Some(track_id) = remove(conn, &old).await? {
            index.remove(&track_id);
        }
        replaced = true;
    }

    let known_path: Option<String> = sqlx::query_scalar("SELECT path FROM local_files WHERE fingerprint = ?1")
        .bind(&f.fingerprint)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(old_path) = known_path {
        if old_path == f.path {
            sqlx::query("UPDATE local_files SET size = ?2, mtime = ?3 WHERE fingerprint = ?1")
                .bind(&f.fingerprint)
                .bind(i64::try_from(f.size).unwrap_or(i64::MAX))
                .bind(f.mtime)
                .execute(&mut *conn)
                .await?;
            return Ok(Stored::Unchanged);
        }
        if Path::new(&old_path).exists() {
            // A second copy of a file we already have: no duplicate.
            return Ok(if replaced { Stored::Updated } else { Stored::Unchanged });
        }
        // The file moved: follow it.
        sqlx::query("UPDATE local_files SET path = ?2, mtime = ?3 WHERE fingerprint = ?1")
            .bind(&f.fingerprint)
            .bind(&f.path)
            .bind(f.mtime)
            .execute(&mut *conn)
            .await?;
        sqlx::query(
            "UPDATE track_sources SET url = ?3, available = 1, unavailable_reason = NULL, fetched_at = ?4
             WHERE provider = ?1 AND item_id = ?2",
        )
        .bind(PROVIDER)
        .bind(&f.fingerprint)
        .bind(&f.path)
        .bind(now)
        .execute(&mut *conn)
        .await?;
        return Ok(Stored::Updated);
    }

    let m = &f.meta;
    let key = MatchKey::new(&m.title, &m.artists, m.duration_ms, m.isrc.as_deref());
    let (track_id, confidence) = match index.find(&key) {
        Some(found) => {
            // Keep the existing (usually richer) metadata; only fill gaps.
            sqlx::query(
                "UPDATE tracks SET isrc = COALESCE(isrc, ?2), duration_ms = COALESCE(duration_ms, ?3),
                   album = COALESCE(album, ?4), artwork_url = COALESCE(artwork_url, ?5)
                 WHERE id = ?1",
            )
            .bind(&found.track_id)
            .bind(&m.isrc)
            .bind(m.duration_ms)
            .bind(&m.album)
            .bind(&f.artwork_url)
            .execute(&mut *conn)
            .await?;
            (found.track_id, found.confidence)
        }
        None => {
            let id = uuid::Uuid::now_v7().to_string();
            let primary = m.artists.first().map(String::as_str).unwrap_or_default();
            sqlx::query(
                "INSERT INTO tracks (id, title, title_norm, artists_json, artist_norm, album,
                   duration_ms, isrc, artwork_url, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(&id)
            .bind(&m.title)
            .bind(basic_norm(&m.title))
            .bind(json_list(&m.artists))
            .bind(basic_norm(primary))
            .bind(&m.album)
            .bind(m.duration_ms)
            .bind(&m.isrc)
            .bind(&f.artwork_url)
            .bind(now)
            .execute(&mut *conn)
            .await?;
            index.insert(id.clone(), key);
            (id, 1.0)
        }
    };

    sqlx::query(
        "INSERT INTO local_files (fingerprint, path, size, mtime, codec, sample_rate, bitrate,
           artwork_path, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(&f.fingerprint)
    .bind(&f.path)
    .bind(i64::try_from(f.size).unwrap_or(i64::MAX))
    .bind(f.mtime)
    .bind(&m.codec)
    .bind(m.sample_rate)
    .bind(m.bitrate)
    .bind(&f.artwork_path)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT INTO track_sources (track_id, provider, item_id, url, available, match_confidence,
           fetched_at, quality, replaygain_track, replaygain_album)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           track_id = excluded.track_id, url = excluded.url, available = 1, unavailable_reason = NULL,
           match_confidence = excluded.match_confidence, fetched_at = excluded.fetched_at,
           quality = excluded.quality, replaygain_track = excluded.replaygain_track,
           replaygain_album = excluded.replaygain_album",
    )
    .bind(&track_id)
    .bind(PROVIDER)
    .bind(&f.fingerprint)
    .bind(&f.path)
    .bind(confidence)
    .bind(now)
    .bind(m.quality())
    .bind(m.replaygain_track)
    .bind(m.replaygain_album)
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT INTO library (track_id, album_id, provider, kind, ref_id, added_at)
         VALUES (?1, NULL, ?2, ?3, ?4, ?5)
         ON CONFLICT(provider, kind, ref_id) DO UPDATE SET track_id = excluded.track_id",
    )
    .bind(&track_id)
    .bind(PROVIDER)
    .bind(KIND)
    .bind(&f.fingerprint)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    Ok(if replaced { Stored::Updated } else { Stored::Imported })
}
