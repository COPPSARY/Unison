//! Player state machine (addendum §1–§2). Rust owns the queue and decides which engine plays:
//! for each track it takes the ordered candidates, asks each engine `can_play`, and loads the
//! first that can, falling back to the next candidate when one fails. Only one engine plays.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::engine::{Candidates, Engine, EngineState, PlaySource, Policy, UnavailableReason};
use super::queue::{Queue, RepeatMode};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::SourceKind;

/// "Previous" restarts the current track when it has played longer than this.
const RESTART_THRESHOLD_MS: u32 = 3_000;
/// Ticks the engine must disagree on playing/paused before the player follows it (paused
/// or resumed outside Unison, e.g. in the Spotify app).
const FOLLOW_TICKS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PlayerStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    /// The current track had no playable source.
    Error,
}

/// What the UI sees (part of `player://state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub status: PlayerStatus,
    pub track_id: Option<String>,
    /// Source actually playing.
    pub source: Option<PlaySource>,
    pub volume: u8,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub queue_length: u32,
}

/// A switch to another source of the same track (`player://fallback`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FallbackEvent {
    pub track_id: String,
    pub from_provider: SourceKind,
    pub to_provider: SourceKind,
    pub reason: UnavailableReason,
}

/// Why an item stopped playing (playback_log.ended_reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndedReason {
    Finished,
    Skipped,
    Error,
    Fallback,
}

impl EndedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            EndedReason::Finished => "finished",
            EndedReason::Skipped => "skipped",
            EndedReason::Error => "error",
            EndedReason::Fallback => "fallback",
        }
    }
}

/// One playback_log row, persisted by the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub track_id: String,
    pub provider: SourceKind,
    pub played_ms: u32,
    pub reason: EndedReason,
}

/// The item currently loaded, for logging.
#[derive(Debug, Clone)]
struct Session {
    track_id: String,
    kind: SourceKind,
    last_pos: u32,
}

fn reason_for(err: &AppError) -> UnavailableReason {
    match &err.code {
        ErrorCode::SpotifyPremiumRequired => UnavailableReason::PremiumRequired,
        ErrorCode::NoSpotifyDevice => UnavailableReason::NoDevice,
        ErrorCode::FileMissing => UnavailableReason::FileMissing,
        ErrorCode::UnsupportedFormat | ErrorCode::DrmProtected => UnavailableReason::UnsupportedFormat,
        ErrorCode::YoutubeEmbedBlocked { .. } | ErrorCode::NotEmbeddable => UnavailableReason::NotEmbeddable,
        ErrorCode::NeedsReconnect => UnavailableReason::NotConnected,
        _ => UnavailableReason::Offline,
    }
}

pub struct Player {
    engines: HashMap<SourceKind, Arc<dyn Engine>>,
    candidates: Arc<dyn Candidates>,
    queue: Queue,
    status: PlayerStatus,
    active: Option<PlaySource>,
    volume: u8,
    /// Why each candidate of the last load was skipped (for logs / UI).
    last_skips: Vec<(SourceKind, UnavailableReason)>,
    /// Source forced by "Play from…" for the next load only.
    force: Option<SourceKind>,
    session: Option<Session>,
    fallbacks: Vec<FallbackEvent>,
    log: Vec<LogEntry>,
    disagree: u8,
}

impl Player {
    pub fn new(engines: Vec<Arc<dyn Engine>>, candidates: Arc<dyn Candidates>, queue: Queue) -> Self {
        Self {
            engines: engines
                .into_iter()
                .flat_map(|e| e.kinds().iter().map(move |k| (*k, e.clone())).collect::<Vec<_>>())
                .collect(),
            candidates,
            queue,
            status: PlayerStatus::Idle,
            active: None,
            volume: 80,
            last_skips: Vec::new(),
            force: None,
            session: None,
            fallbacks: Vec::new(),
            log: Vec::new(),
            disagree: 0,
        }
    }

