//! Managed application state.

use std::sync::Arc;

use sqlx::SqlitePool;
use tracing_appender::non_blocking::WorkerGuard;

/// Shared state available to every command via `tauri::State<AppState>`.
pub struct AppState {
    pub db: SqlitePool,
    /// Local files + HTTP streams (UP-05/06).
    pub native: Arc<crate::core::playback::native::NativeEngine>,
    /// Queue, resolver and engines (UP-09).
    pub player: crate::core::playback::service::PlayerService,
    /// Local file imports, one at a time (UP-10).
    pub local: crate::core::local::LocalImports,
    /// Watched folders (UP-12).
    pub watch: crate::core::local::watch::FolderWatch,
    /// Installed extensions (EXT-01/02): the music sources.
    pub extensions: Arc<crate::core::extensions::ExtensionManager>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        native: Arc<crate::core::playback::native::NativeEngine>,
        extensions: Arc<crate::core::extensions::ExtensionManager>,
        volume: u8,
    ) -> Self {
        native.set_stream_resolver(extensions.clone());
        let engines: Vec<Arc<dyn crate::core::playback::engine::Engine>> = vec![native.clone()];
        let finder: Arc<dyn crate::core::playback::engine::SourceFinder> = extensions.clone();
        let player = crate::core::playback::service::PlayerService::new(db.clone(), engines, volume, Some(finder));
        Self {
            player,
            db,
            native,
            local: crate::core::local::LocalImports::default(),
            watch: crate::core::local::watch::FolderWatch::default(),
            extensions,
        }
    }
}

/// Keeps the non-blocking log writer alive (dropping it flushes and stops file logging).
pub struct LogGuard(#[allow(dead_code)] pub WorkerGuard);
