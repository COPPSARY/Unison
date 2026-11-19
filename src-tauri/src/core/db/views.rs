//! Read-side queries for the library views (spec §5 `library_*`, `playlists_list`). Local DB
//! only. SQL is assembled at compile time (`concat!`), never from user input: sort orders pick
//! one of several constant statements, filters are bound parameters.

use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::model::{Album, Artist, LibrarySort, ListStats, Page, Playlist, ProviderId, SourceRef, Track};

/// Turns user text into a `LIKE` pattern (escaped, lowercase). `None` for empty input.
pub fn like_pattern(filter: Option<&str>) -> Option<String> {
    let text = filter?.trim();
    if text.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('%');
    for c in text.to_lowercase().chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    Some(out)
}

#[derive(sqlx::FromRow)]
struct TrackRow {
    id: String,
    title: String,
    artists_json: String,
    album: Option<String>,
    duration_ms: Option<i64>,
    artwork_url: Option<String>,
    isrc: Option<String>,
    added_at: Option<i64>,
    sources_json: String,
}

#[derive(serde::Deserialize)]
struct SourceJson {
    provider: String,
    item_id: String,
    url: String,
    available: i64,
}

fn to_track(row: TrackRow) -> Track {
    let sources: Vec<SourceJson> = serde_json::from_str(&row.sources_json).unwrap_or_default();
    let available = sources.iter().any(|s| s.available != 0);
    Track {
        id: row.id,
        title: row.title,
        artists: serde_json::from_str(&row.artists_json).unwrap_or_default(),
        album: row.album,
        duration_ms: row.duration_ms.and_then(|d| u32::try_from(d).ok()),
        artwork: row.artwork_url,
        isrc: row.isrc,
        sources: sources
            .into_iter()
            .filter_map(|s| {
                Some(SourceRef { provider: ProviderId::parse(&s.provider)?, item_id: s.item_id, url: s.url })
            })
            .collect(),
        available,
        added_at: row.added_at,
    }
}

macro_rules! liked_select {
    ($order:literal) => {
        concat!(
            "SELECT t.id, t.title, t.artists_json, t.album, t.duration_ms, t.artwork_url, t.isrc,
                    MAX(l.added_at) AS added_at,
                    (SELECT json_group_array(json_object('provider', s.provider, 'item_id', s.item_id,
                              'url', s.url, 'available', s.available))
                       FROM track_sources s WHERE s.track_id = t.id) AS sources_json
             FROM library l JOIN tracks t ON t.id = l.track_id
             WHERE l.kind = 'liked' AND (?1 IS NULL OR l.provider = ?1)
               AND (?2 IS NULL OR t.title_norm LIKE ?2 ESCAPE '\\'
                    OR lower(t.artists_json) LIKE ?2 ESCAPE '\\'
                    OR lower(COALESCE(t.album, '')) LIKE ?2 ESCAPE '\\')
             GROUP BY t.id
             ORDER BY ",
            $order,
            " LIMIT ?3 OFFSET ?4"
        )
    };
}

/// Liked songs page (merged across sources; one row per unified track).
pub async fn liked(
    pool: &SqlitePool,
    source: Option<ProviderId>,
    sort: LibrarySort,
    filter: Option<&str>,
    offset: u32,
    limit: u32,
) -> AppResult<Page<Track>> {
    let pattern = like_pattern(filter);
    let provider = source.map(ProviderId::as_str);
    let sql = match sort {
        LibrarySort::DateAdded => liked_select!("added_at DESC, t.title_norm ASC"),
        LibrarySort::Title => liked_select!("t.title_norm ASC, t.artist_norm ASC"),
        LibrarySort::Artist => liked_select!("t.artist_norm ASC, t.album ASC, t.title_norm ASC"),
    };
    let rows: Vec<TrackRow> = sqlx::query_as(sql)
        .bind(provider)
        .bind(pattern.as_deref())
        .bind(i64::from(limit.min(1000)))
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await?;
    let stats = liked_stats(pool, source, filter).await?;
    let items: Vec<Track> = rows.into_iter().map(to_track).collect();
    let end = offset.saturating_add(u32::try_from(items.len()).unwrap_or(u32::MAX));
    Ok(Page { next_offset: (end < stats.count).then_some(end), total: stats.count, items })
}

