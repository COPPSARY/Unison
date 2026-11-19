//! Player commands. Engine and resolver logic lives in `core::playback` / `providers/`.

use tauri::{AppHandle, Manager, State};

use crate::core::db;
use crate::core::playback::engine::Engine;
use crate::core::playback::native;
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
