//! Player commands. Engine and resolver logic lives in `core::playback` / `providers/`.

use tauri::{AppHandle, Manager, State};

use crate::core::db;
use crate::core::playback::engine::Engine;
use crate::core::playback::engine::SourceStatus;
use crate::core::playback::native;
use crate::core::playback::service::{PlayerAction, PlayerStateEvent};
use crate::model::SourceKind;
use tauri_plugin_opener::OpenerExt;
use crate::error::{AppError, AppResult};
use crate::providers::spotify::player::{self, SpotifyCapabilities};
use crate::state::AppState;

/// Spotify Premium (cached 1 h when known) and the current Spotify Connect devices.
#[tauri::command]
#[specta::specta]
pub async fn spotify_capabilities(state: State<'_, AppState>, refresh: bool) -> AppResult<SpotifyCapabilities> {
    if refresh {
        state.spotify_premium.clear();
    }
    player::capabilities(&state.spotify, &state.spotify_premium).await
}

/// Chooses the Spotify Connect device to play on (`None` = active / first available).
#[tauri::command]
#[specta::specta]
pub fn spotify_set_device(state: State<'_, AppState>, device_id: Option<String>) {
    state.spotify_engine.set_device(device_id);
}

/// The chosen Spotify Connect device, if any.
#[tauri::command]
#[specta::specta]
pub fn spotify_chosen_device(state: State<'_, AppState>) -> Option<String> {
    state.spotify_engine.chosen_device()
}

/// Audio output devices for the NativeEngine (Settings → Playback → Output).
#[tauri::command]
#[specta::specta]
pub async fn player_outputs() -> AppResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(native::output_device_names)
        .await
        .map_err(|e| AppError::internal(format!("list outputs: {e}")))
}

/// Switches the NativeEngine output (mid-track) and saves it (`None` = system default).
#[tauri::command]
#[specta::specta]
pub async fn player_set_output(state: State<'_, AppState>, device: Option<String>) -> AppResult<()> {
    state.native.set_output(device.clone()).await?;
    let mut settings = db::settings::load(&state.db).await?;
    settings.player.output_device = device;
    db::settings::save(&state.db, settings).await?;
    Ok(())
}

/// Plays a short quiet tone through the NativeEngine to check the output device.
#[tauri::command]
#[specta::specta]
pub async fn player_test_sound(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::internal(format!("cache dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("test-tone.wav");
    let tone = path.clone();
    tauri::async_runtime::spawn_blocking(move || native::write_test_tone(&tone, 1.5, 0.2))
        .await
        .map_err(|e| AppError::internal(format!("test tone: {e}")))??;
    let volume = db::settings::load(&state.db).await?.player.volume;
    state.native.set_volume(volume).await?;
    state.native.load_path(path, 0).await
}

/// Plays `track_ids` from `start_index` (resolver picks the source unless `force_source`).
#[tauri::command]
#[specta::specta]
pub async fn player_play(
    app: AppHandle,
    state: State<'_, AppState>,
    track_ids: Vec<String>,
    start_index: u32,
    force_source: Option<SourceKind>,
) -> AppResult<()> {
    let start = usize::try_from(start_index).unwrap_or(0);
    let result = state.player.play(track_ids, start, force_source).await;
    state.player.emit_state(&app).await;
    result
}

/// Transport controls (pause, resume, next, previous, seek, volume, shuffle, repeat).
#[tauri::command]
#[specta::specta]
pub async fn player_control(app: AppHandle, state: State<'_, AppState>, action: PlayerAction) -> AppResult<()> {
    let result = state.player.control(action).await;
    state.player.emit_state(&app).await;
    result
}

/// Current player state (the UI also receives it as `player://state`).
#[tauri::command]
#[specta::specta]
pub async fn player_state(state: State<'_, AppState>) -> AppResult<PlayerStateEvent> {
    state.player.state().await
}

/// Every source of a track with whether it can play here ("Play from…").
#[tauri::command]
#[specta::specta]
pub async fn player_sources(state: State<'_, AppState>, track_id: String) -> AppResult<Vec<SourceStatus>> {
    state.player.sources(&track_id).await
}

/// Opens a track on its platform (e.g. Open in Spotify when it can't play here).
#[tauri::command]
#[specta::specta]
pub async fn track_open_external(
    app: AppHandle,
    state: State<'_, AppState>,
    track_id: String,
    provider: SourceKind,
) -> AppResult<()> {
    let sources = crate::core::playback::resolver::stored_sources(&state.db, &track_id).await?;
    let url = sources
        .into_iter()
        .find(|s| s.source.kind == provider && s.source.url.starts_with("https://"))
        .map(|s| s.source.url)
        .ok_or_else(|| AppError::not_found("no link for this source"))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::internal(format!("open track: {e}")))
}
