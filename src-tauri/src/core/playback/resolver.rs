//! Resolver (addendum §1–§2): turns a unified track into ordered playable candidates using its
//! stored sources and the "Play from" priority. Only sources merged into the same `tracks` row
//! are ever candidates, so a fallback never plays a different song.

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::engine::{Candidates, PlaySource, Policy};
use crate::core::db;
use crate::error::AppResult;
use crate::model::{SourceKind, SpotifyMode};

pub fn kind_of(provider: &str) -> Option<SourceKind> {
    match provider {
        "spotify" => Some(SourceKind::Spotify),
        "youtube" => Some(SourceKind::Youtube),
        "local" => Some(SourceKind::Local),
        "audius" => Some(SourceKind::Audius),
        _ => None,
    }
}

/// A stored source of a track (available or not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSource {
    pub source: PlaySource,
    pub available: bool,
}

pub async fn stored_sources(pool: &SqlitePool, track_id: &str) -> AppResult<Vec<StoredSource>> {
    let rows: Vec<(String, String, String, bool)> =
        sqlx::query_as("SELECT provider, item_id, url, available FROM track_sources WHERE track_id = ?1")
            .bind(track_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(provider, item_id, url, available)| {
            Some(StoredSource { source: PlaySource { kind: kind_of(&provider)?, item_id, url }, available })
        })
        .collect())
}

/// Orders sources by `priority`, dropping unavailable ones and Spotify when disabled.
pub fn order(sources: Vec<StoredSource>, priority: &[SourceKind], spotify_mode: SpotifyMode) -> Vec<PlaySource> {
    let rank = |k: SourceKind| priority.iter().position(|p| *p == k).unwrap_or(priority.len());
    let mut usable: Vec<PlaySource> = sources
        .into_iter()
        .filter(|s| s.available)
        .map(|s| s.source)
        .filter(|s| !(s.kind == SourceKind::Spotify && spotify_mode == SpotifyMode::Never))
        .collect();
    usable.sort_by_key(|s| rank(s.kind));
    usable
}

pub struct Resolver {
    pub db: SqlitePool,
}

#[async_trait]
impl Candidates for Resolver {
    async fn candidates(&self, track_id: &str) -> AppResult<Vec<PlaySource>> {
        let settings = db::settings::load(&self.db).await?;
        let sources = stored_sources(&self.db, track_id).await?;
        Ok(order(sources, &settings.player.source_priority, settings.player.spotify_mode))
    }

    async fn policy(&self) -> Policy {
        match db::settings::load(&self.db).await {
            Ok(s) => Policy {
                auto_fallback: s.player.auto_fallback,
                fallback_on_error_midtrack: s.player.fallback_on_error_midtrack,
            },
            Err(_) => Policy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(kind: SourceKind, id: &str, available: bool) -> StoredSource {
        StoredSource { source: PlaySource { kind, item_id: id.into(), url: String::new() }, available }
    }

    #[test]
    fn orders_by_priority_and_drops_unavailable_and_disabled() {
        let sources = vec![
            s(SourceKind::Youtube, "yt", true),
            s(SourceKind::Spotify, "sp", true),
            s(SourceKind::Local, "gone", false),
            s(SourceKind::Audius, "au", true),
        ];
        let default = order(sources.clone(), &SourceKind::DEFAULT_PRIORITY, SpotifyMode::Auto);
        assert_eq!(default.iter().map(|c| c.item_id.as_str()).collect::<Vec<_>>(), vec!["sp", "yt", "au"]);

        let yt_first = order(sources.clone(), &[SourceKind::Youtube, SourceKind::Audius, SourceKind::Local, SourceKind::Spotify], SpotifyMode::Auto);
        assert_eq!(yt_first[0].item_id, "yt");

        let never = order(sources, &SourceKind::DEFAULT_PRIORITY, SpotifyMode::Never);
        assert!(never.iter().all(|c| c.kind != SourceKind::Spotify));
    }

    #[tokio::test]
    async fn reads_sources_of_one_track_from_the_db() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let input = db::library::TrackInput {
            item_id: "sp1".into(),
            url: "https://open.spotify.com/track/sp1".into(),
            title: "Song".into(),
            artists: vec!["A".into()],
            album: None,
            duration_ms: Some(1),
            isrc: None,
            artwork: None,
            available: true,
        };
        let id = db::library::upsert_track(&mut conn, crate::model::ProviderId::Spotify, &input, 0).await.unwrap();
        drop(conn);
        let resolver = Resolver { db: pool };
        let c = resolver.candidates(&id).await.unwrap();
        assert_eq!(c, vec![PlaySource { kind: SourceKind::Spotify, item_id: "sp1".into(), url: "https://open.spotify.com/track/sp1".into() }]);
        assert_eq!(resolver.policy().await, Policy::default());
    }
}
