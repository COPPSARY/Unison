//! Spotify full sync (spec §8.5): profile → liked → playlists (metadata) → saved albums →
//! followed artists → top + recently played. Each page is written in one transaction;
//! removals are applied only after the complete page set was read.

use std::collections::HashSet;

use serde::de::DeserializeOwned;
use sqlx::SqlitePool;

use super::client::{Paging, SpotifyClient};
use super::models::{ArtistObj, FollowedArtists, PlayHistory, SavedAlbum, SavedTrack, SimplePlaylist, TrackObj};
use super::{map, Profile};
use crate::core::auth::oauth::now_unix;
use crate::core::db::library::{self as lib, basic_norm, LibraryKind, SignalRow};
use crate::core::sync::{CancelFlag, SyncProgress, SyncReporter, SyncStage, SyncSummary};
use crate::error::AppResult;
use crate::model::{ProviderId, SignalKind};

const P: ProviderId = ProviderId::Spotify;
/// Safety cap: 1,000 pages × 50 = 50,000 items per resource.
const MAX_PAGES: usize = 1000;

/// Signal weights (spec §8.3).
pub fn weight(kind: SignalKind) -> f64 {
    match kind {
        SignalKind::TopShort => 5.0,
        SignalKind::TopMedium => 3.0,
        SignalKind::TopLong | SignalKind::Recent | SignalKind::Replay | SignalKind::Followed => 2.0,
        SignalKind::Liked => 1.5,
        SignalKind::Play => 1.0,
        SignalKind::Skip => -1.5,
    }
}

/// Walks `next` links one page at a time.
struct Pager<'a> {
    client: &'a SpotifyClient,
    cancel: &'a CancelFlag,
    next: Option<String>,
    pages: usize,
    /// False if the page cap stopped the walk early (then removals must be skipped).
    complete: bool,
}

impl<'a> Pager<'a> {
    fn new(client: &'a SpotifyClient, cancel: &'a CancelFlag, first: &str) -> Self {
        Self { client, cancel, next: Some(first.to_owned()), pages: 0, complete: true }
    }

    async fn next_page<R: DeserializeOwned, T>(&mut self, extract: impl Fn(R) -> Paging<T>) -> AppResult<Option<Paging<T>>> {
        let Some(url) = self.next.take() else { return Ok(None) };
        self.cancel.check()?;
        if self.pages >= MAX_PAGES {
            self.complete = false;
            tracing::warn!("page cap reached; removals skipped for this resource");
            return Ok(None);
        }
        let page = extract(self.client.get_json::<R>(&url).await?);
        self.pages += 1;
        self.next = page.next.clone();
        Ok(Some(page))
    }
}

fn progress(reporter: &dyn SyncReporter, stage: SyncStage, done: usize, total: Option<u32>) {
    reporter.progress(SyncProgress {
        provider: P,
        stage,
        done: u32::try_from(done).unwrap_or(u32::MAX),
        total,
    });
}

async fn existing_refs(db: &SqlitePool, kind: &str) -> AppResult<HashSet<String>> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT ref_id FROM library WHERE provider = 'spotify' AND kind = ?1")
        .bind(kind)
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().collect())
}

fn count_new(seen: &HashSet<String>, before: &HashSet<String>) -> u32 {
    u32::try_from(seen.difference(before).count()).unwrap_or(u32::MAX)
}

