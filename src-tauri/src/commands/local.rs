use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::core::db;
use crate::core::local::import::{self, ImportConfig};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Imports audio files and folders (referenced in place). Returns the job id at once; progress
/// arrives as `local://import-progress`, and `local://changed` when the library changed.
#[tauri::command]
#[specta::specta]
pub async fn local_import(app: AppHandle, state: State<'_, AppState>, paths: Vec<String>) -> AppResult<u32> {
    let settings = db::settings::load(&state.db).await?;
    let artwork_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("app data dir: {e}")))?
        .join("artwork");
    let job_id = state.local.next_id();
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let config = ImportConfig { extensions: settings.local.extensions, artwork_dir };
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _turn = state.local.lock.lock().await;
        tracing::info!(job_id, paths = paths.len(), "local import started");
        match import::run(&state.db, job_id, paths, config, &app).await {
            Ok(s) => tracing::info!(job_id, imported = s.imported, updated = s.updated, skipped = s.skipped,
                failed = s.failed, "local import finished"),
            Err(err) => tracing::warn!(job_id, error = %err, "local import failed"),
        }
    });
    Ok(job_id)
}
