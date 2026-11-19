//! Starts import jobs (from `local_import`, the Add files / Add folder dialogs, window
//! drag-and-drop, watched folders and Rescan library) on the background runtime, one at a time.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;

use super::import::{self, ImportConfig};
use super::{ImportReporter, LocalChanged, LocalImportProgress};
use crate::core::auth::oauth::now_unix;
use crate::core::db;
use crate::model::WatchedFolder;
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

fn artwork_dir<R: Runtime>(app: &AppHandle<R>) -> AppResult<PathBuf> {
    Ok(app.path().app_data_dir().map_err(|e| AppError::internal(format!("app data dir: {e}")))?.join("artwork"))
}

/// Watcher batches and the launch scan update the library without progress toasts; lists
/// still refresh through `local://changed`.
struct Quiet<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> ImportReporter for Quiet<'_, R> {
    fn progress(&self, _event: LocalImportProgress) {}

    fn changed(&self, event: LocalChanged) {
        self.0.changed(event);
    }
}

/// What a job does besides importing `paths`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Drop, dialogs: progress + summary toast.
    Import,
    /// A newly watched folder: like Import, and its last-scan time is saved.
    NewFolder,
    /// Rescan library: import every watched folder, then check every file (progress + toast).
    Rescan,
    /// Launch scan: like Rescan, quietly.
    QuietRescan,
}

/// Queues an import of `paths`. `copy`: `None` = follow "Copy dropped files", `Some` = force.
/// Returns the job id at once; the work runs in the background.
pub async fn start<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>, copy: Option<bool>) -> AppResult<u32> {
    queue(app, paths, copy, Kind::Import).await
}

/// Rescan library: every watched folder plus the missing-file check. Returns the job id.
pub async fn rescan<R: Runtime>(app: &AppHandle<R>) -> AppResult<u32> {
    let folders = folder_paths(app).await?;
    queue(app, folders, Some(false), Kind::Rescan).await
}

async fn queue<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>, copy: Option<bool>, kind: Kind) -> AppResult<u32> {
    let state = app.state::<AppState>();
    let settings = db::settings::load(&state.db).await?.local;
    let copy_to = if copy.unwrap_or(settings.copy_on_drop) {
        Some(copy_target(app, settings.copy_target.as_deref())?)
    } else {
        None
    };
    let check_missing = matches!(kind, Kind::Rescan | Kind::QuietRescan).then(|| paths.clone());
    let job_id = state.local.next_id();
    let config = ImportConfig { extensions: settings.extensions, artwork_dir: artwork_dir(app)?, copy_to, check_missing };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let _turn = state.local.lock.lock().await;
        tracing::info!(job_id, paths = paths.len(), copy = config.copy_to.is_some(), ?kind, "local import started");
        let scanned: Vec<String> =
            if kind == Kind::Import { Vec::new() } else { paths.iter().map(|p| p.to_string_lossy().into_owned()).collect() };
        let result = if kind == Kind::QuietRescan {
            import::run(&state.db, job_id, paths, config, &Quiet(&app)).await
        } else {
            import::run(&state.db, job_id, paths, config, &app).await
        };
        match result {
            Ok(s) => {
                tracing::info!(
                    job_id,
                    imported = s.imported,
                    updated = s.updated,
                    skipped = s.skipped,
                    failed = s.failed,
                    removed = s.checked.removed,
                    missing = s.checked.missing,
                    restored = s.checked.restored,
                    "local import finished"
                );
                if !scanned.is_empty() {
                    if let Err(err) = mark_scanned(&state.db, &scanned).await {
                        tracing::warn!(error = %err, "could not save last scan time");
                    }
                }
            }
            Err(err) => tracing::warn!(job_id, error = %err, "local import failed"),
        }
    });
    Ok(job_id)
}

async fn mark_scanned(pool: &sqlx::SqlitePool, paths: &[String]) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    db::local::mark_scanned(&mut conn, paths, now_unix()).await
}

async fn folder_paths<R: Runtime>(app: &AppHandle<R>) -> AppResult<Vec<PathBuf>> {
    let state = app.state::<AppState>();
    let mut conn = state.db.acquire().await?;
    Ok(db::local::folders(&mut conn).await?.into_iter().map(|(p, _)| PathBuf::from(p)).collect())
}

/// Watched folders for Settings.
pub async fn folders<R: Runtime>(app: &AppHandle<R>) -> AppResult<Vec<WatchedFolder>> {
    let state = app.state::<AppState>();
    let rows = {
        let mut conn = state.db.acquire().await?;
        db::local::folders(&mut conn).await?
    };
    Ok(rows
        .into_iter()
        .map(|(path, last_scan_at)| WatchedFolder { exists: Path::new(&path).is_dir(), path, last_scan_at })
        .collect())
}