/// Runs a full Spotify sync and returns how many library items were added and removed.
pub async fn run(
    client: &SpotifyClient,
    db: &SqlitePool,
    reporter: &dyn SyncReporter,
    cancel: &CancelFlag,
) -> AppResult<SyncSummary> {
    let now = now_unix();
    let mut summary = SyncSummary::default();

    // Profile: refreshes the display name and gives the user id for playlist ownership.
    progress(reporter, SyncStage::Profile, 0, None);
    let me: Profile = client.get_json("/me").await?;
    sqlx::query("UPDATE accounts SET display_name = ?1, account_id = ?2 WHERE provider = 'spotify'")
        .bind(me.display_name.as_deref())
        .bind(&me.id)
        .execute(db)
        .await?;

    // Liked songs.
    let before = existing_refs(db, "liked").await?;
    let mut seen = HashSet::new();
    let mut done = 0usize;
    let mut pager = Pager::new(client, cancel, "/me/tracks?limit=50");
    while let Some(page) = pager.next_page(|p: Paging<SavedTrack>| p).await? {
        let mut tx = db.begin().await?;
        for item in &page.items {
            done += 1;
            let Some(track) = item.track.as_ref().and_then(map::track) else { continue };
            let id = lib::upsert_track(&mut tx, P, &track, now).await?;
            let added = map::parse_time(item.added_at.as_deref());
            lib::upsert_library(&mut tx, P, LibraryKind::Liked, &track.item_id, Some(&id), added).await?;
            seen.insert(track.item_id);
        }
        tx.commit().await?;
        progress(reporter, SyncStage::Liked, done, page.total);
    }
    summary.added += count_new(&seen, &before);
    if pager.complete {
        let mut conn = db.acquire().await?;
        summary.removed += lib::prune_library(&mut conn, P, LibraryKind::Liked, &seen).await?;
        lib::set_sync_state(&mut conn, P, "liked", now).await?;
    }

    // Playlists (metadata only; items load lazily in T-15).
    let mut seen = HashSet::new();
    let mut pager = Pager::new(client, cancel, "/me/playlists?limit=50");
    while let Some(page) = pager.next_page(|p: Paging<SimplePlaylist>| p).await? {
        let mut tx = db.begin().await?;
        for p in &page.items {
            lib::upsert_playlist(&mut tx, P, &map::playlist(p, &me.id), now).await?;
            seen.insert(p.id.clone());
        }
        tx.commit().await?;
        progress(reporter, SyncStage::Playlists, seen.len(), page.total);
    }
    if pager.complete {
        let mut conn = db.acquire().await?;
        lib::prune_playlists(&mut conn, P, &seen).await?;
        lib::set_sync_state(&mut conn, P, "playlists", now).await?;
    }

    // Saved albums.
    let before = existing_refs(db, "saved_album").await?;
    let mut seen = HashSet::new();
    let mut pager = Pager::new(client, cancel, "/me/albums?limit=50");
    while let Some(page) = pager.next_page(|p: Paging<SavedAlbum>| p).await? {
        let mut tx = db.begin().await?;
        for item in &page.items {
            let album_id = lib::upsert_album(&mut tx, P, &map::album(&item.album)).await?;
            let added = map::parse_time(item.added_at.as_deref());
            lib::upsert_library(&mut tx, P, LibraryKind::SavedAlbum, &item.album.id, Some(&album_id), added).await?;
            seen.insert(item.album.id.clone());
        }
        tx.commit().await?;
        progress(reporter, SyncStage::Albums, seen.len(), page.total);
    }
    summary.added += count_new(&seen, &before);
    if pager.complete {
        let mut conn = db.acquire().await?;
        summary.removed += lib::prune_library(&mut conn, P, LibraryKind::SavedAlbum, &seen).await?;
        lib::set_sync_state(&mut conn, P, "albums", now).await?;
    }

    // Followed artists (cursor paging inside `artists`).
    let before = existing_refs(db, "followed_artist").await?;
    let mut seen = HashSet::new();
    let mut signals: Vec<SignalRow> = Vec::new();
    let mut pager = Pager::new(client, cancel, "/me/following?type=artist&limit=50");
    while let Some(page) = pager.next_page(|f: FollowedArtists| f.artists).await? {
        let mut tx = db.begin().await?;
        for a in &page.items {
            lib::upsert_artist(&mut tx, P, &map::artist(a)).await?;
            lib::upsert_library(&mut tx, P, LibraryKind::FollowedArtist, &a.id, None, None).await?;
            signals.push(SignalRow {
                track: false,
                subject_id: basic_norm(&a.name),
                kind: SignalKind::Followed,
                weight: weight(SignalKind::Followed),
                at: now,
            });
            seen.insert(a.id.clone());
        }
        tx.commit().await?;
        progress(reporter, SyncStage::Artists, seen.len(), page.total);
    }
    summary.added += count_new(&seen, &before);
    if pager.complete {
        let mut conn = db.acquire().await?;
        summary.removed += lib::prune_library(&mut conn, P, LibraryKind::FollowedArtist, &seen).await?;
        lib::set_sync_state(&mut conn, P, "artists", now).await?;
    }

    // Taste: top tracks/artists per time range + recently played → signals (spec §8.3).
    progress(reporter, SyncStage::Taste, 0, None);
    for (range, kind) in [
        ("short_term", SignalKind::TopShort),
        ("medium_term", SignalKind::TopMedium),
        ("long_term", SignalKind::TopLong),
    ] {
        cancel.check()?;
        let tracks: Paging<TrackObj> = client.get_json(&format!("/me/top/tracks?time_range={range}&limit=50")).await?;
        let mut tx = db.begin().await?;
        for t in tracks.items.iter().filter_map(map::track) {
            let id = lib::upsert_track(&mut tx, P, &t, now).await?;
            signals.push(SignalRow { track: true, subject_id: id, kind, weight: weight(kind), at: now });
        }
        tx.commit().await?;
        let artists: Paging<ArtistObj> = client.get_json(&format!("/me/top/artists?time_range={range}&limit=50")).await?;
        signals.extend(artists.items.iter().map(|a| SignalRow {
            track: false,
            subject_id: basic_norm(&a.name),
            kind,
            weight: weight(kind),
            at: now,
        }));
    }
    cancel.check()?;
    let recent: Paging<PlayHistory> = client.get_json("/me/player/recently-played?limit=50").await?;
    let mut tx = db.begin().await?;
    for item in &recent.items {
        let Some(t) = item.track.as_ref().and_then(map::track) else { continue };
        let id = lib::upsert_track(&mut tx, P, &t, now).await?;
        let at = map::parse_time(item.played_at.as_deref()).unwrap_or(now);
        signals.push(SignalRow {
            track: true,
            subject_id: id,
            kind: SignalKind::Recent,
            weight: weight(SignalKind::Recent),
            at,
        });
    }
    lib::replace_signals(
        &mut tx,
        &[SignalKind::TopShort, SignalKind::TopMedium, SignalKind::TopLong, SignalKind::Recent, SignalKind::Followed],
        &signals,
    )
    .await?;
    lib::set_sync_state(&mut tx, P, "taste", now).await?;
    lib::set_last_synced(&mut tx, P, now).await?;
    tx.commit().await?;
    let n = signals.len();
    progress(reporter, SyncStage::Taste, n, Some(u32::try_from(n).unwrap_or(u32::MAX)));

    tracing::info!(added = summary.added, removed = summary.removed, "spotify sync finished");
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::{json, Value};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use crate::core::auth::keychain::{keys, set_json, MemoryStore, SecretStore, StoredTokens};
    use crate::core::db;
    use crate::core::sync::RecordingReporter;
    use crate::providers::spotify::client::RetryPolicy;

    const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/spotify/");

    fn fixture(name: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(format!("{FIX}{name}")).unwrap()).unwrap()
    }

    fn track_json(i: usize) -> Value {
        if i == 7 {
            return json!({
                "id": null, "uri": format!("spotify:local:Me:Demo:Local+{i}:180"),
                "name": "Local file", "is_local": true, "artists": [], "duration_ms": 180000
            });
        }
        json!({
            "id": format!("t{i:05}"),
            "uri": format!("spotify:track:t{i:05}"),
            "name": format!("Song {i}"),
            "artists": [{ "id": "ar_1", "name": "Demo Artist" }],
            "album": { "id": "al_1", "name": "Demo Album", "images": [{ "url": "https://i.scdn.co/image/a", "width": 300, "height": 300 }] },
            "duration_ms": 200000 + i,
            "external_ids": { "isrc": format!("USDEMO{i:06}") },
            "external_urls": { "spotify": format!("https://open.spotify.com/track/t{i:05}") },
            "is_local": false
        })
    }

    /// Serves `/me/tracks` as offset pages of 50 over `total` items. Item 5 is a removed (null)
    /// track, item 7 a local file.
    struct LikedPages {
        total: usize,
        base: String,
    }

    impl Respond for LikedPages {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let offset: usize = req
                .url
                .query_pairs()
                .find(|(k, _)| k == "offset")
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0);
            let end = (offset + 50).min(self.total);
            let items: Vec<Value> = (offset..end)
                .map(|i| {
                    let track = if i == 5 { Value::Null } else { track_json(i) };
                    json!({ "added_at": "2024-01-02T03:04:05Z", "track": track })
                })
                .collect();
            let next = (end < self.total).then(|| format!("{}/me/tracks?offset={end}&limit=50", self.base));
            ResponseTemplate::new(200).set_body_json(json!({
                "items": items, "next": next, "total": self.total, "limit": 50, "offset": offset
            }))
        }
    }

    async fn mount_spotify(server: &MockServer, liked: usize, playlists: Value) {
        server.reset().await;
        let ok = |v: Value| ResponseTemplate::new(200).set_body_json(v);
        Mock::given(method("GET")).and(path("/me")).respond_with(ok(fixture("me.json"))).mount(server).await;
        Mock::given(method("GET"))
            .and(path("/me/tracks"))
            .respond_with(LikedPages { total: liked, base: server.uri() })
            .mount(server)
            .await;
        Mock::given(method("GET")).and(path("/me/playlists")).respond_with(ok(playlists)).mount(server).await;
        Mock::given(method("GET")).and(path("/me/albums")).respond_with(ok(fixture("albums.json"))).mount(server).await;
        Mock::given(method("GET"))
            .and(path("/me/following"))
            .and(query_param("type", "artist"))
            .respond_with(ok(fixture("following.json")))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/top/tracks"))
            .respond_with(ok(json!({ "items": [track_json(1), track_json(9000)], "next": null, "total": 2 })))
            .mount(server)
            .await;
        Mock::given(method("GET")).and(path("/me/top/artists")).respond_with(ok(fixture("top_artists.json"))).mount(server).await;
        Mock::given(method("GET"))
            .and(path("/me/player/recently-played"))
            .respond_with(ok(fixture("recently_played.json")))
            .mount(server)
            .await;
    }

    async fn scalar(db: &SqlitePool, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
    }

    fn test_client(server: &MockServer, db: &SqlitePool, refresh: Option<&str>) -> SpotifyClient {
        let secrets = Arc::new(MemoryStore::default());
        secrets.set(keys::SPOTIFY_CLIENT_ID, "0123456789abcdef0123456789abcdef").unwrap();
        set_json(
            secrets.as_ref(),
            keys::SPOTIFY_TOKENS,
            &StoredTokens {
                access_token: "AT".into(),
                refresh_token: refresh.map(str::to_owned),
                expires_at: now_unix() + 3600,
                scope: None,
            },
        )
        .unwrap();
        SpotifyClient::with_endpoints(
            reqwest::Client::new(),
            secrets,
            db.clone(),
            &server.uri(),
            &format!("{}/api/token", server.uri()),
            RetryPolicy { backoff: vec![Duration::from_millis(1); 4], retry_after_unit: Duration::from_millis(1) },
        )
    }

    #[tokio::test]
    async fn full_sync_then_resync_with_removals() {
        let dir = tempfile::tempdir().unwrap();
        let db = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        db::accounts::upsert_connected(&db, ProviderId::Spotify, "demo-user", Some("Old name"), 0).await.unwrap();
        let server = MockServer::start().await;
        let client = test_client(&server, &db, Some("RT"));

        // First sync: 2,050 liked songs (one null → skipped, one local file → kept unavailable).
        mount_spotify(&server, 2050, fixture("playlists.json")).await;
        let reporter = RecordingReporter::default();
        let started = std::time::Instant::now();
        let summary = run(&client, &db, &reporter, &CancelFlag::default()).await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(30), "sync too slow: {:?}", started.elapsed());

        assert_eq!(summary.added, 2049 + 1 + 1, "liked + saved album + followed artist");
        assert_eq!(summary.removed, 0);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM library WHERE kind = 'liked'").await, 2049);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM track_sources WHERE available = 0").await, 1);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM playlists").await, 2);
        assert_eq!(scalar(&db, "SELECT editable FROM playlists WHERE item_id = 'pl_owned'").await, 1);
        assert_eq!(scalar(&db, "SELECT editable FROM playlists WHERE item_id = 'pl_followed'").await, 0);
        assert_eq!(scalar(&db, "SELECT track_count FROM playlists WHERE item_id = 'pl_owned'").await, 42);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM library WHERE kind = 'saved_album'").await, 1);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM library WHERE kind = 'followed_artist'").await, 1);
        assert!(scalar(&db, "SELECT COUNT(*) FROM signals WHERE kind = 'top_short'").await >= 4);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM signals WHERE kind = 'recent'").await, 1);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM accounts WHERE last_synced_at IS NOT NULL").await, 1);
        let name: String = sqlx::query_scalar("SELECT display_name FROM accounts").fetch_one(&db).await.unwrap();
        assert_eq!(name, "Demo Listener");

        let progress = reporter.progress.lock().unwrap().clone();
        let liked: Vec<_> = progress.iter().filter(|p| p.stage == SyncStage::Liked).collect();
        assert_eq!(liked.len(), 41, "one progress event per page");
        assert_eq!(liked.last().map(|p| (p.done, p.total)), Some((2050, Some(2050))));

        // Re-sync: 50 fewer liked songs and the followed playlist is gone.
        let tracks_before = scalar(&db, "SELECT COUNT(*) FROM tracks").await;
        let mut playlists = fixture("playlists.json");
        if let Some(items) = playlists["items"].as_array_mut() {
            items.truncate(1);
        }
        playlists["total"] = json!(1);
        mount_spotify(&server, 2000, playlists).await;
        let summary = run(&client, &db, &RecordingReporter::default(), &CancelFlag::default()).await.unwrap();
        assert_eq!(summary.added, 0);
        assert_eq!(summary.removed, 50);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM library WHERE kind = 'liked'").await, 1999);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM playlists").await, 1);
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM tracks").await, tracks_before, "tracks are kept");
    }

    #[tokio::test]
    async fn cancelled_sync_stops_without_writing_library() {
        let dir = tempfile::tempdir().unwrap();
        let db = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let server = MockServer::start().await;
        mount_spotify(&server, 120, fixture("playlists.json")).await;
        let client = test_client(&server, &db, None);
        let cancel = CancelFlag::default();
        cancel.cancel();
        assert!(run(&client, &db, &RecordingReporter::default(), &cancel).await.is_err());
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM library").await, 0);
    }
}
