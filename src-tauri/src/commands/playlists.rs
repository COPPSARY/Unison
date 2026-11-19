//! Playlist detail commands. Loading logic lives in `core::playlists` / `providers/`.

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::core::db::playlists;
use crate::error::{AppError, AppResult};
use crate::model::{PlaylistDetail, TrackPage};
use crate::state::AppState;

/// Header data for a playlist.
#[tauri::command]
#[specta::specta]
pub async fn playlist_get(state: State<'_, AppState>, playlist_id: String) -> AppResult<PlaylistDetail> {
    playlists::detail(&state.db, &playlist_id).await
}

/// Stored items in order. The first page loads them from the platform if never loaded
/// (spec §5: "Local; fetches if never synced").
#[tauri::command]
#[specta::specta]
pub async fn playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: String,
    offset: u32,
    limit: u32,
) -> AppResult<TrackPage> {
    if offset == 0 {
        state.playlist_loader().ensure_items(&playlist_id).await?;
    }
    playlists::tracks(&state.db, &playlist_id, offset, limit).await.map(TrackPage::from)
}

/// Opens the playlist on its platform in the default browser.
#[tauri::command]
#[specta::specta]
pub async fn playlist_open_external(app: AppHandle, state: State<'_, AppState>, playlist_id: String) -> AppResult<()> {
    let row = playlists::get_row(&state.db, &playlist_id).await?;
    let url = row.url().ok_or_else(|| AppError::not_found("playlist has no external link"))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::internal(format!("open playlist: {e}")))
}