/// Count and total duration of the (filtered) liked list.
pub async fn liked_stats(pool: &SqlitePool, source: Option<ProviderId>, filter: Option<&str>) -> AppResult<ListStats> {
    const SQL: &str = concat!(
        "SELECT COUNT(*), COALESCE(SUM(d), 0) FROM (SELECT t.id, MAX(t.duration_ms) AS d ",
        "FROM library l JOIN tracks t ON t.id = l.track_id
     WHERE l.kind = 'liked' AND (?1 IS NULL OR l.provider = ?1)
       AND (?2 IS NULL OR t.title_norm LIKE ?2 ESCAPE '\\'
            OR lower(t.artists_json) LIKE ?2 ESCAPE '\\'
            OR lower(COALESCE(t.album, '')) LIKE ?2 ESCAPE '\\')",
        " GROUP BY t.id)"
    );
    let pattern = like_pattern(filter);
    let (count, duration): (i64, i64) = sqlx::query_as(SQL)
        .bind(source.map(ProviderId::as_str))
        .bind(pattern.as_deref())
        .fetch_one(pool)
        .await?;
    Ok(ListStats { count: u32::try_from(count).unwrap_or(u32::MAX), duration_ms: duration as f64 })
}

#[derive(sqlx::FromRow)]
struct AlbumRow {
    id: String,
    provider: String,
    item_id: String,
    name: String,
    artists_json: String,
    artwork_url: Option<String>,
    release_date: Option<String>,
    added_at: Option<i64>,
}

macro_rules! albums_select {
    ($order:literal) => {
        concat!(
            "SELECT a.id, a.provider, a.item_id, a.name, a.artists_json, a.artwork_url, a.release_date,
                    l.added_at
             FROM library l JOIN albums a ON a.id = l.album_id
             WHERE l.kind = 'saved_album' AND (?1 IS NULL OR l.provider = ?1)
               AND (?2 IS NULL OR lower(a.name) LIKE ?2 ESCAPE '\\' OR lower(a.artists_json) LIKE ?2 ESCAPE '\\')
             ORDER BY ",
            $order
        )
    };
}

