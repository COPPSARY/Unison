//! Library repository: provider-neutral upserts used by every provider's sync (spec §7, §8.5).
//! Cross-platform matching (T-30) will plug into `upsert_track`; until then each provider item
//! gets its own unified track row.

use std::collections::HashSet;

use sqlx::SqliteConnection;

use crate::error::AppResult;
use crate::model::{ProviderId, SignalKind};

/// A playable item from one provider, before unification.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackInput {
    pub item_id: String,
    pub url: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub isrc: Option<String>,
    pub artwork: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumInput {
    pub item_id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub artwork: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistInput {
    pub item_id: String,
    pub name: String,
    pub artwork: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistInput {
    pub item_id: String,
    pub name: String,
    pub owner: String,
    pub owner_id: String,
    pub editable: bool,
    pub artwork: Option<String>,
    pub snapshot: Option<String>,
    pub track_count: u32,
}

/// Library row kinds (spec §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    Liked,
    SavedAlbum,
    FollowedArtist,
}

impl LibraryKind {
    fn as_str(self) -> &'static str {
        match self {
            LibraryKind::Liked => "liked",
            LibraryKind::SavedAlbum => "saved_album",
            LibraryKind::FollowedArtist => "followed_artist",
        }
    }
}

pub fn signal_kind_str(kind: SignalKind) -> &'static str {
    match kind {
        SignalKind::TopShort => "top_short",
        SignalKind::TopMedium => "top_medium",
        SignalKind::TopLong => "top_long",
        SignalKind::Recent => "recent",
        SignalKind::Liked => "liked",
        SignalKind::Play => "play",
        SignalKind::Skip => "skip",
        SignalKind::Replay => "replay",
        SignalKind::Followed => "followed",
    }
}

/// Placeholder normalisation (lowercase, collapsed whitespace). T-30 replaces it with the full
/// normaliser and recomputes `*_norm` columns.
pub fn basic_norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn json_list(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".to_owned())
}

/// Inserts or refreshes a provider item and returns its unified track id.
pub async fn upsert_track(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    t: &TrackInput,
    now: i64,
) -> AppResult<String> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = ?1 AND item_id = ?2")
            .bind(provider.as_str())
            .bind(&t.item_id)
            .fetch_optional(&mut *conn)
            .await?;

    let primary_artist = t.artists.first().map(String::as_str).unwrap_or_default();
    let track_id = match existing {
        Some(id) => {
            sqlx::query(
                "UPDATE tracks SET title = ?2, title_norm = ?3, artists_json = ?4, artist_norm = ?5,
                   album = ?6, duration_ms = ?7, isrc = COALESCE(?8, isrc),
                   artwork_url = COALESCE(?9, artwork_url)
                 WHERE id = ?1",
            )
            .bind(&id)
            .bind(&t.title)
            .bind(basic_norm(&t.title))
            .bind(json_list(&t.artists))
            .bind(basic_norm(primary_artist))
            .bind(&t.album)
            .bind(t.duration_ms)
            .bind(&t.isrc)
            .bind(&t.artwork)
            .execute(&mut *conn)
            .await?;
            id
        }
        None => {
            let id = new_id();
            sqlx::query(
                "INSERT INTO tracks (id, title, title_norm, artists_json, artist_norm, album,
                   duration_ms, isrc, artwork_url, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(&id)
            .bind(&t.title)
            .bind(basic_norm(&t.title))
            .bind(json_list(&t.artists))
            .bind(basic_norm(primary_artist))
            .bind(&t.album)
            .bind(t.duration_ms)
            .bind(&t.isrc)
            .bind(&t.artwork)
            .bind(now)
            .execute(&mut *conn)
            .await?;
            id
        }
    };

    sqlx::query(
        "INSERT INTO track_sources (track_id, provider, item_id, url, available, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           url = excluded.url, available = excluded.available, fetched_at = excluded.fetched_at",
    )
    .bind(&track_id)
    .bind(provider.as_str())
    .bind(&t.item_id)
    .bind(&t.url)
    .bind(t.available)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    Ok(track_id)
}

pub async fn upsert_album(conn: &mut SqliteConnection, provider: ProviderId, a: &AlbumInput) -> AppResult<String> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO albums (id, provider, item_id, name, artists_json, artwork_url, release_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           name = excluded.name, artists_json = excluded.artists_json,
           artwork_url = excluded.artwork_url, release_date = excluded.release_date
         RETURNING id",
    )
    .bind(new_id())
    .bind(provider.as_str())
    .bind(&a.item_id)
    .bind(&a.name)
    .bind(json_list(&a.artists))
    .bind(&a.artwork)
    .bind(&a.release_date)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

pub async fn upsert_artist(conn: &mut SqliteConnection, provider: ProviderId, a: &ArtistInput) -> AppResult<String> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO artists (id, provider, item_id, name, name_norm, artwork_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           name = excluded.name, name_norm = excluded.name_norm, artwork_url = excluded.artwork_url
         RETURNING id",
    )
    .bind(new_id())
    .bind(provider.as_str())
    .bind(&a.item_id)
    .bind(&a.name)
    .bind(basic_norm(&a.name))
    .bind(&a.artwork)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

