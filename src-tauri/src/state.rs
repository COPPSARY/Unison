//! Managed application state.

use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tokio::sync::oneshot;
use tracing_appender::non_blocking::WorkerGuard;

use crate::core::auth::keychain::SecretStore;
use crate::core::sync::SyncManager;
use crate::providers::spotify::client::SpotifyClient;

/// Shared state available to every command via `tauri::State<AppState>`.
pub struct AppState {
    pub db: SqlitePool,
    pub http: reqwest::Client,
    pub secrets: Arc<dyn SecretStore>,
    /// Shared so the token cache and the 4-request limit apply app-wide.
    pub spotify: Arc<SpotifyClient>,
    pub sync: SyncManager,
    /// Cancels the login currently waiting for the browser, if any.
    pending_auth: Mutex<Option<oneshot::Sender<()>>>,
}

impl AppState {
    pub fn new(db: SqlitePool, http: reqwest::Client, secrets: Arc<dyn SecretStore>) -> Self {
        let spotify = Arc::new(SpotifyClient::new(http.clone(), secrets.clone(), db.clone()));
        Self { db, http, secrets, spotify, sync: SyncManager::default(), pending_auth: Mutex::new(None) }
    }

    /// Registers a new login; any older one waiting for the browser is cancelled.
    pub fn replace_pending_auth(&self, cancel: oneshot::Sender<()>) {
        if let Ok(mut slot) = self.pending_auth.lock() {
            if let Some(old) = slot.replace(cancel) {
                let _ = old.send(());
            }
        }
    }

    pub fn cancel_pending_auth(&self) {
        if let Ok(mut slot) = self.pending_auth.lock() {
            if let Some(tx) = slot.take() {
                let _ = tx.send(());
            }
        }
    }

    pub fn clear_pending_auth(&self) {
        if let Ok(mut slot) = self.pending_auth.lock() {
            slot.take();
        }
    }
}

/// HTTP client for every provider call: rustls, timeouts, an app User-Agent (no user data).
pub fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Unison/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
}

/// Keeps the non-blocking log writer alive (dropping it flushes and stops file logging).
pub struct LogGuard(#[allow(dead_code)] pub WorkerGuard);
