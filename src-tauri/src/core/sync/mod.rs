//! Sync orchestration (spec §8.5): one sync per provider at a time, cancellable, progress
//! events every page, `sync://finished` at the end. Provider-specific work lives in
//! `providers/<name>/sync.rs`.

pub mod scheduler;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::error::{AppError, ErrorCode};
use crate::model::ProviderId;

/// What a sync is working on (shown in the sidebar status line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SyncStage {
    Profile,
    Liked,
    Playlists,
    Albums,
    Artists,
    Taste,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "sync://progress")]
pub struct SyncProgress {
    pub provider: ProviderId,
    pub stage: SyncStage,
    pub done: u32,
    pub total: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "sync://finished")]
pub struct SyncFinished {
    pub provider: ProviderId,
    pub added: u32,
    pub removed: u32,
    /// Set when the sync failed or was cancelled.
    pub error: Option<AppError>,
}

/// Counts reported at the end of a sync.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub added: u32,
    pub removed: u32,
}

/// Where progress goes: the Tauri app in production, a Vec in tests.
pub trait SyncReporter: Send + Sync {
    fn progress(&self, event: SyncProgress);
    fn finished(&self, event: SyncFinished);
}

impl<R: tauri::Runtime> SyncReporter for tauri::AppHandle<R> {
    fn progress(&self, event: SyncProgress) {
        if let Err(err) = event.emit(self) {
            tracing::warn!(error = %err, "emit sync progress failed");
        }
    }

    fn finished(&self, event: SyncFinished) {
        if let Err(err) = event.emit(self) {
            tracing::warn!(error = %err, "emit sync finished failed");
        }
    }
}

/// Cancellation flag checked between pages.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// `Err(AuthCancelled-like)` once cancelled, for `?` between pages.
    pub fn check(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::new(ErrorCode::Internal, "sync cancelled"))
        } else {
            Ok(())
        }
    }
}

/// Tracks running syncs: at most one per provider.
#[derive(Default)]
pub struct SyncManager {
    running: Mutex<HashMap<ProviderId, CancelFlag>>,
    /// Unix seconds of the last attempt per provider (successful or not).
    last_attempt: Mutex<HashMap<ProviderId, i64>>,
}

impl SyncManager {
    /// Registers a sync; `None` if one is already running for `provider`.
    pub fn begin(&self, provider: ProviderId) -> Option<CancelFlag> {
        let mut running = self.running.lock().ok()?;
        if running.contains_key(&provider) {
            return None;
        }
        let flag = CancelFlag::default();
        running.insert(provider, flag.clone());
        if let Ok(mut attempts) = self.last_attempt.lock() {
            attempts.insert(provider, crate::core::auth::oauth::now_unix());
        }
        Some(flag)
    }

    pub fn end(&self, provider: ProviderId) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&provider);
        }
    }

    pub fn cancel(&self, provider: ProviderId) {
        if let Ok(running) = self.running.lock() {
            if let Some(flag) = running.get(&provider) {
                flag.cancel();
            }
        }
    }

    /// When the last sync for this provider was started in this session.
    pub fn last_attempt(&self, provider: ProviderId) -> Option<i64> {
        self.last_attempt.lock().ok()?.get(&provider).copied()
    }

    pub fn is_running(&self, provider: ProviderId) -> bool {
        self.running.lock().map(|r| r.contains_key(&provider)).unwrap_or(false)
    }
}

/// Test reporter that records every event.
#[cfg(test)]
#[derive(Default)]
pub struct RecordingReporter {
    pub progress: Mutex<Vec<SyncProgress>>,
    pub finished: Mutex<Vec<SyncFinished>>,
}

#[cfg(test)]
impl SyncReporter for RecordingReporter {
    fn progress(&self, event: SyncProgress) {
        self.progress.lock().unwrap().push(event);
    }

    fn finished(&self, event: SyncFinished) {
        self.finished.lock().unwrap().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sync_per_provider() {
        let m = SyncManager::default();
        let flag = m.begin(ProviderId::Spotify).unwrap();
        assert!(m.begin(ProviderId::Spotify).is_none());
        assert!(m.begin(ProviderId::Youtube).is_some());
        assert!(m.is_running(ProviderId::Spotify));
        m.cancel(ProviderId::Spotify);
        assert!(flag.is_cancelled());
        assert!(flag.check().is_err());
        m.end(ProviderId::Spotify);
        assert!(!m.is_running(ProviderId::Spotify));
        assert!(m.begin(ProviderId::Spotify).is_some());
    }
}