/// Starts watching a folder and imports what's in it. A folder inside one already watched is
/// covered already; folders inside the new one are folded into it.
pub async fn add_folder<R: Runtime>(app: &AppHandle<R>, path: &Path) -> AppResult<Vec<WatchedFolder>> {
    if !path.is_dir() {
        return Err(AppError::new(crate::error::ErrorCode::FileMissing, "folder not found"));
    }
    let path = super::scan::absolute(path);
    let existing = folder_paths(app).await?;
    if existing.iter().any(|f| path.starts_with(f)) {
        return folders(app).await;
    }
    {
        let state = app.state::<AppState>();
        let mut tx = state.db.begin().await?;
        for inner in existing.iter().filter(|f| f.starts_with(&path)) {
            db::local::remove_folder(&mut tx, &inner.to_string_lossy()).await?;
        }
        db::local::add_folder(&mut tx, &path.to_string_lossy()).await?;
        tx.commit().await?;
    }
    folders_changed(app).await?;
    queue(app, vec![path], Some(false), Kind::NewFolder).await?;
    folders(app).await
}

/// Stops watching a folder. Songs already added from it stay in the library.
pub async fn remove_folder<R: Runtime>(app: &AppHandle<R>, path: &str) -> AppResult<Vec<WatchedFolder>> {
    {
        let state = app.state::<AppState>();
        let mut conn = state.db.acquire().await?;
        db::local::remove_folder(&mut conn, path).await?;
    }
    folders_changed(app).await?;
    folders(app).await
}

/// Mirrors the list into Settings (`local.watchedFolders`) and restarts the watcher.
async fn folders_changed<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let paths = folder_paths(app).await?;
    let state = app.state::<AppState>();
    let mut settings = db::settings::load(&state.db).await?;
    settings.local.watched_folders = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    db::settings::save(&state.db, settings).await?;
    restart_watcher(app, paths);
    Ok(())
}

/// Swaps the watcher in the background (adding watches walks the folders).
fn restart_watcher<R: Runtime>(app: &AppHandle<R>, folders: Vec<PathBuf>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if let Err(err) = state.watch.replace(&folders) {
            tracing::warn!(error = %err, "could not watch music folders");
        }
    });
}

/// Launch: adopt folders saved in Settings, start the watcher and the batch consumer, then a
/// quiet scan (picks up what changed while Unison was closed and checks for missing files).
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<super::watch::Batch>();
    app.state::<AppState>().watch.set_sender(tx);

    let consumer = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(mut batch) = rx.recv().await {
            // Fold batches that queued up while the last one ran.
            while let Ok(more) = rx.try_recv() {
                batch.extend(more);
            }
            batch.sort();
            batch.dedup();
            if let Err(err) = apply_batch(&consumer, batch).await {
                tracing::warn!(error = %err, "watched folder update failed");
            }
        }
    });

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = adopt_settings(&app).await {
            tracing::warn!(error = %err, "could not read watched folders");
        }
        match folder_paths(&app).await {
            Ok(folders) if !folders.is_empty() => {
                restart_watcher(&app, folders.clone());
                if let Err(err) = queue(&app, folders, Some(false), Kind::QuietRescan).await {
                    tracing::warn!(error = %err, "launch scan failed to start");
                }
            }
            Ok(_) => {
                // Still flag dropped files that went missing while Unison was closed.
                if let Err(err) = queue(&app, Vec::new(), Some(false), Kind::QuietRescan).await {
                    tracing::warn!(error = %err, "launch check failed to start");
                }
            }
            Err(err) => tracing::warn!(error = %err, "could not read watched folders"),
        }
    });
}

/// Folders listed in Settings but not in the table (e.g. settings restored by hand) are added.
async fn adopt_settings<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let state = app.state::<AppState>();
    let listed = db::settings::load(&state.db).await?.local.watched_folders;
    let mut conn = state.db.acquire().await?;
    for path in listed {
        db::local::add_folder(&mut conn, &path).await?;
    }
    Ok(())
}

/// One debounced watcher batch: import what exists (new, changed or moved in), then check the
/// paths that are gone. Runs quietly, in turn with other jobs.
async fn apply_batch<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>) -> AppResult<()> {
    let state = app.state::<AppState>();
    let settings = db::settings::load(&state.db).await?.local;
    let watched = folder_paths(app).await?;
    let (present, gone): (Vec<PathBuf>, Vec<PathBuf>) = paths
        .into_iter()
        .filter(|p| watched.iter().any(|root| p.starts_with(root)))
        .partition(|p| p.exists());
    let _turn = state.local.lock.lock().await;
    if !present.is_empty() {
        tracing::info!(paths = present.len(), "watched folder changed");
        let config = ImportConfig {
            extensions: settings.extensions,
            artwork_dir: artwork_dir(app)?,
            copy_to: None,
            check_missing: None,
        };
        let job_id = state.local.next_id();
        import::run(&state.db, job_id, present, config, &Quiet(app)).await?;
    }
    if !gone.is_empty() {
        let checked = super::missing::check(&state.db, &watched, Some(&gone)).await?;
        if checked.changed() {
            tracing::info!(removed = checked.removed, missing = checked.missing, "watched files gone");
            app.changed(LocalChanged {
                added: 0,
                removed: checked.removed,
                updated: checked.missing + checked.restored,
            });
        }
    }
    Ok(())
}

/// "Locate file…" for a song whose file moved.
pub async fn locate<R: Runtime>(app: &AppHandle<R>, track_id: &str, path: &Path) -> AppResult<()> {
    let state = app.state::<AppState>();
    let extensions = db::settings::load(&state.db).await?.local.extensions;
    import::locate(&state.db, track_id, path, &extensions, artwork_dir(app)?).await?;
    app.changed(LocalChanged { added: 0, removed: 0, updated: 1 });
    Ok(())
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
