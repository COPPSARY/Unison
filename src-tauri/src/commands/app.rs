use tauri::State;

use crate::core::db;
use crate::error::AppResult;
use crate::model::AppInfo;
use crate::state::AppState;

/// App version and database schema version.
#[tauri::command]
#[specta::specta]
pub async fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: db::schema_version(&state.db).await?,
    })
}
