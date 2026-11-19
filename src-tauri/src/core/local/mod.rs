//! Local files (addendum §4): import pipeline that turns audio files on disk into tracks.
//! Files are referenced in place and only ever read; tags are never modified.

pub mod artwork;
pub mod fingerprint;
pub mod import;
pub mod scan;
pub mod tags;
#[cfg(test)]
pub mod testing;

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::error::AppError;

/// Files per batch: one transaction and one progress event each.
pub const BATCH_SIZE: usize = 200;
/// At most this many per-file problems are kept for the UI.
pub const MAX_ISSUES: usize = 200;

/// Why a file was not imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    /// The path no longer exists.
    Missing,
    /// Not an audio type this build can play (e.g. opus, or a non-audio file chosen directly).
    UnsupportedFormat,
    /// DRM-protected purchase (.m4p).
    DrmProtected,
    /// Could not be read or parsed.
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub path: String,
    pub reason: IssueKind,
}

impl Issue {
    pub fn new(path: &Path, reason: IssueKind) -> Self {
        Self { path: path.to_string_lossy().into_owned(), reason }
    }
}

/// `local://import-progress`: sent after every batch and once more with `finished: true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "local://import-progress")]
pub struct LocalImportProgress {
    pub job_id: u32,
    /// Audio files looked at so far.
    pub scanned: u32,
    /// Known once the paths have been walked.
    pub total: Option<u32>,
    /// New or updated in the library.
    pub imported: u32,
    /// Already imported and unchanged (or a second copy of an imported file).
    pub skipped: u32,
    /// Per-file problems so far (first `MAX_ISSUES`).
    pub errors: Vec<Issue>,
    pub finished: bool,
    /// Set when the job stopped early.
    pub error: Option<AppError>,
}

/// `local://changed`: the local library changed (sent at the end of a job that changed it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "local://changed")]
pub struct LocalChanged {
    pub added: u32,
    pub removed: u32,
    pub updated: u32,
}

/// Where import events go: the Tauri app in production, a Vec in tests.
pub trait ImportReporter: Send + Sync {
    fn progress(&self, event: LocalImportProgress);
    fn changed(&self, event: LocalChanged);
}

impl<R: tauri::Runtime> ImportReporter for tauri::AppHandle<R> {
    fn progress(&self, event: LocalImportProgress) {
        if let Err(err) = event.emit(self) {
            tracing::warn!(error = %err, "emit import progress failed");
        }
    }

    fn changed(&self, event: LocalChanged) {
        if let Err(err) = event.emit(self) {
            tracing::warn!(error = %err, "emit local changed failed");
        }
    }
}

/// Hands out job ids and runs one import at a time (later jobs wait their turn).
#[derive(Debug, Default)]
pub struct LocalImports {
    next: AtomicU32,
    pub lock: tokio::sync::Mutex<()>,
}

impl LocalImports {
    pub fn next_id(&self) -> u32 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }
}