/// Upserts playlist metadata. Returns `(id, snapshot_changed)`; a changed snapshot resets
/// `items_synced` so items are re-fetched lazily (T-15).
pub async fn upsert_playlist(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    p: &PlaylistInput,
    now: i64,
) -> AppResult<(String, bool)> {
    let previous: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT id, snapshot FROM playlists WHERE provider = ?1 AND item_id = ?2")
            .bind(provider.as_str())
            .bind(&p.item_id)
            .fetch_optional(&mut *conn)
            .await?;
    let changed = previous.as_ref().is_none_or(|(_, snap)| *snap != p.snapshot);
    let id = previous.map(|(id, _)| id).unwrap_or_else(new_id);
    sqlx::query(
        "INSERT INTO playlists (id, provider, item_id, name, owner, owner_id, editable, artwork_url,
           snapshot, items_synced, synced_at, track_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           name = excluded.name, owner = excluded.owner, owner_id = excluded.owner_id,
           editable = excluded.editable, artwork_url = excluded.artwork_url,
           snapshot = excluded.snapshot, synced_at = excluded.synced_at,
           track_count = excluded.track_count,
           items_synced = CASE WHEN ?12 THEN 0 ELSE playlists.items_synced END",
    )
    .bind(&id)
    .bind(provider.as_str())
    .bind(&p.item_id)
    .bind(&p.name)
    .bind(&p.owner)
    .bind(&p.owner_id)
    .bind(p.editable)
    .bind(&p.artwork)
    .bind(&p.snapshot)
    .bind(now)
    .bind(p.track_count)
    .bind(changed)
    .execute(&mut *conn)
    .await?;
    Ok((id, changed))
}

