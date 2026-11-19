use tauri::State;

use crate::core::db;
use crate::error::AppResult;
use crate::model::Settings;
use crate::state::AppState;

/// Current settings (defaults for anything never saved).
#[tauri::command]
#[specta::specta]
pub async fn settings_get(state: State<'_, AppState>) -> AppResult<Settings> {
    db::settings::load(&state.db).await
}

/// Saves settings and returns the stored (sanitised) value.
#[tauri::command]
#[specta::specta]
pub async fn settings_set(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    db::settings::save(&state.db, settings).await
}
