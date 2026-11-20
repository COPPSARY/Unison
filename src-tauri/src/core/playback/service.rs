//! Player service (UP-09): owns the `Player`, runs the 500 ms tick (end / failure / position),
//! persists playback_log, and emits `player://state` and `player://fallback`.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;
use tokio::sync::Mutex;

use super::engine::{Engine, SourceStatus, UnavailableReason};
use super::player::{FallbackEvent, LogEntry, Player, PlayerSnapshot, PlayerStatus};
use super::queue::{Queue, RepeatMode};
use super::resolver::{self, Resolver};
use crate::core::now_unix;
use crate::core::db;
use crate::error::AppResult;
use crate::model::SourceKind;
use crate::state::AppState;

const TICK: Duration = Duration::from_millis(500);

/// Title, artists and artwork of the current track (for the player bar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub artwork: Option<String>,
}

/// `player://state` (spec §5, addendum §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "player://state")]
pub struct PlayerStateEvent {
    #[serde(flatten)]
    pub snapshot: PlayerSnapshot,
    pub track: Option<NowPlaying>,
    pub position_ms: u32,
    pub duration_ms: Option<u32>,
    pub can_seek: bool,
}

/// `player://fallback` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "player://fallback")]
pub struct PlayerFallback {
    pub track_id: String,
    pub from_provider: SourceKind,
    pub to_provider: SourceKind,
    pub reason: UnavailableReason,
}

impl From<FallbackEvent> for PlayerFallback {
    fn from(f: FallbackEvent) -> Self {
        Self { track_id: f.track_id, from_provider: f.from_provider, to_provider: f.to_provider, reason: f.reason }
    }
}

/// Transport actions (spec §5 `player_control`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerAction {
    Pause,
    Resume,
    Toggle,
    Next,
    Prev,
    Seek {
        #[serde(rename = "positionMs")]
        position_ms: u32,
    },
    Volume {
        volume: u8,
    },
    Shuffle {
        on: bool,
    },
    Repeat {
        mode: RepeatMode,
    },
}

pub struct PlayerService {
    db: SqlitePool,
    player: Mutex<Player>,
    engines: Vec<Arc<dyn Engine>>,
    now_playing: Mutex<Option<(String, NowPlaying)>>,
    /// Last snapshot the UI was told, so "loading" can be announced while a load holds the
    /// player's lock.
    last_snapshot: std::sync::Mutex<Option<PlayerSnapshot>>,
}

