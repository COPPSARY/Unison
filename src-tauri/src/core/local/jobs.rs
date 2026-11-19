//! Starts import jobs (from `local_import`, the Add files / Add folder dialogs and window
//! drag-and-drop) on the background runtime, one at a time.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::import::{self, ImportConfig};
use crate::core::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// `local://drag`: files are being dragged over the window (`active`) or the drag ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "local://drag")]
pub struct LocalDrag {
    pub active: bool,
}

/// Folder copy-on-drop copies into: the setting, else `<Music>/Unison`.
pub fn copy_target<R: Runtime>(app: &AppHandle<R>, setting: Option<&str>) -> AppResult<PathBuf> {
    if let Some(dir) = setting {
        return Ok(PathBuf::from(dir));
    }
    let music = app
        .path()
        .audio_dir()
        .or_else(|_| app.path().home_dir().map(|h| h.join("Music")))
        .map_err(|e| AppError::internal(format!("music dir: {e}")))?;
    Ok(music.join("Unison"))
}

/// Queues an import of `paths`. `copy`: `None` = follow "Copy dropped files", `Some` = force.
/// Returns the job id at once; the work runs in the background.
pub async fn start<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>, copy: Option<bool>) -> AppResult<u32> {
    let state = app.state::<AppState>();
    let settings = db::settings::load(&state.db).await?.local;
    let artwork_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("app data dir: {e}")))?
        .join("artwork");
    let copy_to = if copy.unwrap_or(settings.copy_on_drop) {
        Some(copy_target(app, settings.copy_target.as_deref())?)
    } else {
        None
    };
    let job_id = state.local.next_id();
    let config = ImportConfig { extensions: settings.extensions, artwork_dir, copy_to };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _turn = state.local.lock.lock().await;
        tracing::info!(job_id, paths = paths.len(), copy = config.copy_to.is_some(), "local import started");
        match import::run(&state.db, job_id, paths, config, &app).await {
            Ok(s) => tracing::info!(
                job_id,
                imported = s.imported,
                updated = s.updated,
                skipped = s.skipped,
                failed = s.failed,
                "local import finished"
            ),
            Err(err) => tracing::warn!(job_id, error = %err, "local import failed"),
        }
    });
    Ok(job_id)
}

/// Window drag-and-drop (addendum §4 "Ways in" 1): show/hide the overlay, import on drop.
pub fn on_drag_drop<R: Runtime>(app: &AppHandle<R>, event: &tauri::DragDropEvent) {
    let emit = |active: bool| {
        if let Err(err) = (LocalDrag { active }).emit(app) {
            tracing::warn!(error = %err, "emit drag state failed");
        }
    };
    match event {
        tauri::DragDropEvent::Enter { .. } => emit(true),
        tauri::DragDropEvent::Leave => emit(false),
        tauri::DragDropEvent::Drop { paths, .. } => {
            emit(false);
            if paths.is_empty() {
                return;
            }
            let app = app.clone();
            let paths = paths.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = start(&app, paths, None).await {
                    tracing::warn!(error = %err, "could not start import for dropped files");
                }
            });
        }
        _ => {}
    }
}
