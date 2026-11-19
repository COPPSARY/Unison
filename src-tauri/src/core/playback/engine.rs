//! Playback engines (addendum §1): NativeEngine (local + Audius), YouTubeEngine (IFrame),
//! SpotifyConnectEngine (remote control). The player talks to all of them through this trait.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::AppResult;
use crate::model::SourceKind;

/// One concrete way to play a track: which engine and which item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaySource {
    pub kind: SourceKind,
    /// Spotify track id, YouTube video id, Audius track id or local fingerprint.
    pub item_id: String,
    /// Spotify/YouTube/Audius URL or absolute file path.
    pub url: String,
}

/// Why a source can't play right now (shown in "Play from…" and Settings → Playback).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UnavailableReason {
    NotConnected,
    PremiumRequired,
    NoDevice,
    NotEmbeddable,
    FileMissing,
    UnsupportedFormat,
    Gated,
    /// Turned off in Settings (e.g. Spotify mode "Never").
    Disabled,
    Offline,
    /// The track has no source of this kind.
    NoSource,
}

/// What `player_sources` returns per source kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub kind: SourceKind,
    pub source: Option<PlaySource>,
    pub can_play: bool,
    pub reason: Option<UnavailableReason>,
}

/// Snapshot of an engine's playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineState {
    pub position_ms: u32,
    pub duration_ms: Option<u32>,
    pub playing: bool,
    pub ended: bool,
    /// The item stopped because of an error (e.g. a dropped stream), not because it finished.
    pub failed: bool,
}

#[async_trait]
pub trait Engine: Send + Sync {
    /// Source kinds this engine plays (NativeEngine: local files and Audius streams).
    fn kinds(&self) -> &'static [SourceKind];

    /// Cheap check before loading (Premium/device, embeddable, file exists, reachable).
    async fn can_play(&self, source: &PlaySource) -> Result<(), UnavailableReason>;

    /// Loads (and starts) `source` at `start_ms`.
    async fn load(&self, source: &PlaySource, start_ms: u32) -> AppResult<()>;

    async fn play(&self) -> AppResult<()>;

    async fn pause(&self) -> AppResult<()>;

    /// Stops and releases the current item.
    async fn stop(&self) -> AppResult<()>;

    async fn seek(&self, position_ms: u32) -> AppResult<()>;

    /// 0–100; each engine maps it to its own scale.
    async fn set_volume(&self, volume: u8) -> AppResult<()>;

    async fn state(&self) -> AppResult<EngineState>;
}

/// Ordered playable candidates for a track. UP-09's resolver implements this from
/// track_sources + settings; tests use fixed lists.
#[async_trait]
pub trait Candidates: Send + Sync {
    async fn candidates(&self, track_id: &str) -> AppResult<Vec<PlaySource>>;
}
