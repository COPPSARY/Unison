//! Playlist detail commands (playlists are stored in SQLite).

use tauri::State;

use crate::core::db::playlists;
use crate::error::AppResult;
use crate::model::{PlaylistDetail, TrackPage};
use crate::state::AppState;

/// Header data for a playlist.
#[tauri::command]
#[specta::specta]
pub async fn playlist_get(state: State<'_, AppState>, playlist_id: String) -> AppResult<PlaylistDetail> {
    playlists::detail(&state.db, &playlist_id).await
}

/// Stored items in order.
#[tauri::command]
#[specta::specta]
pub async fn playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
    offset: u32,
    limit: u32,
) -> AppResult<TrackPage> {
    playlists::tracks(&state.db, &playlist_id, offset, limit).await.map(TrackPage::from)
}
