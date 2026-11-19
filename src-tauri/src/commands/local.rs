//! Local files: import, Add files / Add folder dialogs.

use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::core::db;
use crate::core::local::jobs;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Imports audio files and folders. `copy`: `null` = follow "Copy dropped files". Returns the
/// job id at once; progress arrives as `local://import-progress`, and `local://changed` when
/// the library changed.
#[tauri::command]
#[specta::specta]
pub async fn local_import(app: AppHandle, paths: Vec<String>, copy: Option<bool>) -> AppResult<u32> {
    jobs::start(&app, paths.into_iter().map(PathBuf::from).collect(), copy).await
}

fn to_paths(picked: Vec<FilePath>) -> Vec<PathBuf> {
    picked.into_iter().filter_map(|p| p.into_path().ok()).collect()
}

fn closed(_: oneshot::error::RecvError) -> AppError {
    AppError::internal("file dialog closed without an answer")
}

/// Native "Add files" dialog (audio types from Settings → Local files), then imports the
/// choice like a drop. `null` when cancelled. `title` / `filter_label` come from en.json.
#[tauri::command]
#[specta::specta]
pub async fn local_add_files(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    filter_label: String,
) -> AppResult<Option<u32>> {
    let extensions = db::settings::load(&state.db).await?.local.extensions;
    let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .add_filter(filter_label, &exts)
        .pick_files(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.map_err(closed)? else { return Ok(None) };
    jobs::start(&app, to_paths(picked), None).await.map(Some)
}

/// Native "Add folder" dialog; the folder is always referenced in place (never copied).
#[tauri::command]
#[specta::specta]
pub async fn local_add_folder(app: AppHandle, title: String) -> AppResult<Option<u32>> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let Some(folder) = rx.await.map_err(closed)? else { return Ok(None) };
    jobs::start(&app, to_paths(vec![folder]), Some(false)).await.map(Some)
}

/// Where copy-on-drop puts files (the setting, else `<Music>/Unison`), for Settings.
#[tauri::command]
#[specta::specta]
pub async fn local_copy_target(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let setting = db::settings::load(&state.db).await?.local.copy_target;
    jobs::copy_target(&app, setting.as_deref()).map(|p| p.to_string_lossy().into_owned())
}
