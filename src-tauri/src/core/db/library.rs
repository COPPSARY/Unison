//! Library repository: source-neutral upserts (spec §7). `provider` is the `track_sources`
//! provider string (`local`, `audius`, `extension`).

use sqlx::SqliteConnection;

use crate::error::AppResult;

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

/// Inserts or refreshes a source item and returns its unified track id.
pub async fn upsert_track(
    conn: &mut SqliteConnection,
    provider: &str,
    t: &TrackInput,
    now: i64,
) -> AppResult<String> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = ?1 AND item_id = ?2")
            .bind(provider)
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
    .bind(provider)
    .bind(&t.item_id)
    .bind(&t.url)
    .bind(t.available)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    Ok(track_id)
}

pub async fn upsert_album(conn: &mut SqliteConnection, provider: &str, a: &AlbumInput) -> AppResult<String> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO albums (id, provider, item_id, name, artists_json, artwork_url, release_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           name = excluded.name, artists_json = excluded.artists_json,
           artwork_url = excluded.artwork_url, release_date = excluded.release_date
         RETURNING id",
    )
    .bind(new_id())
    .bind(provider)
    .bind(&a.item_id)
    .bind(&a.name)
    .bind(json_list(&a.artists))
    .bind(&a.artwork)
    .bind(&a.release_date)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

