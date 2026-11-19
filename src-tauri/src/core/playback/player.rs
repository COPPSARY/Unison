//! Player state machine (addendum §1). Rust owns the queue and decides which engine plays:
//! for each track it takes the ordered candidates, asks each engine `can_play`, and loads the
//! first that can. Only one engine plays at a time.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::engine::{Candidates, Engine, PlaySource, UnavailableReason};
use super::queue::{Queue, RepeatMode};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::SourceKind;

/// "Previous" restarts the current track when it has played longer than this.
const RESTART_THRESHOLD_MS: u32 = 3_000;

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

/// What the UI sees (sent as `player://state` from UP-09 on).
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

pub struct Player {
    engines: HashMap<SourceKind, Arc<dyn Engine>>,
    candidates: Arc<dyn Candidates>,
    queue: Queue,
    status: PlayerStatus,
    active: Option<PlaySource>,
    volume: u8,
    /// Why each candidate of the last failed load was skipped (for logs / UI).
    last_skips: Vec<(SourceKind, UnavailableReason)>,
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
        }
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

    fn active_engine(&self) -> Option<Arc<dyn Engine>> {
        self.active.as_ref().and_then(|s| self.engines.get(&s.kind).cloned())
    }

    /// Replaces the queue with `track_ids` and plays from `start`.
    pub async fn play(&mut self, track_ids: Vec<String>, start: usize) -> AppResult<()> {
        self.queue.set(track_ids, start);
        self.load_current().await
    }

    /// Loads the current queue item through the first engine that can play it.
    async fn load_current(&mut self) -> AppResult<()> {
        let Some(track_id) = self.queue.current().map(str::to_owned) else {
            self.stop_active().await?;
            self.status = PlayerStatus::Idle;
            return Ok(());
        };
        self.status = PlayerStatus::Loading;
        self.last_skips.clear();
        let candidates = self.candidates.candidates(&track_id).await?;

        for source in candidates {
            let Some(engine) = self.engines.get(&source.kind).cloned() else {
                self.last_skips.push((source.kind, UnavailableReason::NotConnected));
                continue;
            };
            if let Err(reason) = engine.can_play(&source).await {
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
            engine.load(&source, 0).await?;
            self.active = Some(source);
            self.status = PlayerStatus::Playing;
            return Ok(());
        }

        self.stop_active().await?;
        self.status = PlayerStatus::Error;
        Err(AppError::new(ErrorCode::NoPlayableSource, "no source of this track can play here"))
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
        }
        Ok(())
    }

    pub async fn resume(&mut self) -> AppResult<()> {
        if let Some(engine) = self.active_engine() {
            engine.play().await?;
            self.status = PlayerStatus::Playing;
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
        if self.queue.next(true).is_some() {
            self.load_current().await
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
        self.queue.prev();
        self.load_current().await
    }

    /// Called when the active engine reports the end of the track: auto-advance.
    pub async fn on_ended(&mut self) -> AppResult<()> {
        if self.queue.next(false).is_some() {
            self.load_current().await
        } else {
            self.stop_active().await?;
            self.status = PlayerStatus::Idle;
            Ok(())
        }
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
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::core::playback::engine::EngineState;
    use crate::core::playback::queue::Rng;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Records every call; `blocked` items fail `can_play`.
    pub struct FakeEngine {
        pub kind: SourceKind,
        pub log: Arc<Mutex<Vec<String>>>,
        pub blocked: Vec<String>,
        pub position_ms: Mutex<u32>,
    }

    impl FakeEngine {
        pub fn new(kind: SourceKind, log: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
            Arc::new(Self { kind, log, blocked: Vec::new(), position_ms: Mutex::new(0) })
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
            if self.blocked.contains(&source.item_id) {
                Err(UnavailableReason::NotEmbeddable)
            } else {
                Ok(())
            }
        }
        async fn load(&self, source: &PlaySource, _start: u32) -> AppResult<()> {
            *self.position_ms.lock().unwrap() = 0;
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
            Ok(EngineState { position_ms: *self.position_ms.lock().unwrap(), ..EngineState::default() })
        }
    }

    /// Track "tN" → candidates by a fixed map; default: one local source "tN-local".
    pub struct FixedCandidates(pub HashMap<String, Vec<PlaySource>>);

    #[async_trait]
    impl Candidates for FixedCandidates {
        async fn candidates(&self, track_id: &str) -> AppResult<Vec<PlaySource>> {
            Ok(self.0.get(track_id).cloned().unwrap_or_else(|| {
                vec![PlaySource { kind: SourceKind::Local, item_id: format!("{track_id}-local"), url: String::new() }]
            }))
        }
    }

    pub fn src(kind: SourceKind, id: &str) -> PlaySource {
        PlaySource { kind, item_id: id.into(), url: String::new() }
    }

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("t{i}")).collect()
    }

    fn player(map: HashMap<String, Vec<PlaySource>>, blocked_yt: Vec<String>) -> (Player, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let local = FakeEngine::new(SourceKind::Local, log.clone());
        let yt = Arc::new(FakeEngine { kind: SourceKind::Youtube, log: log.clone(), blocked: blocked_yt, position_ms: Mutex::new(0) });
        let p = Player::new(vec![local, yt], Arc::new(FixedCandidates(map)), Queue::new(Rng::seeded(3)));
        (p, log)
    }

    fn loads(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().iter().filter(|l| l.contains("load")).cloned().collect()
    }

    #[tokio::test]
    async fn plays_next_prev_and_auto_advances_to_idle() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.play(ids(3), 0).await.unwrap();
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
    }

    #[tokio::test]
    async fn prev_restarts_after_three_seconds() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.play(ids(2), 1).await.unwrap();
        p.seek(10_000).await.unwrap();
        p.prev().await.unwrap();
        assert_eq!(p.snapshot().track_id.as_deref(), Some("t1"), "restarted, not moved");
        assert!(log.lock().unwrap().last().unwrap().ends_with("seek 0"));
    }

    #[tokio::test]
    async fn repeat_one_and_shuffle_through_the_player() {
        let (mut p, _log) = player(HashMap::new(), vec![]);
        p.play(ids(4), 0).await.unwrap();
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
        p.play(ids(2), 0).await.unwrap();
        p.next().await.unwrap();
        let snap = p.snapshot();
        assert_eq!(snap.source, Some(src(SourceKind::Youtube, "v1")));
        assert_eq!(p.last_skips(), &[(SourceKind::Youtube, UnavailableReason::NotEmbeddable)]);
        let entries = log.lock().unwrap().clone();
        let pause_local = entries.iter().position(|l| l == "Local:pause").unwrap();
        let load_yt = entries.iter().position(|l| l == "Youtube:load v1").unwrap();
        assert!(pause_local < load_yt, "previous engine paused before the new one loads");
    }

    #[tokio::test]
    async fn no_playable_source_is_an_error_state() {
        let mut map = HashMap::new();
        map.insert("t0".to_owned(), vec![src(SourceKind::Spotify, "sp")]); // no Spotify engine
        let (mut p, _log) = player(map, vec![]);
        let err = p.play(ids(1), 0).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NoPlayableSource);
        assert_eq!(p.snapshot().status, PlayerStatus::Error);
        assert_eq!(p.last_skips(), &[(SourceKind::Spotify, UnavailableReason::NotConnected)]);
    }

    #[tokio::test]
    async fn volume_toggle_and_seek_go_to_the_active_engine() {
        let (mut p, log) = player(HashMap::new(), vec![]);
        p.toggle().await.unwrap(); // idle: no-op
        p.play(ids(1), 0).await.unwrap();
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
}
