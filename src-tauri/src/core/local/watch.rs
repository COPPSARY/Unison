//! Watched folders (addendum §4 "Ways in" 3): a debounced notify watcher that hands batches of
//! changed paths to the app, which imports what's there and drops what's gone.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_full::notify::event::{AccessKind, AccessMode};
use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use tokio::sync::mpsc::UnboundedSender;

use crate::error::{AppError, AppResult};

/// Quiet time after the last event before a batch is sent (copies of big files keep a file
/// busy; the batch still lands well inside the 5 s the addendum asks for).
pub const DEBOUNCE: Duration = Duration::from_millis(1500);

pub type Batch = Vec<PathBuf>;
type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Reads (opening a file to import it) aren't changes; a finished write is.
fn relevant(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

/// Partial downloads and hidden files are never imported.
fn ignored(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    name.starts_with('.')
        || name.starts_with("~$")
        || [".part", ".crdownload", ".tmp", ".download"].iter().any(|ext| lower.ends_with(ext))
}

/// Starts watching `folders` (missing ones are skipped and logged). Each debounced batch of
/// changed paths goes to `tx`.
pub fn spawn(folders: &[PathBuf], tx: UnboundedSender<Batch>) -> AppResult<Watcher> {
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| match result {
        Ok(events) => {
            let mut paths: Vec<PathBuf> = events
                .into_iter()
                .filter(|e| relevant(&e.event.kind))
                .flat_map(|e| e.event.paths)
                .filter(|p| !ignored(p))
                .collect();
            paths.sort();
            paths.dedup();
            if !paths.is_empty() {
                let _ = tx.send(paths);
            }
        }
        Err(errors) => {
            for err in errors {
                tracing::warn!(error = %err, "folder watcher error");
            }
        }
    })
    .map_err(|e| AppError::internal(format!("folder watcher: {e}")))?;
    for folder in folders {
        if let Err(err) = debouncer.watch(folder, RecursiveMode::Recursive) {
            tracing::warn!(error = %err, "can't watch a music folder");
        }
    }
    Ok(debouncer)
}

/// The live watcher and where its batches go. Replaced whenever the folder list changes.
#[derive(Default)]
pub struct FolderWatch {
    watcher: Mutex<Option<Watcher>>,
    tx: Mutex<Option<UnboundedSender<Batch>>>,
}

impl FolderWatch {
    pub fn set_sender(&self, tx: UnboundedSender<Batch>) {
        if let Ok(mut slot) = self.tx.lock() {
            *slot = Some(tx);
        }
    }

    /// Watches exactly `folders` from now on. Blocking: adding watches walks the folders.
    pub fn replace(&self, folders: &[PathBuf]) -> AppResult<()> {
        let tx = self.tx.lock().ok().and_then(|slot| slot.clone());
        let Some(tx) = tx else { return Ok(()) };
        let next = if folders.is_empty() { None } else { Some(spawn(folders, tx)?) };
        if let Ok(mut slot) = self.watcher.lock() {
            // Dropping the old debouncer stops it.
            *slot = next;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn wait(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Batch>) -> Batch {
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap()
    }

    #[test]
    fn ignores_partial_and_hidden_files() {
        assert!(ignored(Path::new("/m/.song.mp3")));
        assert!(ignored(Path::new("/m/song.mp3.part")));
        assert!(ignored(Path::new("C:\\m\\song.mp3.crdownload")));
        assert!(!ignored(Path::new("/m/song.mp3")));
        assert!(!relevant(&EventKind::Access(AccessKind::Read)));
        assert!(relevant(&EventKind::Access(AccessKind::Close(AccessMode::Write))));
    }

    #[tokio::test]
    async fn reports_added_and_deleted_files_within_five_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let _watcher = spawn(&[dir.path().to_path_buf()], tx).unwrap();
        let file = dir.path().join("new.mp3");

        std::fs::write(&file, b"x").unwrap();
        let batch = wait(&mut rx).await;
        assert!(batch.iter().any(|p| p.ends_with("new.mp3")), "{batch:?}");

        std::fs::remove_file(&file).unwrap();
        let mut seen = false;
        for _ in 0..3 {
            let batch = wait(&mut rx).await;
            if batch.iter().any(|p| p.ends_with("new.mp3")) && !file.exists() {
                seen = true;
                break;
            }
        }
        assert!(seen);
    }
}