pub async fn upsert_artist(conn: &mut SqliteConnection, provider: &str, a: &ArtistInput) -> AppResult<String> {
    let id: String = sqlx::query_scalar(
        "INSERT INTO artists (id, provider, item_id, name, name_norm, artwork_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(provider, item_id) DO UPDATE SET
           name = excluded.name, name_norm = excluded.name_norm, artwork_url = excluded.artwork_url
         RETURNING id",
    )
    .bind(new_id())
    .bind(provider)
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
    provider: &str,
    p: &PlaylistInput,
    now: i64,
) -> AppResult<(String, bool)> {
    let previous: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT id, snapshot FROM playlists WHERE provider = ?1 AND item_id = ?2")
            .bind(provider)
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
    .bind(provider)
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
    provider: &str,
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
    .bind(provider)
    .bind(kind.as_str())
    .bind(ref_id)
    .bind(added_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Replaces a playlist's items with `items` (unified track id, added_at) in order and marks
/// them synced. `track_count` becomes the stored count so the header matches the table.
pub async fn replace_playlist_items(
    conn: &mut SqliteConnection,
    playlist_id: &str,
    provider: &str,
    items: &[(String, Option<i64>)],
    now: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM playlist_items WHERE playlist_id = ?1")
        .bind(playlist_id)
        .execute(&mut *conn)
        .await?;
    for (position, (track_id, added_at)) in items.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_items (playlist_id, position, track_id, source_provider, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(playlist_id)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(track_id)
        .bind(provider)
        .bind(added_at)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query("UPDATE playlists SET items_synced = 1, synced_at = ?2, track_count = ?3 WHERE id = ?1")
        .bind(playlist_id)
        .bind(now)
        .bind(i64::try_from(items.len()).unwrap_or(i64::MAX))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Adds a source to `track_id` unless that source is already stored (on any track). Returns
/// the track the source belongs to.
pub async fn attach_source(
    conn: &mut SqliteConnection,
    track_id: &str,
    provider: &str,
    t: &TrackInput,
    now: i64,
) -> AppResult<String> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = ?1 AND item_id = ?2")
            .bind(provider)
            .bind(&t.item_id)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some(id) = existing {
        return Ok(id);
    }
    sqlx::query(
        "INSERT INTO track_sources (track_id, provider, item_id, url, available, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(track_id)
    .bind(provider)
    .bind(&t.item_id)
    .bind(&t.url)
    .bind(t.available)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(track_id.to_owned())
}

/// Makes one source play first among its track's sources of the same kind.
pub async fn prefer_source(conn: &mut SqliteConnection, provider: &str, item_id: &str, now: i64) -> AppResult<()> {
    sqlx::query("UPDATE track_sources SET preference = ?3 WHERE provider = ?1 AND item_id = ?2")
        .bind(provider)
        .bind(item_id)
        .bind(now)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn is_liked(conn: &mut SqliteConnection, track_id: &str) -> AppResult<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library WHERE kind = 'liked' AND track_id = ?1")
        .bind(track_id)
        .fetch_one(&mut *conn)
        .await?;
    Ok(n > 0)
}

/// Album row id for a liked track: `lower(album)|artist_norm` (matches SQLite's ASCII `lower`).
fn album_ref(album: &str, artist_norm: &str) -> String {
    format!("{}|{artist_norm}", album.to_ascii_lowercase())
}

/// Likes a track; its album and primary artist appear under Albums and Artists.
/// `ref_id` is the source item the like came from.
pub async fn like_track(
    conn: &mut SqliteConnection,
    track_id: &str,
    provider: &str,
    ref_id: &str,
    now: i64,
) -> AppResult<()> {
    let (artists_json, artist_norm, album, artwork): (String, String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT artists_json, artist_norm, album, artwork_url FROM tracks WHERE id = ?1")
            .bind(track_id)
            .fetch_one(&mut *conn)
            .await?;
    let artists: Vec<String> = serde_json::from_str(&artists_json).unwrap_or_default();
    upsert_library(conn, provider, LibraryKind::Liked, ref_id, Some(track_id), Some(now)).await?;
    let primary = artists.first().cloned().unwrap_or_default();
    if let Some(album) = album.filter(|a| !a.trim().is_empty()) {
        let input = AlbumInput {
            item_id: album_ref(&album, &artist_norm),
            name: album,
            artists: artists.iter().take(1).cloned().collect(),
            artwork,
            release_date: None,
        };
        let album_id = upsert_album(conn, provider, &input).await?;
        upsert_library(conn, provider, LibraryKind::SavedAlbum, &input.item_id, Some(&album_id), Some(now)).await?;
    }
    if !primary.trim().is_empty() {
        let input = ArtistInput { item_id: artist_norm.clone(), name: primary, artwork: None };
        upsert_artist(conn, provider, &input).await?;
        upsert_library(conn, provider, LibraryKind::FollowedArtist, &artist_norm, None, Some(now)).await?;
    }
    Ok(())
}

/// Unlikes a track; albums and artists with no other liked track leave Albums and Artists.
pub async fn unlike_track(conn: &mut SqliteConnection, track_id: &str, provider: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM library WHERE kind = 'liked' AND track_id = ?1").bind(track_id).execute(&mut *conn).await?;
    sqlx::query(
        "DELETE FROM library WHERE kind = 'saved_album' AND provider = ?1 AND NOT EXISTS (
           SELECT 1 FROM library l JOIN tracks t ON t.id = l.track_id
           WHERE l.kind = 'liked' AND lower(COALESCE(t.album, '')) || '|' || t.artist_norm = library.ref_id)",
    )
    .bind(provider)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "DELETE FROM library WHERE kind = 'followed_artist' AND provider = ?1 AND NOT EXISTS (
           SELECT 1 FROM library l JOIN tracks t ON t.id = l.track_id
           WHERE l.kind = 'liked' AND t.artist_norm = library.ref_id)",
    )
    .bind(provider)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;

    fn track(id: &str) -> TrackInput {
        TrackInput {
            item_id: id.into(),
            url: format!("https://music.example/track/{id}"),
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
    async fn track_upsert_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();

        let a = upsert_track(&mut conn, "extension", &track("t1"), 1).await.unwrap();
        let again = upsert_track(&mut conn, "extension", &track("t1"), 2).await.unwrap();
        assert_eq!(a, again, "same provider item keeps its unified id");
        let norm: String = sqlx::query_scalar("SELECT title_norm FROM tracks WHERE id = ?1")
            .bind(&a)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(norm, "song t1");

        let b = upsert_track(&mut conn, "extension", &track("t2"), 1).await.unwrap();
        upsert_library(&mut conn, "extension", LibraryKind::Liked, "t1", Some(&a), Some(10)).await.unwrap();
        upsert_library(&mut conn, "extension", LibraryKind::Liked, "t2", Some(&b), Some(11)).await.unwrap();
        let liked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library WHERE kind = 'liked'")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(liked, 2);
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
        let (id, changed) = upsert_playlist(&mut conn, "extension", &p, 1).await.unwrap();
        assert!(changed);
        sqlx::query("UPDATE playlists SET items_synced = 1 WHERE id = ?1").bind(&id).execute(&mut *conn).await.unwrap();
        let (_, changed) = upsert_playlist(&mut conn, "extension", &p, 2).await.unwrap();
        assert!(!changed);
        let synced: i64 = sqlx::query_scalar("SELECT items_synced FROM playlists WHERE id = ?1").bind(&id).fetch_one(&mut *conn).await.unwrap();
        assert_eq!(synced, 1);
        p.snapshot = Some("s2".into());
        let (same_id, changed) = upsert_playlist(&mut conn, "extension", &p, 3).await.unwrap();
        assert!(changed);
        assert_eq!(same_id, id);
        let synced: i64 = sqlx::query_scalar("SELECT items_synced FROM playlists WHERE id = ?1").bind(&id).fetch_one(&mut *conn).await.unwrap();
        assert_eq!(synced, 0);
    }

    #[tokio::test]
    async fn liking_fills_albums_and_artists_and_unliking_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let a = upsert_track(&mut conn, "extension", &track("x:1"), 1).await.unwrap();
        let b = upsert_track(&mut conn, "extension", &TrackInput { album: Some("Album".into()), ..track("x:2") }, 1).await.unwrap();
        // A second source joins the first track instead of making a new one.
        let joined = attach_source(&mut conn, &a, "extension", &track("y:1"), 1).await.unwrap();
        assert_eq!(joined, a);
        assert_eq!(attach_source(&mut conn, &b, "extension", &track("y:1"), 1).await.unwrap(), a, "already stored");

        like_track(&mut conn, &a, "extension", "x:1", 5).await.unwrap();
        like_track(&mut conn, &b, "extension", "x:2", 6).await.unwrap();
        assert!(is_liked(&mut conn, &a).await.unwrap());
        let count = |kind: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library WHERE kind = ?1").bind(kind).fetch_one(&pool).await.unwrap()
            }
        };
        assert_eq!((count("liked").await, count("saved_album").await, count("followed_artist").await), (2, 1, 1));

        unlike_track(&mut conn, &a, "extension").await.unwrap();
        assert_eq!((count("liked").await, count("saved_album").await, count("followed_artist").await), (1, 1, 1));
        unlike_track(&mut conn, &b, "extension").await.unwrap();
        assert_eq!((count("liked").await, count("saved_album").await, count("followed_artist").await), (0, 0, 0));
    }
}