pub async fn albums(pool: &SqlitePool, source: Option<ProviderId>, sort: LibrarySort, filter: Option<&str>) -> AppResult<Vec<Album>> {
    let sql = match sort {
        LibrarySort::DateAdded => albums_select!("l.added_at DESC, lower(a.name)"),
        LibrarySort::Title => albums_select!("lower(a.name), lower(a.artists_json)"),
        LibrarySort::Artist => albums_select!("lower(a.artists_json), a.release_date DESC"),
    };
    let rows: Vec<AlbumRow> = sqlx::query_as(sql)
        .bind(source.map(ProviderId::as_str))
        .bind(like_pattern(filter).as_deref())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(Album {
                provider: ProviderId::parse(&r.provider)?,
                id: r.id,
                item_id: r.item_id,
                name: r.name,
                artists: serde_json::from_str(&r.artists_json).unwrap_or_default(),
                artwork: r.artwork_url,
                release_date: r.release_date,
                added_at: r.added_at,
            })
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct ArtistRow {
    id: String,
    provider: String,
    item_id: String,
    name: String,
    artwork_url: Option<String>,
}

macro_rules! artists_select {
    ($order:literal) => {
        concat!(
            "SELECT a.id, a.provider, a.item_id, a.name, a.artwork_url
             FROM library l JOIN artists a ON a.provider = l.provider AND a.item_id = l.ref_id
             WHERE l.kind = 'followed_artist' AND (?1 IS NULL OR l.provider = ?1)
               AND (?2 IS NULL OR a.name_norm LIKE ?2 ESCAPE '\\')
             ORDER BY ",
            $order
        )
    };
}

/// Followed artists / subscriptions. `DateAdded` keeps the provider's follow order.
pub async fn artists(pool: &SqlitePool, source: Option<ProviderId>, sort: LibrarySort, filter: Option<&str>) -> AppResult<Vec<Artist>> {
    let sql = match sort {
        LibrarySort::DateAdded => artists_select!("l.rowid DESC"),
        LibrarySort::Title | LibrarySort::Artist => artists_select!("a.name_norm"),
    };
    let rows: Vec<ArtistRow> = sqlx::query_as(sql)
        .bind(source.map(ProviderId::as_str))
        .bind(like_pattern(filter).as_deref())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(Artist {
                provider: ProviderId::parse(&r.provider)?,
                id: r.id,
                item_id: r.item_id,
                name: r.name,
                artwork: r.artwork_url,
            })
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct PlaylistRow {
    id: String,
    provider: Option<String>,
    name: String,
    owner: Option<String>,
    track_count: i64,
    artwork_url: Option<String>,
    editable: bool,
}

macro_rules! playlists_select {
    ($order:literal) => {
        concat!(
            "SELECT id, provider, name, owner, track_count, artwork_url, editable
             FROM playlists
             WHERE (?1 IS NULL OR provider = ?1)
               AND (?2 IS NULL OR lower(name) LIKE ?2 ESCAPE '\\' OR lower(COALESCE(owner, '')) LIKE ?2 ESCAPE '\\')
             ORDER BY ",
            $order
        )
    };
}

/// Playlists from every source plus Unison playlists (`provider` NULL). `DateAdded` = most
/// recently synced/created first; `Artist` sorts by owner.
pub async fn playlists(pool: &SqlitePool, source: Option<ProviderId>, sort: LibrarySort, filter: Option<&str>) -> AppResult<Vec<Playlist>> {
    let sql = match sort {
        LibrarySort::DateAdded => playlists_select!("rowid ASC"),
        LibrarySort::Title => playlists_select!("lower(name)"),
        LibrarySort::Artist => playlists_select!("lower(COALESCE(owner, '')), lower(name)"),
    };
    let rows: Vec<PlaylistRow> = sqlx::query_as(sql)
        .bind(source.map(ProviderId::as_str))
        .bind(like_pattern(filter).as_deref())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| Playlist {
            id: r.id,
            provider: r.provider.as_deref().and_then(ProviderId::parse),
            name: r.name,
            owner: r.owner.unwrap_or_default(),
            track_count: u32::try_from(r.track_count).unwrap_or(0),
            artwork: r.artwork_url,
            editable: r.editable,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::{self, library as lib};
    use crate::core::db::library::{AlbumInput, ArtistInput, LibraryKind, PlaylistInput, TrackInput};

    fn track(id: &str, title: &str, artist: &str, dur: u32) -> TrackInput {
        TrackInput {
            item_id: id.into(),
            url: format!("https://open.spotify.com/track/{id}"),
            title: title.into(),
            artists: vec![artist.into()],
            album: Some(format!("{artist} album")),
            duration_ms: Some(dur),
            isrc: None,
            artwork: None,
            available: id != "t3",
        }
    }

    async fn seeded() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut c = pool.acquire().await.unwrap();
        let p = ProviderId::Spotify;
        for (i, (id, title, artist)) in [("t1", "Bravo", "Zed"), ("t2", "alpha 100%", "Mia"), ("t3", "Charlie", "Ann")].iter().enumerate() {
            let tid = lib::upsert_track(&mut c, p, &track(id, title, artist, 1000 * (i as u32 + 1)), 0).await.unwrap();
            lib::upsert_library(&mut c, p, LibraryKind::Liked, id, Some(&tid), Some(100 + i as i64)).await.unwrap();
        }
        let al = lib::upsert_album(&mut c, p, &AlbumInput { item_id: "al".into(), name: "Demo".into(), artists: vec!["Zed".into()], artwork: None, release_date: None }).await.unwrap();
        lib::upsert_library(&mut c, p, LibraryKind::SavedAlbum, "al", Some(&al), Some(5)).await.unwrap();
        lib::upsert_artist(&mut c, p, &ArtistInput { item_id: "ar".into(), name: "Zed".into(), artwork: None }).await.unwrap();
        lib::upsert_library(&mut c, p, LibraryKind::FollowedArtist, "ar", None, None).await.unwrap();
        lib::upsert_playlist(&mut c, p, &PlaylistInput { item_id: "pl".into(), name: "Mix".into(), owner: "Me".into(), owner_id: "me".into(), editable: true, artwork: None, snapshot: None, track_count: 7 }, 0).await.unwrap();
        drop(c);
        (dir, pool)
    }

    #[tokio::test]
    async fn liked_sorts_filters_pages_and_reports_stats() {
        let (_d, pool) = seeded().await;
        let titles = |p: &Page<Track>| p.items.iter().map(|t| t.title.clone()).collect::<Vec<_>>();

        let by_date = liked(&pool, None, LibrarySort::DateAdded, None, 0, 50).await.unwrap();
        assert_eq!(titles(&by_date), vec!["Charlie", "alpha 100%", "Bravo"]);
        assert_eq!((by_date.total, by_date.next_offset), (3, None));
        assert!(!by_date.items[0].available, "t3 is unavailable");
        assert_eq!(by_date.items[0].sources.len(), 1);
        assert_eq!(by_date.items[0].added_at, Some(102));

        let by_title = liked(&pool, None, LibrarySort::Title, None, 0, 50).await.unwrap();
        assert_eq!(titles(&by_title), vec!["alpha 100%", "Bravo", "Charlie"]);
        let by_artist = liked(&pool, None, LibrarySort::Artist, None, 0, 50).await.unwrap();
        assert_eq!(titles(&by_artist), vec!["Charlie", "alpha 100%", "Bravo"]);

        let page = liked(&pool, None, LibrarySort::Title, None, 0, 2).await.unwrap();
        assert_eq!((page.items.len(), page.next_offset, page.total), (2, Some(2), 3));

        // Filter matches title, artist, album; `%` is literal.
        assert_eq!(titles(&liked(&pool, None, LibrarySort::Title, Some("MIA"), 0, 50).await.unwrap()), vec!["alpha 100%"]);
        assert_eq!(liked(&pool, None, LibrarySort::Title, Some("100%"), 0, 50).await.unwrap().total, 1);
        assert_eq!(liked(&pool, None, LibrarySort::Title, Some("%"), 0, 50).await.unwrap().total, 1);
        assert_eq!(liked(&pool, Some(ProviderId::Youtube), LibrarySort::Title, None, 0, 50).await.unwrap().total, 0);

        let stats = liked_stats(&pool, None, None).await.unwrap();
        assert_eq!((stats.count, stats.duration_ms), (3, 6000.0));
    }

    #[tokio::test]
    async fn grids_list_albums_artists_playlists() {
        let (_d, pool) = seeded().await;
        let a = albums(&pool, None, LibrarySort::DateAdded, None).await.unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].artists, vec!["Zed"]);
        assert!(albums(&pool, None, LibrarySort::Title, Some("nope")).await.unwrap().is_empty());
        let ar = artists(&pool, Some(ProviderId::Spotify), LibrarySort::Title, Some("ze")).await.unwrap();
        assert_eq!(ar.len(), 1);
        let pl = playlists(&pool, None, LibrarySort::Title, None).await.unwrap();
        assert_eq!((pl.len(), pl[0].track_count, pl[0].editable), (1, 7, true));
    }

    #[test]
    fn like_patterns_escape_wildcards() {
        assert_eq!(like_pattern(Some("  ")), None);
        assert_eq!(like_pattern(Some("A_b%")).as_deref(), Some("%a\\_b\\%%"));
    }
}
