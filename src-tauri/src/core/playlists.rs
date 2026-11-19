//! Playlist item loading (spec §8.5): lazily when a playlist is opened, and in the background
//! after a sync for owned playlists. Dispatches to the right provider; one fetch at a time.

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::core::db::playlists::{self, PlaylistRow};
use crate::error::AppResult;
use crate::model::ProviderId;
use crate::providers::spotify::{self, client::SpotifyClient};

/// Owned playlists prefetched per background run.
const PREFETCH_LIMIT: u32 = 50;

pub struct PlaylistLoader<'a> {
    pub db: &'a SqlitePool,
    pub spotify: &'a SpotifyClient,
    /// Serialises fetches so two opens never load the same playlist twice.
    pub lock: &'a Mutex<()>,
}

impl PlaylistLoader<'_> {
    /// Loads `id`'s items from its platform if they were never loaded (or the snapshot changed).
    /// No-op for playlists whose items the platform doesn't share.
    pub async fn ensure_items(&self, id: &str) -> AppResult<()> {
        let _guard = self.lock.lock().await;
        // Re-read under the lock: another task may have just loaded it.
        let row = playlists::get_row(self.db, id).await?;
        if !needs_fetch(&row) {
            return Ok(());
        }
        match row.provider() {
            Some(ProviderId::Spotify) => {
                spotify::playlists::fetch_items(self.spotify, self.db, &row).await?;
            }
            // YouTube playlist items arrive with T-22; Unison playlists are local.
            Some(ProviderId::Youtube) | None => {}
        }
        Ok(())
    }

    /// Background prefetch of owned Spotify playlists. Errors are logged, not raised.
    pub async fn prefetch_owned(&self) {
        let Ok(ids) = playlists::unsynced_owned(self.db, PREFETCH_LIMIT).await else { return };
        for id in ids {
            if let Err(err) = self.ensure_items(&id).await {
                tracing::warn!(error = %err, "playlist prefetch stopped");
                return;
            }
        }
    }
}

pub fn needs_fetch(row: &PlaylistRow) -> bool {
    !row.items_synced && row.items_available() && row.provider().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(provider: Option<&str>, editable: bool, synced: bool) -> PlaylistRow {
        PlaylistRow {
            id: "p".into(),
            provider: provider.map(str::to_owned),
            item_id: Some("x".into()),
            name: "P".into(),
            owner: None,
            editable,
            artwork_url: None,
            track_count: 0,
            items_synced: synced,
        }
    }

    #[test]
    fn fetch_rules() {
        assert!(needs_fetch(&row(Some("spotify"), true, false)));
        assert!(!needs_fetch(&row(Some("spotify"), true, true)), "already loaded");
        assert!(!needs_fetch(&row(Some("spotify"), false, false)), "followed: Spotify won't share items");
        assert!(!needs_fetch(&row(None, true, false)), "Unison playlists are local");
    }
}

#[cfg(test)]
mod loader_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use crate::core::db::library as lib;

    #[tokio::test]
    async fn loads_once_and_again_after_snapshot_change() {
        let (_d, pool, server, client, id) = crate::providers::spotify::playlists::tests::setup().await;
        Mock::given(method("GET"))
            .and(path("/playlists/pl_owned/items"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{ "added_at": null, "item": { "id": "x1", "name": "Only", "type": "track", "duration_ms": 1 } }],
                "next": null, "total": 1
            })))
            .expect(2)
            .mount(&server)
            .await;
        let lock = Mutex::new(());
        let loader = PlaylistLoader { db: &pool, spotify: &client, lock: &lock };

        loader.ensure_items(&id).await.unwrap();
        loader.ensure_items(&id).await.unwrap(); // already loaded: no request
        assert_eq!(playlists::tracks(&pool, &id, 0, 10).await.unwrap().total, 1);

        // A new snapshot from the next sync marks the items stale.
        let mut conn = pool.acquire().await.unwrap();
        lib::upsert_playlist(
            &mut conn,
            ProviderId::Spotify,
            &lib::PlaylistInput {
                item_id: "pl_owned".into(),
                name: "Late night".into(),
                owner: "Demo".into(),
                owner_id: "demo-user".into(),
                editable: true,
                artwork: None,
                snapshot: Some("s2".into()),
                track_count: 1,
            },
            1,
        )
        .await
        .unwrap();
        drop(conn);
        loader.ensure_items(&id).await.unwrap(); // refetched (second expected request)
    }

    #[tokio::test]
    async fn followed_playlists_never_request_items() {
        let (_d, pool, server, client, id) = crate::providers::spotify::playlists::tests::setup().await;
        sqlx::query("UPDATE playlists SET editable = 0 WHERE id = ?1").bind(&id).execute(&pool).await.unwrap();
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&server).await;
        let lock = Mutex::new(());
        PlaylistLoader { db: &pool, spotify: &client, lock: &lock }.ensure_items(&id).await.unwrap();
        let detail = playlists::detail(&pool, &id).await.unwrap();
        assert!(!detail.items_available);
        assert_eq!(detail.duration_ms, None);
    }
}
