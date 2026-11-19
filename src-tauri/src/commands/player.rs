//! Player commands. Engine and resolver logic lives in `core::playback` / `providers/`.

use tauri::State;

use crate::error::AppResult;
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
