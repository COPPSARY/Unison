//! YouTube player commands (UP-03). The player itself runs in the webview.

use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::core::playback::youtube::{YoutubeCommand, YoutubeTestHub, TEST_TIMEOUT, TEST_VIDEO_ID};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Plays a known embeddable video in the visible test player and reports whether YouTube
/// accepted the embed (Settings → YouTube player → Test playback).
#[tauri::command]
#[specta::specta]
pub async fn youtube_test_playback(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let rx = state.youtube_test.begin();
    YoutubeCommand::Test { video_id: TEST_VIDEO_ID.to_owned() }
        .emit(&app)
        .map_err(|e| AppError::internal(format!("emit youtube test: {e}")))?;
    let result = YoutubeTestHub::wait(rx, TEST_TIMEOUT).await;
    match &result {
        Ok(()) => tracing::info!("youtube test playback ok"),
        Err(err) => tracing::warn!(error = %err, "youtube test playback failed"),
    }
    result
}

/// The UI's answer to a test: `ok` when the player reached PLAYING, else the onError code.
#[tauri::command]
#[specta::specta]
pub fn youtube_test_report(state: State<'_, AppState>, ok: bool, code: Option<u16>) {
    state.youtube_test.report(if ok { Ok(()) } else { Err(code.unwrap_or(0)) });
}
