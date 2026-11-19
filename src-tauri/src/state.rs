//! Managed application state.

use sqlx::SqlitePool;
use tracing_appender::non_blocking::WorkerGuard;

/// Shared state available to every command via `tauri::State<AppState>`.
pub struct AppState {
    pub db: SqlitePool,
}

/// Keeps the non-blocking log writer alive (dropping it flushes and stops file logging).
pub struct LogGuard(#[allow(dead_code)] pub WorkerGuard);