/// Records an item as liked/saved/followed. `target` is the unified track id (liked), album id
/// (saved_album) or nothing (followed_artist; `ref_id` is the provider artist id).
pub async fn upsert_library(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    kind: LibraryKind,
    ref_id: &str,
    target: Option<&str>,
    added_at: Option<i64>,
) -> AppResult<()> {
    let (track_id, album_id) = match kind {
        LibraryKind::Liked => (target, None),
        LibraryKind::SavedAlbum => (None, target),
        LibraryKind::FollowedArtist => (None, None),
    };
    sqlx::query(
        "INSERT INTO library (track_id, album_id, provider, kind, ref_id, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(provider, kind, ref_id) DO UPDATE SET
           track_id = excluded.track_id, album_id = excluded.album_id,
           added_at = COALESCE(excluded.added_at, library.added_at)",
    )
    .bind(track_id)
    .bind(album_id)
    .bind(provider.as_str())
    .bind(kind.as_str())
    .bind(ref_id)
    .bind(added_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Removes library rows of `kind` whose `ref_id` is not in `keep`. Call only after a complete
/// page set (spec §8.5). Tracks themselves are kept. Returns how many rows were removed.
pub async fn prune_library(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    kind: LibraryKind,
    keep: &HashSet<String>,
) -> AppResult<u32> {
    let existing: Vec<String> =
        sqlx::query_scalar("SELECT ref_id FROM library WHERE provider = ?1 AND kind = ?2")
            .bind(provider.as_str())
            .bind(kind.as_str())
            .fetch_all(&mut *conn)
            .await?;
    let mut removed = 0u32;
    for ref_id in existing.iter().filter(|r| !keep.contains(*r)) {
        sqlx::query("DELETE FROM library WHERE provider = ?1 AND kind = ?2 AND ref_id = ?3")
            .bind(provider.as_str())
            .bind(kind.as_str())
            .bind(ref_id)
            .execute(&mut *conn)
            .await?;
        removed += 1;
    }
    Ok(removed)
}

/// Removes provider playlists not in `keep` (their items cascade). Unison playlists untouched.
pub async fn prune_playlists(
    conn: &mut SqliteConnection,
    provider: ProviderId,
    keep: &HashSet<String>,
) -> AppResult<u32> {
    let existing: Vec<String> = sqlx::query_scalar("SELECT item_id FROM playlists WHERE provider = ?1")
        .bind(provider.as_str())
        .fetch_all(&mut *conn)
        .await?;
    let mut removed = 0u32;
    for item_id in existing.iter().filter(|r| !keep.contains(*r)) {
        sqlx::query("DELETE FROM playlists WHERE provider = ?1 AND item_id = ?2")
            .bind(provider.as_str())
            .bind(item_id)
            .execute(&mut *conn)
            .await?;
        removed += 1;
    }
    Ok(removed)
}

/// A taste signal ready to store: `subject_id` is a unified track id or a normalised artist name.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalRow {
    pub track: bool,
    pub subject_id: String,
    pub kind: SignalKind,
    pub weight: f64,
    pub at: i64,
}

/// Replaces all signals of the given kinds (top/recent are re-fetched in full each sync).
pub async fn replace_signals(
    conn: &mut SqliteConnection,
    kinds: &[SignalKind],
    rows: &[SignalRow],
) -> AppResult<()> {
    for kind in kinds {
        sqlx::query("DELETE FROM signals WHERE kind = ?1")
            .bind(signal_kind_str(*kind))
            .execute(&mut *conn)
            .await?;
    }
    for r in rows {
        sqlx::query("INSERT INTO signals (subject_type, subject_id, kind, weight, at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(if r.track { "track" } else { "artist" })
            .bind(&r.subject_id)
            .bind(signal_kind_str(r.kind))
            .bind(r.weight)
            .bind(r.at)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

pub async fn set_sync_state(conn: &mut SqliteConnection, provider: ProviderId, resource: &str, now: i64) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO sync_state (provider, resource, last_synced_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(provider, resource) DO UPDATE SET last_synced_at = excluded.last_synced_at",
    )
    .bind(provider.as_str())
    .bind(resource)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn set_last_synced(conn: &mut SqliteConnection, provider: ProviderId, now: i64) -> AppResult<()> {
    sqlx::query("UPDATE accounts SET last_synced_at = ?1 WHERE provider = ?2")
        .bind(now)
        .bind(provider.as_str())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn count_library(conn: &mut SqliteConnection, provider: ProviderId, kind: LibraryKind) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM library WHERE provider = ?1 AND kind = ?2")
        .bind(provider.as_str())
        .bind(kind.as_str())
        .fetch_one(&mut *conn)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;

    fn track(id: &str) -> TrackInput {
        TrackInput {
            item_id: id.into(),
            url: format!("https://open.spotify.com/track/{id}"),
            title: format!("  Song  {id} "),
            artists: vec!["Artist".into()],
            album: Some("Album".into()),
            duration_ms: Some(200_000),
            isrc: None,
            artwork: None,
            available: true,
        }
    }

    #[tokio::test]
    async fn track_upsert_is_stable_and_prune_keeps_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();

        let a = upsert_track(&mut conn, ProviderId::Spotify, &track("t1"), 1).await.unwrap();
        let again = upsert_track(&mut conn, ProviderId::Spotify, &track("t1"), 2).await.unwrap();
        assert_eq!(a, again, "same provider item keeps its unified id");
        let norm: String = sqlx::query_scalar("SELECT title_norm FROM tracks WHERE id = ?1")
            .bind(&a)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(norm, "song t1");

        let b = upsert_track(&mut conn, ProviderId::Spotify, &track("t2"), 1).await.unwrap();
        upsert_library(&mut conn, ProviderId::Spotify, LibraryKind::Liked, "t1", Some(&a), Some(10)).await.unwrap();
        upsert_library(&mut conn, ProviderId::Spotify, LibraryKind::Liked, "t2", Some(&b), Some(11)).await.unwrap();

        let keep: HashSet<String> = ["t2".to_owned()].into();
        assert_eq!(prune_library(&mut conn, ProviderId::Spotify, LibraryKind::Liked, &keep).await.unwrap(), 1);
        assert_eq!(count_library(&mut conn, ProviderId::Spotify, LibraryKind::Liked).await.unwrap(), 1);
        let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&mut *conn).await.unwrap();
        assert_eq!(tracks, 2, "tracks survive library pruning");
    }

    #[tokio::test]
    async fn playlist_snapshot_change_resets_items_synced() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let mut p = PlaylistInput {
            item_id: "p1".into(),
            name: "Mix".into(),
            owner: "Me".into(),
            owner_id: "me".into(),
            editable: true,
            artwork: None,
            snapshot: Some("s1".into()),
            track_count: 3,
        };
        let (id, changed) = upsert_playlist(&mut conn, ProviderId::Spotify, &p, 1).await.unwrap();
        assert!(changed);
        sqlx::query("UPDATE playlists SET items_synced = 1 WHERE id = ?1").bind(&id).execute(&mut *conn).await.unwrap();
        let (_, changed) = upsert_playlist(&mut conn, ProviderId::Spotify, &p, 2).await.unwrap();
        assert!(!changed);
        let synced: i64 = sqlx::query_scalar("SELECT items_synced FROM playlists WHERE id = ?1").bind(&id).fetch_one(&mut *conn).await.unwrap();
        assert_eq!(synced, 1);
        p.snapshot = Some("s2".into());
        let (same_id, changed) = upsert_playlist(&mut conn, ProviderId::Spotify, &p, 3).await.unwrap();
        assert!(changed);
        assert_eq!(same_id, id);
        let synced: i64 = sqlx::query_scalar("SELECT items_synced FROM playlists WHERE id = ?1").bind(&id).fetch_one(&mut *conn).await.unwrap();
        assert_eq!(synced, 0);
        assert_eq!(prune_playlists(&mut conn, ProviderId::Spotify, &HashSet::new()).await.unwrap(), 1);
    }
}
