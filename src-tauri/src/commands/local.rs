//! Local files: import, Add files / Add folder dialogs, watched folders, rescan, Locate file….

use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::core::db;
use crate::core::local::jobs;
use crate::error::{AppError, AppResult};
use crate::model::WatchedFolder;
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

/// Native folder picker; `null` when cancelled. Used by Settings → Add folder….
#[tauri::command]
#[specta::specta]
pub async fn local_pick_folder(app: AppHandle, title: String) -> AppResult<Option<String>> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let picked = rx.await.map_err(closed)?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

/// Native single-file picker for audio files; `null` when cancelled. Used by Locate file….
#[tauri::command]
#[specta::specta]
pub async fn local_pick_file(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    filter_label: String,
) -> AppResult<Option<String>> {
    let extensions = db::settings::load(&state.db).await?.local.extensions;
    let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let (tx, rx) = oneshot::channel();
    app.dialog().file().set_title(title).add_filter(filter_label, &exts).pick_file(move |picked| {
        let _ = tx.send(picked);
    });
    let picked = rx.await.map_err(closed)?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

/// Watched folders with their last scan time.
#[tauri::command]
#[specta::specta]
pub async fn local_folders_list(app: AppHandle) -> AppResult<Vec<WatchedFolder>> {
    jobs::folders(&app).await
}

/// Watches a folder and imports what's in it (in the background). Returns the new list.
#[tauri::command]
#[specta::specta]
pub async fn local_folder_add(app: AppHandle, path: String) -> AppResult<Vec<WatchedFolder>> {
    jobs::add_folder(&app, &PathBuf::from(path)).await
}

/// Stops watching a folder; its songs stay in the library. Returns the new list.
#[tauri::command]
#[specta::specta]
pub async fn local_folder_remove(app: AppHandle, path: String) -> AppResult<Vec<WatchedFolder>> {
    jobs::remove_folder(&app, &path).await
}

/// Rescan library: watched folders + missing-file check. Returns the job id; progress arrives as
/// `local://import-progress` with `rescan: true`.
#[tauri::command]
#[specta::specta]
pub async fn local_rescan(app: AppHandle) -> AppResult<u32> {
    jobs::rescan(&app).await
}

/// Locate file…: points a song whose local file moved at `new_path`.
#[tauri::command]
#[specta::specta]
pub async fn local_locate(app: AppHandle, track_id: String, new_path: String) -> AppResult<()> {
    jobs::locate(&app, &track_id, &PathBuf::from(new_path)).await
}
