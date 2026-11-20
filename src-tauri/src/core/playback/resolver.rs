//! Resolver (addendum §1–§2): turns a unified track into ordered playable candidates using its
//! stored sources and the "Play from" priority. Only sources merged into the same `tracks` row
//! are ever candidates, so a fallback never plays a different song.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;

use super::engine::{Candidates, PlaySource, Policy, SourceFinder};
use crate::core::db;
use crate::error::AppResult;
use crate::model::SourceKind;

pub fn kind_of(provider: &str) -> Option<SourceKind> {
    SourceKind::parse(provider)
}

/// A stored source of a track (available or not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSource {
    pub source: PlaySource,
    pub available: bool,
}

pub async fn stored_sources(pool: &SqlitePool, track_id: &str) -> AppResult<Vec<StoredSource>> {
    let rows: Vec<(String, String, String, bool)> =
        sqlx::query_as(
            "SELECT provider, item_id, url, available FROM track_sources WHERE track_id = ?1
             ORDER BY preference DESC, rowid",
        )
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

/// Orders sources by `priority`, dropping unavailable ones.
pub fn order(sources: Vec<StoredSource>, priority: &[SourceKind]) -> Vec<PlaySource> {
    let rank = |k: SourceKind| priority.iter().position(|p| *p == k).unwrap_or(priority.len());
    let mut usable: Vec<PlaySource> = sources
        .into_iter()
        .filter(|s| s.available)
        .map(|s| s.source)
        .collect();
    usable.sort_by_key(|s| rank(s.kind));
    usable
}

pub struct Resolver {
    pub db: SqlitePool,
    /// Looks for more sources when a track's own can't play (the extensions).
    pub finder: Option<Arc<dyn SourceFinder>>,
}

impl Resolver {
    pub fn new(db: SqlitePool) -> Self {
        Self { db, finder: None }
    }
}

#[async_trait]
impl Candidates for Resolver {
    async fn find_more(&self, track_id: &str) -> bool {
        match &self.finder {
            Some(finder) => finder.find_more(track_id).await,
            None => false,
        }
    }

    async fn candidates(&self, track_id: &str) -> AppResult<Vec<PlaySource>> {
        let settings = db::settings::load(&self.db).await?;
        let sources = stored_sources(&self.db, track_id).await?;
        Ok(order(sources, &settings.player.source_priority))
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
    fn orders_by_priority_and_drops_unavailable() {
        let sources = vec![
            s(SourceKind::Extension, "ex", true),
            s(SourceKind::Local, "gone", false),
            s(SourceKind::Audius, "au", true),
        ];
        let default = order(sources.clone(), &SourceKind::DEFAULT_PRIORITY);
        assert_eq!(default.iter().map(|c| c.item_id.as_str()).collect::<Vec<_>>(), vec!["au", "ex"]);

        let ext_first = order(sources, &[SourceKind::Extension, SourceKind::Audius, SourceKind::Local]);
        assert_eq!(ext_first[0].item_id, "ex");
    }

    #[tokio::test]
    async fn reads_sources_of_one_track_from_the_db() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let input = db::library::TrackInput {
            item_id: "demo:t1".into(),
            url: "https://music.example/t1".into(),
            title: "Song".into(),
            artists: vec!["A".into()],
            album: None,
            duration_ms: Some(1),
            isrc: None,
            artwork: None,
            available: true,
        };
        let id = db::library::upsert_track(&mut conn, "extension", &input, 0).await.unwrap();
        drop(conn);
        let resolver = Resolver::new(pool);
        let c = resolver.candidates(&id).await.unwrap();
        assert_eq!(c, vec![PlaySource { kind: SourceKind::Extension, item_id: "demo:t1".into(), url: "https://music.example/t1".into() }]);
        assert_eq!(resolver.policy().await, Policy::default());
    }
}