    /// Starting volume (0–100) before anything is loaded.
    pub fn with_volume(mut self, volume: u8) -> Self {
        self.volume = volume.min(100);
        self
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            status: self.status,
            track_id: self.queue.current().map(str::to_owned),
            source: self.active.clone(),
            volume: self.volume,
            repeat: self.queue.repeat(),
            shuffle: self.queue.shuffle(),
            queue_length: u32::try_from(self.queue.len()).unwrap_or(u32::MAX),
        }
    }

    pub fn last_skips(&self) -> &[(SourceKind, UnavailableReason)] {
        &self.last_skips
    }

    /// Fallbacks since the last call (emitted as `player://fallback`).
    pub fn take_fallbacks(&mut self) -> Vec<FallbackEvent> {
        std::mem::take(&mut self.fallbacks)
    }

    /// playback_log rows since the last call.
    pub fn take_log(&mut self) -> Vec<LogEntry> {
        std::mem::take(&mut self.log)
    }

    fn active_engine(&self) -> Option<Arc<dyn Engine>> {
        self.active.as_ref().and_then(|s| self.engines.get(&s.kind).cloned())
    }

    /// Closes the current session into the log.
    fn end_session(&mut self, reason: EndedReason) {
        if let Some(s) = self.session.take() {
            self.log.push(LogEntry { track_id: s.track_id, provider: s.kind, played_ms: s.last_pos, reason });
        }
    }

    /// Replaces the queue with `track_ids` and plays from `start`, optionally from one source.
    pub async fn play(&mut self, track_ids: Vec<String>, start: usize, force: Option<SourceKind>) -> AppResult<()> {
        self.end_session(EndedReason::Skipped);
        self.queue.set(track_ids, start);
        self.force = force;
        self.load_current(&[]).await
    }

    /// Loads the current queue item through the first engine that can play it, skipping
    /// `exclude` (sources that already failed for this item).
    async fn load_current(&mut self, exclude: &[PlaySource]) -> AppResult<()> {
        let Some(track_id) = self.queue.current().map(str::to_owned) else {
            self.stop_active().await?;
            self.status = PlayerStatus::Idle;
            return Ok(());
        };
        self.status = PlayerStatus::Loading;
        self.last_skips.clear();
        let policy: Policy = self.candidates.policy().await;
        let force = self.force.take();
        let candidates: Vec<PlaySource> = self
            .candidates
            .candidates(&track_id)
            .await?
            .into_iter()
            .filter(|c| force.is_none_or(|f| c.kind == f))
            .filter(|c| !exclude.contains(c))
            .collect();

        let mut first_failure: Option<(SourceKind, UnavailableReason)> = None;
        let mut last_error: Option<AppError> = None;
        let count = candidates.len();
        for source in candidates {
            if first_failure.is_some() && !policy.auto_fallback {
                break;
            }
            let Some(engine) = self.engines.get(&source.kind).cloned() else {
                first_failure.get_or_insert((source.kind, UnavailableReason::NotConnected));
                self.last_skips.push((source.kind, UnavailableReason::NotConnected));
                continue;
            };
            if let Err(reason) = engine.can_play(&source).await {
                first_failure.get_or_insert((source.kind, reason.clone()));
                self.last_skips.push((source.kind, reason));
                continue;
            }
            // Only one engine plays at a time: pause the previous one when switching.
            if let Some(prev) = self.active_engine() {
                if !Arc::ptr_eq(&prev, &engine) {
                    prev.pause().await?;
                }
            }
            engine.set_volume(self.volume).await?;
            if let Err(err) = engine.load(&source, 0).await {
                let reason = reason_for(&err);
                tracing::info!(kind = ?source.kind, error = %err, "source failed to load");
                first_failure.get_or_insert((source.kind, reason.clone()));
                self.last_skips.push((source.kind, reason));
                last_error = Some(err);
                continue;
            }
            if let Some((from, reason)) = first_failure.take() {
                if from != source.kind {
                    self.log.push(LogEntry { track_id: track_id.clone(), provider: from, played_ms: 0, reason: EndedReason::Fallback });
                    self.fallbacks.push(FallbackEvent { track_id: track_id.clone(), from_provider: from, to_provider: source.kind, reason });
                }
            }
            self.session = Some(Session { track_id: track_id.clone(), kind: source.kind, last_pos: 0 });
            self.active = Some(source);
            self.status = PlayerStatus::Playing;
            self.disagree = 0;
            return Ok(());
        }

        self.stop_active().await?;
        self.status = PlayerStatus::Error;
        // One source that failed with a specific reason says more than "no playable source".
        match last_error {
            Some(err) if count == 1 => Err(err),
            _ => Err(AppError::new(ErrorCode::NoPlayableSource, "no source of this track can play here")),
        }
    }

    async fn stop_active(&mut self) -> AppResult<()> {
        if let Some(engine) = self.active_engine() {
            engine.stop().await?;
        }
        self.active = None;
        Ok(())
    }

    pub async fn pause(&mut self) -> AppResult<()> {
        if let Some(engine) = self.active_engine() {
            engine.pause().await?;
            self.status = PlayerStatus::Paused;
            self.disagree = 0;
        }
        Ok(())
    }

    pub async fn resume(&mut self) -> AppResult<()> {
        if let Some(engine) = self.active_engine() {
            engine.play().await?;
            self.status = PlayerStatus::Playing;
            self.disagree = 0;
        }
        Ok(())
    }

    pub async fn toggle(&mut self) -> AppResult<()> {
        match self.status {
            PlayerStatus::Playing => self.pause().await,
            PlayerStatus::Paused => self.resume().await,
            _ => Ok(()),
        }
    }

    pub async fn next(&mut self) -> AppResult<()> {
        self.end_session(EndedReason::Skipped);
        if self.queue.next(true).is_some() {
            self.load_current(&[]).await
        } else {
            self.stop_active().await?;
            self.status = PlayerStatus::Idle;
            Ok(())
        }
    }

    /// Restarts the track if it has played a few seconds, otherwise goes to the previous one.
    pub async fn prev(&mut self) -> AppResult<()> {
        if let Some(engine) = self.active_engine() {
            if engine.state().await?.position_ms > RESTART_THRESHOLD_MS {
                return engine.seek(0).await;
            }
        }
        self.end_session(EndedReason::Skipped);
        self.queue.prev();
        self.load_current(&[]).await
    }

    /// Called when the active engine reports the end of the track: auto-advance.
    pub async fn on_ended(&mut self) -> AppResult<()> {
        self.end_session(EndedReason::Finished);
        if self.queue.next(false).is_some() {
            self.load_current(&[]).await
        } else {
            self.stop_active().await?;
            self.status = PlayerStatus::Idle;
            Ok(())
        }
    }

    /// Periodic check by the service: tracks position, advances at the end, falls back to
    /// another source of the same track when the engine failed mid-track.
    pub async fn tick(&mut self) -> AppResult<Option<EngineState>> {
        if !matches!(self.status, PlayerStatus::Playing | PlayerStatus::Paused) {
            return Ok(None);
        }
        let Some(engine) = self.active_engine() else { return Ok(None) };
        let state = engine.state().await?;
        if let Some(s) = self.session.as_mut() {
            s.last_pos = s.last_pos.max(state.position_ms);
        }
        if state.failed {
            let failed = self.active.clone();
            let policy = self.candidates.policy().await;
            if let (Some(failed), true) = (failed, policy.fallback_on_error_midtrack) {
                let track_id = self.queue.current().unwrap_or_default().to_owned();
                self.end_session(EndedReason::Fallback);
                self.stop_active().await?;
                let before = self.fallbacks.len();
                let result = self.load_current(std::slice::from_ref(&failed)).await;
                if result.is_ok() && self.fallbacks.len() == before {
                    if let Some(to) = self.active.as_ref().map(|a| a.kind) {
                        self.fallbacks.push(FallbackEvent {
                            track_id,
                            from_provider: failed.kind,
                            to_provider: to,
                            reason: UnavailableReason::Offline,
                        });
                    }
                }
                result?;
            } else {
                self.end_session(EndedReason::Error);
                self.on_ended().await?;
            }
            return Ok(None);
        }
        if state.ended {
            self.on_ended().await?;
            return Ok(None);
        }
        if state.playing == (self.status == PlayerStatus::Playing) {
            self.disagree = 0;
        } else {
            self.disagree += 1;
            if self.disagree >= FOLLOW_TICKS {
                self.status = if state.playing { PlayerStatus::Playing } else { PlayerStatus::Paused };
                self.disagree = 0;
            }
        }
        Ok(Some(state))
    }

    pub async fn seek(&mut self, position_ms: u32) -> AppResult<()> {
        match self.active_engine() {
            Some(engine) => engine.seek(position_ms).await,
            None => Ok(()),
        }
    }

    /// One volume for all engines (each maps 0–100 to its own scale).
    pub async fn set_volume(&mut self, volume: u8) -> AppResult<()> {
        self.volume = volume.min(100);
        if let Some(engine) = self.active_engine() {
            engine.set_volume(self.volume).await?;
        }
        Ok(())
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.queue.set_shuffle(on);
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.queue.set_repeat(repeat);
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Current engine state (position / duration) for the UI.
    pub async fn engine_state(&self) -> AppResult<Option<EngineState>> {
        match self.active_engine() {
            Some(e) => e.state().await.map(Some),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::playback::queue::Rng;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Records every call; `blocked` items fail `can_play`, `load_error` makes `load` fail,
    /// `failed` makes `state()` report a failure.
    pub struct FakeEngine {
        pub kind: SourceKind,
        pub log: Arc<Mutex<Vec<String>>>,
        pub blocked: Vec<(String, UnavailableReason)>,
        pub load_error: Option<ErrorCode>,
        pub position_ms: Mutex<u32>,
        pub failed: Mutex<bool>,
        pub ended: Mutex<bool>,
    }

    impl FakeEngine {
        pub fn new(kind: SourceKind, log: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
            Arc::new(Self::plain(kind, log))
        }

        pub fn plain(kind: SourceKind, log: Arc<Mutex<Vec<String>>>) -> Self {
            Self {
                kind,
                log,
                blocked: Vec::new(),
                load_error: None,
                position_ms: Mutex::new(0),
                failed: Mutex::new(false),
                ended: Mutex::new(false),
            }
        }

        fn rec(&self, what: String) {
            self.log.lock().unwrap().push(format!("{:?}:{what}", self.kind));
        }
    }

    #[async_trait]
    impl Engine for FakeEngine {
        fn kinds(&self) -> &'static [SourceKind] {
            match self.kind {
                SourceKind::Local => &[SourceKind::Local],
                SourceKind::Spotify => &[SourceKind::Spotify],
                SourceKind::Youtube => &[SourceKind::Youtube],
                SourceKind::Audius => &[SourceKind::Audius],
            }
        }
        async fn can_play(&self, source: &PlaySource) -> Result<(), UnavailableReason> {
            match self.blocked.iter().find(|(id, _)| *id == source.item_id || id == "*") {
                Some((_, reason)) => Err(reason.clone()),
                None => Ok(()),
            }
        }
        async fn load(&self, source: &PlaySource, _start: u32) -> AppResult<()> {
            if let Some(code) = &self.load_error {
                return Err(AppError::new(code.clone(), "fake load error"));
            }
            *self.position_ms.lock().unwrap() = 0;
            *self.failed.lock().unwrap() = false;
            *self.ended.lock().unwrap() = false;
            self.rec(format!("load {}", source.item_id));
            Ok(())
        }
        async fn play(&self) -> AppResult<()> {
            self.rec("play".into());
            Ok(())
        }
        async fn pause(&self) -> AppResult<()> {
            self.rec("pause".into());
            Ok(())
        }
        async fn stop(&self) -> AppResult<()> {
            self.rec("stop".into());
            Ok(())
        }
        async fn seek(&self, ms: u32) -> AppResult<()> {
            *self.position_ms.lock().unwrap() = ms;
            self.rec(format!("seek {ms}"));
            Ok(())
        }
        async fn set_volume(&self, v: u8) -> AppResult<()> {
            self.rec(format!("volume {v}"));
            Ok(())
        }
        async fn state(&self) -> AppResult<EngineState> {
            Ok(EngineState {
                position_ms: *self.position_ms.lock().unwrap(),
                playing: true,
                failed: *self.failed.lock().unwrap(),
                ended: *self.ended.lock().unwrap(),
                ..EngineState::default()
            })
        }
    }

    /// Track "tN" → candidates by a fixed map; default: one local source "tN-local".
    pub struct FixedCandidates(pub HashMap<String, Vec<PlaySource>>, pub Policy);

    #[async_trait]
    impl Candidates for FixedCandidates {
        async fn candidates(&self, track_id: &str) -> AppResult<Vec<PlaySource>> {
            Ok(self.0.get(track_id).cloned().unwrap_or_else(|| {
                vec![PlaySource { kind: SourceKind::Local, item_id: format!("{track_id}-local"), url: String::new() }]
            }))
        }
        async fn policy(&self) -> Policy {
            self.1
        }
    }

    pub fn src(kind: SourceKind, id: &str) -> PlaySource {
        PlaySource { kind, item_id: id.into(), url: String::new() }
    }

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("t{i}")).collect()
    }

    type Log = Arc<Mutex<Vec<String>>>;

    fn player_with(engines: Vec<Arc<dyn Engine>>, map: HashMap<String, Vec<PlaySource>>, policy: Policy) -> Player {
        Player::new(engines, Arc::new(FixedCandidates(map, policy)), Queue::new(Rng::seeded(3)))
    }

    fn player(map: HashMap<String, Vec<PlaySource>>, blocked_yt: Vec<String>) -> (Player, Log) {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let local = FakeEngine::new(SourceKind::Local, log.clone());
        let yt = Arc::new(FakeEngine {
            blocked: blocked_yt.into_iter().map(|b| (b, UnavailableReason::NotEmbeddable)).collect(),
            ..FakeEngine::plain(SourceKind::Youtube, log.clone())
        });
        (player_with(vec![local, yt], map, Policy::default()), log)
    }

    fn loads(log: &Log) -> Vec<String> {
        log.lock().unwrap().iter().filter(|l| l.contains("load")).cloned().collect()
    }

    #[tokio::test]
    async fn plays_next_prev_and_auto_advances_to_idle() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.play(ids(3), 0, None).await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Playing);
        p.next().await.unwrap();
        p.on_ended().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t2"));
        p.prev().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t1"));
        assert_eq!(loads(&log), vec!["Local:load t0-local", "Local:load t1-local", "Local:load t2-local", "Local:load t1-local"]);
        p.next().await.unwrap();
        p.on_ended().await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Idle, "end of queue");
        assert!(log.lock().unwrap().last().unwrap().ends_with("stop"));
        let reasons: Vec<_> = p.take_log().into_iter().map(|l| l.reason).collect();
        assert_eq!(reasons, vec![EndedReason::Skipped, EndedReason::Finished, EndedReason::Skipped, EndedReason::Skipped, EndedReason::Finished]);
    }

    #[tokio::test]
    async fn prev_restarts_after_three_seconds() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.play(ids(2), 1, None).await.unwrap();
        p.seek(10_000).await.unwrap();
        p.prev().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t1"), "restarted, not moved");
        assert!(log.lock().unwrap().last().unwrap().ends_with("seek 0"));
    }

    #[tokio::test]
    async fn repeat_one_and_shuffle_through_the_player() {
        let (mut p, _log) = player(HashMap::new(), vec![]);
        p.play(ids(4), 0, None).await.unwrap();
        p.set_repeat(RepeatMode::One);
        p.on_ended().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t0"));
        p.set_repeat(RepeatMode::Off);
        p.set_shuffle(true);
        let mut seen = vec![p.snapshot().track_id.unwrap()];
        for _ in 0..3 {
            p.on_ended().await.unwrap();
            seen.push(p.snapshot().track_id.unwrap());
        }
        seen.sort();
        assert_eq!(seen, ids(4));
        assert!(p.snapshot().shuffle);
    }

    #[tokio::test]
    async fn picks_first_playable_candidate_and_pauses_previous_engine() {
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Local, "f0")]);
        map.insert("t1".to_owned(), vec![src(SourceKind::Youtube, "blocked"), src(SourceKind::Youtube, "v1")]);
        let (mut p, log) = player(map, vec!["blocked".into()]);
        p.play(ids(2), 0, None).await.unwrap();
        p.next().await.unwrap();
        assert_eq!(p.snapshot().source, Some(src(SourceKind::Youtube, "v1")));
        assert_eq!(p.last_skips(), &[(SourceKind::Youtube, UnavailableReason::NotEmbeddable)]);
        let entries = log.lock().unwrap().clone();
        let pause_local = entries.iter().position(|l| l == "Local:pause").unwrap();
        let load_yt = entries.iter().position(|l| l == "Youtube:load v1").unwrap();
        assert!(pause_local < load_yt, "previous engine paused before the new one loads");
    }

    #[tokio::test]
    async fn volume_toggle_and_seek_go_to_the_active_engine() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.toggle().await.unwrap(); // idle: no-op
        p.play(ids(1), 0, None).await.unwrap();
        p.set_volume(250).await.unwrap();
        assert_eq!(p.snapshot().volume, 100);
        p.toggle().await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Paused);
        p.toggle().await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Playing);
        let entries = log.lock().unwrap().clone();
        assert!(entries.contains(&"Local:volume 100".to_owned()));
        assert!(entries.ends_with(&["Local:pause".to_owned(), "Local:play".to_owned()]));
    }

    // ---- Fallback test matrix (addendum UP-09) ----

    fn matrix_engines(log: &Log, spotify_premium: bool, youtube_blocked: bool) -> Vec<Arc<dyn Engine>> {
        let spotify = FakeEngine {
            blocked: if spotify_premium { vec![] } else { vec![("*".into(), UnavailableReason::PremiumRequired)] },
            ..FakeEngine::plain(SourceKind::Spotify, log.clone())
        };
        let youtube = FakeEngine {
            blocked: if youtube_blocked { vec![("*".into(), UnavailableReason::NotEmbeddable)] } else { vec![] },
            ..FakeEngine::plain(SourceKind::Youtube, log.clone())
        };
        vec![
            Arc::new(spotify),
            Arc::new(youtube),
            FakeEngine::new(SourceKind::Local, log.clone()),
            FakeEngine::new(SourceKind::Audius, log.clone()),
        ]
    }

    fn all_sources() -> HashMap<String, Vec<PlaySource>> {
        let mut map = HashMap::new();
        map.insert(
            "t0".to_owned(),
            vec![src(SourceKind::Spotify, "sp"), src(SourceKind::Youtube, "yt"), src(SourceKind::Local, "file"), src(SourceKind::Audius, "au")],
        );
        map
    }

    #[tokio::test]
    async fn free_spotify_user_plays_via_youtube_and_logs_a_fallback() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let mut p = player_with(matrix_engines(&log, false, false), all_sources(), Policy::default());
        p.play(ids(1), 0, None).await.unwrap();
        assert_eq!(p.snapshot().source.map(|s| s.kind), Some(SourceKind::Youtube));
        let fb = p.take_fallbacks();
        assert_eq!(fb.len(), 1);
        assert_eq!((fb[0].from_provider, fb[0].to_provider, fb[0].reason.clone()), (SourceKind::Spotify, SourceKind::Youtube, UnavailableReason::PremiumRequired));
        let rows = p.take_log();
        assert_eq!(rows[0].reason, EndedReason::Fallback);
        assert_eq!(rows[0].provider, SourceKind::Spotify);
    }

    #[tokio::test]
    async fn blocked_youtube_falls_to_local() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Youtube, "yt"), src(SourceKind::Local, "file"), src(SourceKind::Audius, "au")]);
        let mut p = player_with(matrix_engines(&log, true, true), map, Policy::default());
        p.play(ids(1), 0, None).await.unwrap();
        assert_eq!(p.snapshot().source.map(|s| s.kind), Some(SourceKind::Local));
        assert_eq!(p.take_fallbacks()[0].reason, UnavailableReason::NotEmbeddable);
    }

    #[tokio::test]
    async fn only_spotify_without_premium_is_no_playable_source() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Spotify, "sp")]);
        let mut p = player_with(matrix_engines(&log, false, false), map, Policy::default());
        let err = p.play(ids(1), 0, None).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NoPlayableSource);
        assert_eq!(p.snapshot().status, PlayerStatus::Error);
        assert_eq!(p.last_skips(), &[(SourceKind::Spotify, UnavailableReason::PremiumRequired)]);
    }

    #[tokio::test]
    async fn a_failed_load_falls_back_and_a_single_failure_keeps_its_error() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let spotify = FakeEngine { load_error: Some(ErrorCode::NoSpotifyDevice), ..FakeEngine::plain(SourceKind::Spotify, log.clone()) };
        let engines: Vec<Arc<dyn Engine>> = vec![Arc::new(spotify), FakeEngine::new(SourceKind::Youtube, log.clone())];
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Spotify, "sp"), src(SourceKind::Youtube, "yt")]);
        map.insert("t1".to_owned(), vec![src(SourceKind::Spotify, "sp1")]);
        let mut p = player_with(engines, map, Policy::default());
        p.play(ids(2), 0, None).await.unwrap();
        assert_eq!(p.snapshot().source.map(|s| s.kind), Some(SourceKind::Youtube));
        assert_eq!(p.take_fallbacks()[0].reason, UnavailableReason::NoDevice);
        let err = p.next().await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NoSpotifyDevice, "the specific reason is kept");
    }

    #[tokio::test]
    async fn auto_fallback_off_stops_at_the_first_source() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let policy = Policy { auto_fallback: false, ..Policy::default() };
        let mut p = player_with(matrix_engines(&log, false, false), all_sources(), policy);
        assert_eq!(p.play(ids(1), 0, None).await.unwrap_err().code, ErrorCode::NoPlayableSource);
    }

    #[tokio::test]
    async fn forced_source_plays_only_that_kind() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let mut p = player_with(matrix_engines(&log, true, false), all_sources(), Policy::default());
        p.play(ids(1), 0, Some(SourceKind::Audius)).await.unwrap();
        assert_eq!(p.snapshot().source.map(|s| s.kind), Some(SourceKind::Audius));
        assert!(p.take_fallbacks().is_empty());
    }

    #[tokio::test]
    async fn mid_track_failure_falls_back_to_the_next_source_of_the_same_song() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let local = FakeEngine::new(SourceKind::Local, log.clone());
        let audius = Arc::new(FakeEngine::plain(SourceKind::Audius, log.clone()));
        let engines: Vec<Arc<dyn Engine>> = vec![audius.clone(), local];
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Audius, "au"), src(SourceKind::Local, "file")]);
        let mut p = player_with(engines, map, Policy::default());
        p.play(ids(2), 0, None).await.unwrap();
        *audius.position_ms.lock().unwrap() = 42_000;
        assert!(p.tick().await.unwrap().is_some());
        *audius.failed.lock().unwrap() = true;
        p.tick().await.unwrap();
        let snap = p.snapshot();
        assert_eq!(snap.track_id.as_deref(), Some("t0"), "same song, not a substitute");
        assert_eq!(snap.source.map(|s| s.kind), Some(SourceKind::Local));
        let fb = p.take_fallbacks();
        assert_eq!((fb[0].from_provider, fb[0].to_provider), (SourceKind::Audius, SourceKind::Local));
        let rows = p.take_log();
        assert_eq!(rows[0], LogEntry { track_id: "t0".into(), provider: SourceKind::Audius, played_ms: 42_000, reason: EndedReason::Fallback });
    }

    #[tokio::test]
    async fn follows_the_engine_only_after_it_disagrees_for_a_few_ticks() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let local = Arc::new(FakeEngine::plain(SourceKind::Local, log.clone()));
        let mut p = player_with(vec![local.clone()], HashMap::new(), Policy::default());
        p.play(ids(1), 0, None).await.unwrap();
        p.pause().await.unwrap();
        // FakeEngine always reports playing: a stale "playing" must not undo the pause at once.
        p.tick().await.unwrap();
        p.tick().await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Paused);
        p.tick().await.unwrap();
        assert_eq!(p.snapshot().status, PlayerStatus::Playing, "resumed outside Unison");
    }

    #[tokio::test]
    async fn tick_advances_when_the_engine_reports_the_end() {
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let local = Arc::new(FakeEngine::plain(SourceKind::Local, log.clone()));
        let mut p = player_with(vec![local.clone()], HashMap::new(), Policy::default());
        p.play(ids(2), 0, None).await.unwrap();
        *local.ended.lock().unwrap() = true;
        p.tick().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t1"));
    }
}
