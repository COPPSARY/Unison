//! Settings repository: one row per top-level `Settings` field, value stored as JSON.

use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::model::Settings;

/// Loads settings; unknown keys are ignored and missing or unreadable ones take defaults.
pub async fn load(pool: &SqlitePool) -> AppResult<Settings> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;

    let defaults = serde_json::to_value(Settings::default())
        .map_err(|e| AppError::internal(format!("settings default: {e}")))?;
    let Value::Object(mut merged) = defaults else {
        return Err(AppError::internal("settings default is not an object"));
    };

    // Apply stored fields one by one so a single bad value can't reset everything.
    for (key, raw) in rows {
        if !merged.contains_key(&key) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            tracing::warn!(%key, "ignoring unreadable setting");
            continue;
        };
        let mut candidate: Map<String, Value> = merged.clone();
        candidate.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(Value::Object(candidate.clone())).is_ok() {
            merged = candidate;
        } else {
            tracing::warn!(%key, "ignoring invalid setting value");
        }
    }

    serde_json::from_value::<Settings>(Value::Object(merged))
        .map(Settings::sanitized)
        .map_err(|e| AppError::internal(format!("settings: {e}")))
}

/// Saves all fields in one transaction and returns what was stored.
pub async fn save(pool: &SqlitePool, settings: Settings) -> AppResult<Settings> {
    let settings = settings.sanitized();
    let Value::Object(map) = serde_json::to_value(&settings)
        .map_err(|e| AppError::internal(format!("settings encode: {e}")))?
    else {
        return Err(AppError::internal("settings did not encode to an object"));
    };
    let mut tx = pool.begin().await?;
    for (key, value) in map {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value.to_string())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;
    use crate::model::{ProviderId, ReplayGainMode, SourceKind, SpotifyMode, Theme, WindowMaterial};

    async fn pool() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        (dir, pool)
    }

    #[tokio::test]
    async fn empty_db_gives_defaults() {
        let (_d, pool) = pool().await;
        assert_eq!(load(&pool).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn round_trips() {
        let (_d, pool) = pool().await;
        let wanted = Settings {
            theme: Theme::Light,
            glass_frost: 0.75,
            window_material: WindowMaterial::Acrylic,
            sidebar_collapsed: vec!["library".into()],
            sync_interval_min: 60,
            preferred_source: Some(ProviderId::Youtube),
            ..Settings::default()
        };
        let saved = save(&pool, wanted.clone()).await.unwrap();
        assert_eq!(saved, wanted);
        assert_eq!(load(&pool).await.unwrap(), wanted);
    }

    #[tokio::test]
    async fn player_local_audius_sections_round_trip_under_their_keys() {
        let (_d, pool) = pool().await;
        let mut wanted = Settings::default();
        wanted.player.source_priority = vec![SourceKind::Youtube, SourceKind::Local, SourceKind::Spotify, SourceKind::Audius];
        wanted.player.spotify_mode = SpotifyMode::Never;
        wanted.player.replaygain = ReplayGainMode::Album;
        wanted.player.crossfade_ms = 4_000;
        wanted.local.watched_folders = vec![r"C:\Users\me\Music".into()];
        wanted.audius.show_trending_row = true;
        save(&pool, wanted.clone()).await.unwrap();
        assert_eq!(load(&pool).await.unwrap(), wanted);

        let keys: Vec<String> = sqlx::query_scalar("SELECT key FROM settings ORDER BY key").fetch_all(&pool).await.unwrap();
        for k in ["audius", "local", "player"] {
            assert!(keys.iter().any(|x| x == k), "missing key {k}: {keys:?}");
        }
        // An older DB without these keys still loads, with defaults.
        sqlx::query("DELETE FROM settings WHERE key IN ('player', 'local', 'audius')").execute(&pool).await.unwrap();
        let s = load(&pool).await.unwrap();
        assert_eq!(s.player, crate::model::PlayerSettings::default());
    }

    #[tokio::test]
    async fn one_bad_field_keeps_the_others() {
        let (_d, pool) = pool().await;
        for (k, v) in [("theme", r#""light""#), ("syncIntervalMin", r#""oops""#), ("futureKey", "1")] {
            sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2)")
                .bind(k)
                .bind(v)
                .execute(&pool)
                .await
                .unwrap();
        }
        let s = load(&pool).await.unwrap();
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.sync_interval_min, 30);
    }
}
