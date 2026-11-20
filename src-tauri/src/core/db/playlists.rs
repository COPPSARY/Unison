//! Playlist detail reads (spec §5 `playlist_tracks`): metadata + stored items.

use sqlx::SqlitePool;

use super::views::{to_track, TrackRow};
use crate::error::{AppError, AppResult};
use crate::model::{Page, Playlist, PlaylistDetail, Track};

/// A playlist row.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PlaylistRow {
    pub id: String,
    pub provider: Option<String>,
    pub item_id: Option<String>,
    pub name: String,
    pub owner: Option<String>,
    pub editable: bool,
    pub artwork_url: Option<String>,
    pub track_count: i64,
    pub items_synced: bool,
}

pub async fn get_row(pool: &SqlitePool, id: &str) -> AppResult<PlaylistRow> {
    sqlx::query_as(
        "SELECT id, provider, item_id, name, owner, editable, artwork_url, track_count, items_synced
         FROM playlists WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("playlist not found"))
}

pub async fn detail(pool: &SqlitePool, id: &str) -> AppResult<PlaylistDetail> {
    let row = get_row(pool, id).await?;
    let duration: Option<i64> = if row.items_synced {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(t.duration_ms), 0) FROM playlist_items pi
             JOIN tracks t ON t.id = pi.track_id WHERE pi.playlist_id = ?1",
        )
        .bind(id)
        .fetch_one(pool)
        .await?
    } else {
        None
    };
    Ok(PlaylistDetail {
        duration_ms: duration.map(|d| d as f64),
        playlist: Playlist {
            provider: row.provider,
            id: row.id,
            name: row.name,
            owner: row.owner.unwrap_or_default(),
            track_count: u32::try_from(row.track_count).unwrap_or(0),
            artwork: row.artwork_url,
            editable: row.editable,
        },
    })
}

/// Stored items in playlist order.
pub async fn tracks(pool: &SqlitePool, id: &str, offset: u32, limit: u32) -> AppResult<Page<Track>> {
    let rows: Vec<TrackRow> = sqlx::query_as(
        "SELECT t.id, t.title, t.artists_json, t.album, t.duration_ms, t.artwork_url, t.isrc,
                pi.added_at,
                (SELECT json_group_array(json_object('provider', s.provider, 'item_id', s.item_id,
                          'url', s.url, 'available', s.available))
                   FROM track_sources s WHERE s.track_id = t.id) AS sources_json
         FROM playlist_items pi JOIN tracks t ON t.id = pi.track_id
         WHERE pi.playlist_id = ?1
         ORDER BY pi.position
         LIMIT ?2 OFFSET ?3",
    )
    .bind(id)
    .bind(i64::from(limit.min(1000)))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let total = u32::try_from(total).unwrap_or(u32::MAX);
    let items: Vec<Track> = rows.into_iter().map(to_track).collect();
    let end = offset.saturating_add(u32::try_from(items.len()).unwrap_or(u32::MAX));
    Ok(Page { next_offset: (end < total).then_some(end), total, items })
}

