//! YouTubeEngine (addendum §1, UP-07): drives the official IFrame player in the webview.
//! Commands go out as `player://youtube-command`; the `YouTubeDock` reports state back via
//! `youtube_player_report`. Rust keeps the authoritative state; the player is never hidden.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use super::engine::{Engine, EngineState, PlaySource, UnavailableReason};
use super::youtube::YoutubeCommand;
use crate::error::{AppError, AppResult};
use crate::model::SourceKind;

/// Where YouTube commands go: the Tauri app in production, a recorder in tests.
pub trait CommandSink: Send + Sync {
    fn send(&self, command: YoutubeCommand) -> AppResult<()>;
}

impl<R: tauri::Runtime> CommandSink for tauri::AppHandle<R> {
    fn send(&self, command: YoutubeCommand) -> AppResult<()> {
        command
            .emit(self)
            .map_err(|e| AppError::internal(format!("emit youtube command: {e}")))
    }
}

/// IFrame player states as reported by the UI (IFrame API -1, 0, 1, 2, 3, 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum YoutubePlayerState {
    Unstarted,
    Ended,
    Playing,
    Paused,
    Buffering,
    Cued,
}

#[derive(Debug, Clone, Default)]
struct Inner {
    state: EngineState,
    video_id: Option<String>,
    error_code: Option<u16>,
}

pub struct YouTubeEngine {
    sink: Arc<dyn CommandSink>,
    inner: Mutex<Inner>,
    host_ready: AtomicBool,
}

impl YouTubeEngine {
    pub fn new(sink: Arc<dyn CommandSink>) -> Self {
        Self { sink, inner: Mutex::new(Inner::default()), host_ready: AtomicBool::new(false) }
    }

    /// The webview's YouTubeDock mounted (true) or unmounted (false).
    pub fn set_host_ready(&self, ready: bool) {
        self.host_ready.store(ready, Ordering::SeqCst);
    }

    fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> AppResult<T> {
        let mut inner = self.inner.lock().map_err(|_| AppError::internal("youtube engine lock poisoned"))?;
        Ok(f(&mut inner))
    }

    /// State report from the IFrame player (`youtube_player_report`).
    pub fn report(
        &self,
        state: YoutubePlayerState,
        position_ms: u32,
        duration_ms: Option<u32>,
        error_code: Option<u16>,
    ) -> AppResult<()> {
        self.with(|inner| {
            if inner.video_id.is_none() {
                return; // stale report after stop
            }
            inner.state.position_ms = position_ms;
            if duration_ms.is_some() {
                inner.state.duration_ms = duration_ms;
            }
            if let Some(code) = error_code {
                inner.error_code = Some(code);
                inner.state.playing = false;
                inner.state.failed = true;
                inner.state.ended = true;
                return;
            }
            inner.state.playing = matches!(state, YoutubePlayerState::Playing | YoutubePlayerState::Buffering);
            inner.state.ended = state == YoutubePlayerState::Ended;
        })
    }

    /// IFrame error code of the current item, if it failed (e.g. 150 = embedding disabled).
    pub fn error_code(&self) -> Option<u16> {
        self.inner.lock().ok().and_then(|i| i.error_code)
    }
}

