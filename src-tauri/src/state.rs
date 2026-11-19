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
    /// Local files + HTTP streams (UP-05/06).
    pub native: Arc<crate::core::playback::native::NativeEngine>,
    /// Queue, resolver and engines (UP-09).
    pub player: crate::core::playback::service::PlayerService,
    /// Official IFrame player in the webview (UP-07).
    pub youtube: Arc<crate::core::playback::youtube_engine::YouTubeEngine>,
    pub sync: SyncManager,
    /// Spotify Premium answer, cached 1 h (UP-04).
    pub spotify_premium: Arc<crate::providers::spotify::player::PremiumCache>,
    /// Spotify Connect remote control (UP-08).
    pub spotify_engine: Arc<crate::providers::spotify::engine::SpotifyConnectEngine>,
    /// Pending YouTube Test playback (UP-03).
    pub youtube_test: crate::core::playback::youtube::YoutubeTestHub,
    /// One playlist-items fetch at a time.
    pub playlist_lock: tokio::sync::Mutex<()>,
    /// Cancels the login currently waiting for the browser, if any.
    pending_auth: Mutex<Option<oneshot::Sender<()>>>,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        http: reqwest::Client,
        secrets: Arc<dyn SecretStore>,
        native: Arc<crate::core::playback::native::NativeEngine>,
        youtube: Arc<crate::core::playback::youtube_engine::YouTubeEngine>,
        volume: u8,
    ) -> Self {
        let spotify = Arc::new(SpotifyClient::new(http.clone(), secrets.clone(), db.clone()));
        let spotify_premium = Arc::new(crate::providers::spotify::player::PremiumCache::default());
        let spotify_engine = Arc::new(crate::providers::spotify::engine::SpotifyConnectEngine::new(
            spotify.clone(),
            spotify_premium.clone(),
        ));
        let engines: Vec<Arc<dyn crate::core::playback::engine::Engine>> =
            vec![native.clone(), youtube.clone(), spotify_engine.clone()];
        let player = crate::core::playback::service::PlayerService::new(db.clone(), engines, volume);
        Self {
            player,
            db,
            http,
            secrets,
            spotify,
            native,
            youtube,
            sync: SyncManager::default(),
            playlist_lock: tokio::sync::Mutex::new(()),
            youtube_test: crate::core::playback::youtube::YoutubeTestHub::default(),
            spotify_premium,
            spotify_engine,
            pending_auth: Mutex::new(None),
        }
    }

    pub fn playlist_loader(&self) -> crate::core::playlists::PlaylistLoader<'_> {
        crate::core::playlists::PlaylistLoader { db: &self.db, spotify: &self.spotify, lock: &self.playlist_lock }
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
