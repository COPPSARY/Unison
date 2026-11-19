//! Library views (local DB only; spec §5).

use tauri::State;

use crate::core::db::views;
use crate::error::AppResult;
use crate::model::{Album, Artist, LibrarySort, ListStats, Playlist, ProviderId, TrackPage};
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn library_liked(
    state: State<'_, AppState>,
    source: Option<ProviderId>,
    sort: LibrarySort,
    filter: Option<String>,
    offset: u32,
    limit: u32,
) -> AppResult<TrackPage> {
    views::liked(&state.db, source, sort, filter.as_deref(), offset, limit).await.map(TrackPage::from)
}

#[tauri::command]
#[specta::specta]
pub async fn library_liked_stats(
    state: State<'_, AppState>,
    source: Option<ProviderId>,
    filter: Option<String>,
) -> AppResult<ListStats> {
    views::liked_stats(&state.db, source, filter.as_deref()).await
}

/// Library → Local files (tracks with an imported local file).
#[tauri::command]
#[specta::specta]
pub async fn library_local(
    state: State<'_, AppState>,
    sort: LibrarySort,
    filter: Option<String>,
    offset: u32,
    limit: u32,
) -> AppResult<TrackPage> {
    views::local(&state.db, sort, filter.as_deref(), offset, limit).await.map(TrackPage::from)
}

#[tauri::command]
#[specta::specta]
pub async fn library_local_stats(state: State<'_, AppState>, filter: Option<String>) -> AppResult<ListStats> {
    views::local_stats(&state.db, filter.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn library_albums(
    state: State<'_, AppState>,
    source: Option<ProviderId>,
    sort: LibrarySort,
    filter: Option<String>,
) -> AppResult<Vec<Album>> {
    views::albums(&state.db, source, sort, filter.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn library_artists(
    state: State<'_, AppState>,
    source: Option<ProviderId>,
    sort: LibrarySort,
    filter: Option<String>,
) -> AppResult<Vec<Artist>> {
    views::artists(&state.db, source, sort, filter.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlists_list(
    state: State<'_, AppState>,
    source: Option<ProviderId>,
    sort: LibrarySort,
    filter: Option<String>,
) -> AppResult<Vec<Playlist>> {
    views::playlists(&state.db, source, sort, filter.as_deref()).await
}
