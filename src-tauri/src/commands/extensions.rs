//! Extensions (EXT-01/02): saved registries, preview / install from a GitHub, package or
//! registry link, list, enable, remove, search and save results for playback. All logic lives
//! in core::extensions.

use tauri::{AppHandle, State};

use crate::core::extensions::songs::{SearchAll, SongDetail};
use crate::core::extensions::{ExtensionInfo, ExtensionPreview, ExtensionResults, RegistryPreview};
use crate::error::AppResult;
use crate::state::AppState;

/// Installed extensions, by name.
#[tauri::command]
#[specta::specta]
pub async fn extension_list(state: State<'_, AppState>) -> AppResult<Vec<ExtensionInfo>> {
    state.extensions.list().await
}

/// Reads the extension behind a GitHub or package link (pinned) without installing it.
#[tauri::command]
#[specta::specta]
pub async fn extension_preview(state: State<'_, AppState>, url: String) -> AppResult<ExtensionPreview> {
    state.extensions.preview(&url).await
}

/// Reads a registry.json link and previews every extension it lists, without installing any.
#[tauri::command]
#[specta::specta]
pub async fn extension_registry(state: State<'_, AppState>, url: String) -> AppResult<RegistryPreview> {
    state.extensions.registry(&url).await
}

/// Registry links added on the Extensions page.
#[tauri::command]
#[specta::specta]
pub async fn extension_registries(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.extensions.registries().await
}

/// Reads a registry link and saves it when it works.
#[tauri::command]
#[specta::specta]
pub async fn extension_registry_add(state: State<'_, AppState>, url: String) -> AppResult<RegistryPreview> {
    state.extensions.add_registry(&url).await
}

/// Forgets a saved registry (installed extensions stay). Returns the remaining links.
#[tauri::command]
#[specta::specta]
pub async fn extension_registry_remove(state: State<'_, AppState>, url: String) -> AppResult<Vec<String>> {
    state.extensions.remove_registry(&url).await
}

/// Installs the previewed extension at `sha` (commit or package SHA-256). Returns the list.
#[tauri::command]
#[specta::specta]
pub async fn extension_install(state: State<'_, AppState>, url: String, sha: String) -> AppResult<Vec<ExtensionInfo>> {
    state.extensions.install(&url, &sha).await
}

#[tauri::command]
#[specta::specta]
pub async fn extension_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<Vec<ExtensionInfo>> {
    state.extensions.set_enabled(&id, enabled).await
}

#[tauri::command]
#[specta::specta]
pub async fn extension_remove(state: State<'_, AppState>, id: String) -> AppResult<Vec<ExtensionInfo>> {
    state.extensions.remove(&id).await
}

/// Searches every enabled extension; each section carries its own error.
#[tauri::command]
#[specta::specta]
pub async fn extension_search(state: State<'_, AppState>, query: String) -> AppResult<Vec<ExtensionResults>> {
    state.extensions.search(&query).await
}

/// Saves search results (by source key) and returns their library track ids, same order.
#[tauri::command]
#[specta::specta]
pub async fn extension_save_tracks(state: State<'_, AppState>, keys: Vec<String>) -> AppResult<Vec<String>> {
    state.extensions.save_tracks(&keys).await
}

/// Searches every enabled extension and merges the results into one song list.
#[tauri::command]
#[specta::specta]
pub async fn extension_search_all(state: State<'_, AppState>, query: String) -> AppResult<SearchAll> {
    state.extensions.search_all(&query).await
}

/// The track page for a song (by any of its source keys).
#[tauri::command]
#[specta::specta]
pub async fn extension_song(state: State<'_, AppState>, key: String) -> AppResult<SongDetail> {
    state.extensions.song(&key).await
}

/// Plays a song from `source_key` (at `quality` for that extension when given).
#[tauri::command]
#[specta::specta]
pub async fn extension_play_song(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    source_key: String,
    quality: Option<String>,
) -> AppResult<()> {
    let id = state.extensions.prepare_play(&key, &source_key, quality.as_deref()).await?;
    let result = state.player.play(vec![id], 0, None).await;
    state.player.emit_state(&app).await;
    result
}

/// Likes or unlikes a song. Returns the new state.
#[tauri::command]
#[specta::specta]
pub async fn extension_like_song(state: State<'_, AppState>, key: String, liked: bool) -> AppResult<bool> {
    state.extensions.like_song(&key, liked).await
}
