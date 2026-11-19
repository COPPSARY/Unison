//! Spotify playlist items (`GET /playlists/{id}/items`, Feb 2026). Only owned or collaborative
//! playlists return items; callers check that first.

use sqlx::SqlitePool;

use super::client::{Paging, SpotifyClient};
use super::map;
use super::models::PlaylistItem;
use crate::core::auth::oauth::now_unix;
use crate::core::db::library as lib;
use crate::core::db::playlists::PlaylistRow;
use crate::error::{AppError, AppResult};
use crate::model::ProviderId;

/// 200 pages × 100 = 20,000 items (above Spotify's 10,000-item playlist limit).
const MAX_PAGES: usize = 200;

/// Fetches every item of `row` and replaces the stored items. Returns how many were stored.
pub async fn fetch_items(client: &SpotifyClient, db: &SqlitePool, row: &PlaylistRow) -> AppResult<u32> {
    let item_id = row
        .item_id
        .as_deref()
        .ok_or_else(|| AppError::internal("spotify playlist without item id"))?;
    let mut inputs = Vec::new();
    let mut next = Some(format!("/playlists/{item_id}/items?limit=100"));
    let mut pages = 0usize;
    while let Some(url) = next.take() {
        if pages >= MAX_PAGES {
            break;
        }
        let page: Paging<PlaylistItem> = client.get_json(&url).await?;
        pages += 1;
        for entry in page.items {
            // Removed tracks come back as `item: null`; episodes are skipped by the mapper.
            let Some(track) = entry.item.as_ref().and_then(map::track) else { continue };
            inputs.push((track, map::parse_time(entry.added_at.as_deref())));
        }
        next = page.next;
    }

    let now = now_unix();
    let mut tx = db.begin().await?;
    let mut items = Vec::with_capacity(inputs.len());
    for (track, added_at) in &inputs {
        let id = lib::upsert_track(&mut tx, ProviderId::Spotify, track, now).await?;
        items.push((id, *added_at));
    }
    lib::replace_playlist_items(&mut tx, &row.id, ProviderId::Spotify, &items, now).await?;
    tx.commit().await?;
    tracing::debug!(count = items.len(), "spotify playlist items stored");
    Ok(u32::try_from(items.len()).unwrap_or(u32::MAX))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::core::auth::keychain::{keys, set_json, MemoryStore, StoredTokens};
    use crate::core::db::{self, library::PlaylistInput, playlists};
    use crate::providers::spotify::client::RetryPolicy;

    const FIXTURE: &str = include_str!("../../../../tests/fixtures/spotify/playlist_items.json");

    pub(crate) async fn setup() -> (tempfile::TempDir, SqlitePool, MockServer, SpotifyClient, String) {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let (id, _) = lib::upsert_playlist(
            &mut conn,
            ProviderId::Spotify,
            &PlaylistInput {
                item_id: "pl_owned".into(),
                name: "Late night".into(),
                owner: "Demo".into(),
                owner_id: "demo-user".into(),
                editable: true,
                artwork: None,
                snapshot: Some("s1".into()),
                track_count: 5,
            },
            0,
        )
        .await
        .unwrap();
        drop(conn);
        let server = MockServer::start().await;
        let secrets = Arc::new(MemoryStore::default());
        set_json(secrets.as_ref(), keys::SPOTIFY_TOKENS, &StoredTokens {
            access_token: "AT".into(),
            refresh_token: None,
            expires_at: now_unix() + 3600,
            scope: None,
        })
        .unwrap();
        let client = SpotifyClient::with_endpoints(
            reqwest::Client::new(),
            secrets,
            pool.clone(),
            &server.uri(),
            &server.uri(),
            RetryPolicy { backoff: vec![Duration::from_millis(1)], retry_after_unit: Duration::from_millis(1) },
        );
        (dir, pool, server, client, id)
    }

    #[tokio::test]
    async fn stores_items_in_order_skipping_null_and_episodes() {
        let (_d, pool, server, client, id) = setup().await;
        let base = server.uri();
        let first: serde_json::Value = serde_json::from_str(&FIXTURE.replace("{{BASE}}", &base)).unwrap();
        Mock::given(method("GET"))
            .and(path("/playlists/pl_owned/items"))
            .and(query_param("limit", "100"))
            .and(wiremock::matchers::query_param_is_missing("offset"))
            .respond_with(ResponseTemplate::new(200).set_body_json(first))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/playlists/pl_owned/items"))
            .and(query_param("offset", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [{ "added_at": "2024-02-01T00:00:00Z", "track": {
                    "id": "legacy", "name": "Legacy shape", "artists": [{ "name": "Old API" }], "duration_ms": 1000, "type": "track"
                }}],
                "next": null, "total": 6
            })))
            .mount(&server)
            .await;

        let row = playlists::get_row(&pool, &id).await.unwrap();
        assert!(row.items_available());
        let stored = fetch_items(&client, &pool, &row).await.unwrap();
        assert_eq!(stored, 4, "2 tracks + 1 local file + 1 legacy-shape track; null and episode skipped");

        let page = playlists::tracks(&pool, &id, 0, 50).await.unwrap();
        let titles: Vec<_> = page.items.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["First song", "Second song", "My local file", "Legacy shape"]);
        assert!(!page.items[2].available);
        assert_eq!(page.items[0].added_at, Some(1_704_067_200));
        let detail = playlists::detail(&pool, &id).await.unwrap();
        assert_eq!(detail.playlist.track_count, 4);
        assert_eq!(detail.duration_ms, Some(200_000.0 + 180_000.0 + 120_000.0 + 1_000.0));
        assert_eq!(detail.url.as_deref(), Some("https://open.spotify.com/playlist/pl_owned"));
    }
}