#[async_trait]
impl Engine for YouTubeEngine {
    fn kinds(&self) -> &'static [SourceKind] {
        &[SourceKind::Youtube]
    }

    async fn can_play(&self, source: &PlaySource) -> Result<(), UnavailableReason> {
        if source.kind != SourceKind::Youtube || source.item_id.is_empty() {
            return Err(UnavailableReason::NoSource);
        }
        if !self.host_ready.load(Ordering::SeqCst) {
            // The visible player isn't mounted, so YouTube can't play (never play hidden).
            return Err(UnavailableReason::NotConnected);
        }
        Ok(())
    }

    async fn load(&self, source: &PlaySource, start_ms: u32) -> AppResult<()> {
        self.with(|inner| {
            *inner = Inner {
                state: EngineState { position_ms: start_ms, ..EngineState::default() },
                video_id: Some(source.item_id.clone()),
                error_code: None,
            };
        })?;
        self.sink.send(YoutubeCommand::Load { video_id: source.item_id.clone(), start_ms })
    }

    async fn play(&self) -> AppResult<()> {
        self.sink.send(YoutubeCommand::Play)
    }

    async fn pause(&self) -> AppResult<()> {
        self.sink.send(YoutubeCommand::Pause)
    }

    async fn stop(&self) -> AppResult<()> {
        self.with(|inner| *inner = Inner::default())?;
        self.sink.send(YoutubeCommand::Stop)
    }

    async fn seek(&self, position_ms: u32) -> AppResult<()> {
        self.with(|inner| {
            inner.state.position_ms = position_ms;
            inner.state.ended = false;
        })?;
        self.sink.send(YoutubeCommand::Seek { position_ms })
    }

    async fn set_volume(&self, volume: u8) -> AppResult<()> {
        self.sink.send(YoutubeCommand::Volume { volume: volume.min(100) })
    }

    async fn state(&self) -> AppResult<EngineState> {
        self.with(|inner| inner.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<YoutubeCommand>>);

    impl CommandSink for Recorder {
        fn send(&self, command: YoutubeCommand) -> AppResult<()> {
            self.0.lock().unwrap().push(command);
            Ok(())
        }
    }

    fn engine() -> (YouTubeEngine, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        (YouTubeEngine::new(rec.clone()), rec)
    }

    fn video(id: &str) -> PlaySource {
        PlaySource { kind: SourceKind::Youtube, item_id: id.into(), url: format!("https://www.youtube.com/watch?v={id}") }
    }

    #[tokio::test]
    async fn needs_the_visible_host_before_it_can_play() {
        let (e, _) = engine();
        assert_eq!(e.can_play(&video("v1")).await, Err(UnavailableReason::NotConnected));
        e.set_host_ready(true);
        assert_eq!(e.can_play(&video("v1")).await, Ok(()));
        let local = PlaySource { kind: SourceKind::Local, item_id: "x".into(), url: String::new() };
        assert_eq!(e.can_play(&local).await, Err(UnavailableReason::NoSource));
    }

    #[tokio::test]
    async fn commands_go_to_the_webview() {
        let (e, rec) = engine();
        e.load(&video("v1"), 1_500).await.unwrap();
        e.pause().await.unwrap();
        e.play().await.unwrap();
        e.seek(30_000).await.unwrap();
        e.set_volume(150).await.unwrap();
        e.stop().await.unwrap();
        assert_eq!(
            *rec.0.lock().unwrap(),
            vec![
                YoutubeCommand::Load { video_id: "v1".into(), start_ms: 1_500 },
                YoutubeCommand::Pause,
                YoutubeCommand::Play,
                YoutubeCommand::Seek { position_ms: 30_000 },
                YoutubeCommand::Volume { volume: 100 },
                YoutubeCommand::Stop,
            ]
        );
    }

    #[tokio::test]
    async fn reports_drive_play_pause_seek_and_ended() {
        let (e, _) = engine();
        e.load(&video("v1"), 0).await.unwrap();
        e.report(YoutubePlayerState::Playing, 2_000, Some(212_000), None).unwrap();
        let s = e.state().await.unwrap();
        assert!(s.playing && !s.ended);
        assert_eq!((s.position_ms, s.duration_ms), (2_000, Some(212_000)));

        e.report(YoutubePlayerState::Paused, 2_500, None, None).unwrap();
        assert!(!e.state().await.unwrap().playing);

        e.seek(100_000).await.unwrap();
        assert_eq!(e.state().await.unwrap().position_ms, 100_000);

        e.report(YoutubePlayerState::Ended, 212_000, None, None).unwrap();
        let s = e.state().await.unwrap();
        assert!(s.ended && !s.failed);
    }

    #[tokio::test]
    async fn embed_errors_fail_the_item_and_stale_reports_are_ignored() {
        let (e, _) = engine();
        e.load(&video("v1"), 0).await.unwrap();
        e.report(YoutubePlayerState::Unstarted, 0, None, Some(150)).unwrap();
        let s = e.state().await.unwrap();
        assert!(s.failed && s.ended);
        assert_eq!(e.error_code(), Some(150));

        e.stop().await.unwrap();
        e.report(YoutubePlayerState::Playing, 9_000, None, None).unwrap();
        assert_eq!(e.state().await.unwrap(), EngineState::default());
    }
}