async fn now_playing(pool: &SqlitePool, track_id: &str) -> Option<NowPlaying> {
    let row: Option<(String, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT title, artists_json, album, artwork_url FROM tracks WHERE id = ?1")
            .bind(track_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.map(|(title, artists, album, artwork)| NowPlaying {
        title,
        artists: serde_json::from_str(&artists).unwrap_or_default(),
        album,
        artwork,
    })
}

fn provider_str(kind: SourceKind) -> &'static str {
    kind.as_str()
}

impl PlayerService {
    /// `finder` looks for more sources when a track's own can't play (the extensions).
    pub fn new(
        db: SqlitePool,
        engines: Vec<Arc<dyn Engine>>,
        volume: u8,
        finder: Option<Arc<dyn super::engine::SourceFinder>>,
    ) -> Self {
        let resolver = Arc::new(Resolver { db: db.clone(), finder });
        // Volume applies when the first item loads.
        let player = Player::new(engines.clone(), resolver, Queue::default()).with_volume(volume);
        Self {
            db,
            player: Mutex::new(player),
            engines,
            now_playing: Mutex::new(None),
            last_snapshot: std::sync::Mutex::new(None),
        }
    }

    async fn persist(&self, log: Vec<LogEntry>) {
        let now = now_unix();
        for row in log {
            let started = now - i64::from(row.played_ms / 1000);
            let result = sqlx::query(
                "INSERT INTO playback_log (track_id, provider, started_at, played_ms, ended_reason)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&row.track_id)
            .bind(provider_str(row.provider))
            .bind(started)
            .bind(i64::from(row.played_ms))
            .bind(row.reason.as_str())
            .execute(&self.db)
            .await;
            if let Err(err) = result {
                tracing::warn!(error = %err, "playback_log insert failed");
            }
        }
    }

    /// Title, artists and artwork of a track (cached for the current one).
    async fn track_info(&self, id: &str) -> Option<NowPlaying> {
        let mut cache = self.now_playing.lock().await;
        match cache.as_ref() {
            Some((cached, np)) if cached == id => Some(np.clone()),
            _ => {
                let np = now_playing(&self.db, id).await;
                *cache = np.clone().map(|n| (id.to_owned(), n));
                np
            }
        }
    }

    /// Current state for the UI (also the `player_state` command).
    pub async fn state(&self) -> AppResult<PlayerStateEvent> {
        let player = self.player.lock().await;
        let snapshot = player.snapshot();
        let engine = player.engine_state().await.unwrap_or(None);
        drop(player);
        if let Ok(mut last) = self.last_snapshot.lock() {
            *last = Some(snapshot.clone());
        }
        let track = match &snapshot.track_id {
            None => None,
            Some(id) => self.track_info(id).await,
        };
        let active = matches!(snapshot.status, PlayerStatus::Playing | PlayerStatus::Paused);
        Ok(PlayerStateEvent {
            position_ms: engine.filter(|_| active).map_or(0, |e| e.position_ms),
            duration_ms: engine.filter(|_| active).and_then(|e| e.duration_ms),
            can_seek: active,
            track,
            snapshot,
        })
    }

    async fn drain<R: Runtime>(&self, app: &AppHandle<R>) {
        let (fallbacks, log) = {
            let mut p = self.player.lock().await;
            (p.take_fallbacks(), p.take_log())
        };
        for f in fallbacks {
            tracing::info!(from = ?f.from_provider, to = ?f.to_provider, "playback fallback");
            if let Err(err) = PlayerFallback::from(f).emit(app) {
                tracing::warn!(error = %err, "emit fallback failed");
            }
        }
        self.persist(log).await;
    }

    pub async fn emit_state<R: Runtime>(&self, app: &AppHandle<R>) {
        self.drain(app).await;
        if let Ok(state) = self.state().await {
            if let Err(err) = state.emit(app) {
                tracing::warn!(error = %err, "emit player state failed");
            }
        }
    }

    pub async fn play(&self, track_ids: Vec<String>, start: usize, force: Option<SourceKind>) -> AppResult<()> {
        self.player.lock().await.play(track_ids, start, force).await
    }

    /// Tells the UI a track is loading right away (title, artwork, "loading"), without waiting
    /// for the player: a load can take many seconds and holds the player's lock throughout.
    async fn announce_loading<R: Runtime>(&self, app: &AppHandle<R>, track_id: &str) {
        let base = self.last_snapshot.lock().ok().and_then(|l| l.clone());
        let Some(base) = base else { return };
        let snapshot = PlayerSnapshot {
            status: PlayerStatus::Loading,
            track_id: Some(track_id.to_owned()),
            source: None,
            ..base
        };
        let event = PlayerStateEvent {
            snapshot,
            track: self.track_info(track_id).await,
            position_ms: 0,
            duration_ms: None,
            can_seek: false,
        };
        if let Err(err) = event.emit(app) {
            tracing::warn!(error = %err, "emit loading state failed");
        }
    }

    /// Plays like [`play`](Self::play), showing the track as loading at once and the result
    /// (playing, or the error) when the load ends.
    pub async fn play_announced<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        track_ids: Vec<String>,
        start: usize,
        force: Option<SourceKind>,
    ) -> AppResult<()> {
        if let Some(id) = track_ids.get(start) {
            // The first play after startup has no snapshot yet: read it now (no load is running).
            if self.last_snapshot.lock().is_ok_and(|l| l.is_none()) {
                let _ = self.state().await;
            }
            self.announce_loading(app, id).await;
        }
        let result = self.play(track_ids, start, force).await;
        self.emit_state(app).await;
        result
    }

    pub async fn control(&self, action: PlayerAction) -> AppResult<()> {
        let mut p = self.player.lock().await;
        match action {
            PlayerAction::Pause => p.pause().await,
            PlayerAction::Resume => p.resume().await,
            PlayerAction::Toggle => p.toggle().await,
            PlayerAction::Next => p.next().await,
            PlayerAction::Prev => p.prev().await,
            PlayerAction::Seek { position_ms } => p.seek(position_ms).await,
            PlayerAction::Volume { volume } => {
                p.set_volume(volume).await?;
                drop(p);
                // Remember the volume (addendum §5).
                let mut settings = db::settings::load(&self.db).await?;
                settings.player.volume = volume.min(100);
                db::settings::save(&self.db, settings).await.map(|_| ())
            }
            PlayerAction::Shuffle { on } => {
                p.set_shuffle(on);
                Ok(())
            }
            PlayerAction::Repeat { mode } => {
                p.set_repeat(mode);
                Ok(())
            }
        }
    }

    /// "Play from…": every source kind with its status for one track (addendum §2).
    pub async fn sources(&self, track_id: &str) -> AppResult<Vec<SourceStatus>> {
        let settings = db::settings::load(&self.db).await?;
        let stored = resolver::stored_sources(&self.db, track_id).await?;
        let mut out = Vec::new();
        for kind in settings.player.source_priority {
            let Some(found) = stored.iter().find(|s| s.source.kind == kind) else {
                out.push(SourceStatus { kind, source: None, can_play: false, reason: Some(UnavailableReason::NoSource) });
                continue;
            };
            let reason = if !found.available {
                Some(UnavailableReason::Unavailable)
            } else {
                match self.engines.iter().find(|e| e.kinds().contains(&kind)) {
                    None => Some(UnavailableReason::NotConnected),
                    Some(engine) => engine.can_play(&found.source).await.err(),
                }
            };
            out.push(SourceStatus { kind, source: Some(found.source.clone()), can_play: reason.is_none(), reason });
        }
        Ok(out)
    }

    /// One tick: end / failure handling. Returns whether something is playing.
    async fn tick(&self) -> bool {
        let mut p = self.player.lock().await;
        if let Err(err) = p.tick().await {
            tracing::warn!(error = %err, "player tick");
        }
        matches!(p.snapshot().status, PlayerStatus::Playing | PlayerStatus::Loading)
    }
}

/// Starts the background tick. Emits state every tick while playing and once when it stops.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut was_active = false;
        loop {
            tokio::time::sleep(TICK).await;
            let state = app.state::<AppState>();
            let active = state.player.tick().await;
            if active || was_active {
                state.player.emit_state(&app).await;
            }
            was_active = active;
        }
    });
}
