//! SpotifyConnectEngine (addendum §1, UP-08): remote-controls the user's Spotify app through
//! the Web API player endpoints. Spotify plays the audio; Unison never streams it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Method, StatusCode};
use serde::Deserialize;

use super::client::SpotifyClient;
use super::player::{devices, PremiumCache};
use crate::core::playback::engine::{Engine, EngineState, PlaySource, UnavailableReason};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::SourceKind;

pub const POLL_EVERY: Duration = Duration::from_secs(1);
/// "Ended" when paused within this much of the end (spec §8.6).
const END_MARGIN_MS: u32 = 1_000;

#[derive(Debug, Clone, Default)]
struct Inner {
    state: EngineState,
    /// Spotify track id we started.
    track_id: Option<String>,
    device_id: Option<String>,
    /// Highest progress seen for this item (for "paused at 0 after progress").
    max_progress: u32,
}

#[derive(Deserialize)]
struct PlaybackJson {
    #[serde(default)]
    is_playing: bool,
    progress_ms: Option<u32>,
    item: Option<ItemJson>,
}

#[derive(Deserialize)]
struct ItemJson {
    id: Option<String>,
    duration_ms: Option<u32>,
}

#[derive(Deserialize, Default)]
struct ErrorBody {
    #[serde(default)]
    error: ErrorDetail,
}

#[derive(Deserialize, Default)]
struct ErrorDetail {
    #[serde(default)]
    message: String,
    #[serde(default)]
    reason: String,
}

pub struct SpotifyConnectEngine {
    client: Arc<SpotifyClient>,
    premium: Arc<PremiumCache>,
    /// Device chosen in Settings (`spotify_set_device`).
    chosen: Mutex<Option<String>>,
    inner: Arc<Mutex<Inner>>,
    poller: Mutex<Option<tokio::task::JoinHandle<()>>>,
    poll_every: Duration,
}

impl SpotifyConnectEngine {
    pub fn new(client: Arc<SpotifyClient>, premium: Arc<PremiumCache>) -> Self {
        Self::with_poll(client, premium, POLL_EVERY)
    }

    pub fn with_poll(client: Arc<SpotifyClient>, premium: Arc<PremiumCache>, poll_every: Duration) -> Self {
        Self {
            client,
            premium,
            chosen: Mutex::new(None),
            inner: Arc::new(Mutex::new(Inner::default())),
            poller: Mutex::new(None),
            poll_every,
        }
    }

    pub fn set_device(&self, device_id: Option<String>) {
        if let Ok(mut c) = self.chosen.lock() {
            *c = device_id;
        }
    }

    pub fn chosen_device(&self) -> Option<String> {
        self.chosen.lock().ok().and_then(|c| c.clone())
    }

    /// Chosen device if still listed, else the active one, else the first controllable one.
    async fn target_device(&self) -> AppResult<Option<String>> {
        let list = devices(&self.client).await?;
        let usable: Vec<_> = list.iter().filter(|d| d.id.is_some() && !d.is_restricted).collect();
        let chosen = self.chosen_device();
        let pick = usable
            .iter()
            .find(|d| d.id == chosen && chosen.is_some())
            .or_else(|| usable.iter().find(|d| d.is_active))
            .or_else(|| usable.first());
        Ok(pick.and_then(|d| d.id.clone()))
    }

    /// Sends a player command; maps Premium / device errors.
    async fn command(&self, method: Method, path: String, body: Option<serde_json::Value>) -> AppResult<()> {
        let response = self
            .client
            .send_allowing(method, &path, body.as_ref(), &[StatusCode::FORBIDDEN, StatusCode::NOT_FOUND])
            .await?;
        match response.status() {
            s if s.is_success() => Ok(()),
            StatusCode::FORBIDDEN => {
                let body: ErrorBody = response.json().await.unwrap_or_default();
                if body.error.reason.eq_ignore_ascii_case("PREMIUM_REQUIRED")
                    || body.error.message.to_ascii_lowercase().contains("premium")
                {
                    self.premium.set(false);
                    Err(AppError::new(ErrorCode::SpotifyPremiumRequired, "spotify playback needs Premium"))
                } else {
                    Err(AppError::new(ErrorCode::Upstream { status: 403 }, format!("spotify player: {}", body.error.message)))
                }
            }
            _ => Err(AppError::new(ErrorCode::NoSpotifyDevice, "no active Spotify device")),
        }
    }

    fn device_query(&self) -> String {
        self.inner
            .lock()
            .ok()
            .and_then(|i| i.device_id.clone())
            .map(|d| format!("device_id={}", url::form_urlencoded::byte_serialize(d.as_bytes()).collect::<String>()))
            .unwrap_or_default()
    }

    fn path(&self, base: &str, extra: &str) -> String {
        let q: Vec<String> = [self.device_query(), extra.to_owned()].into_iter().filter(|s| !s.is_empty()).collect();
        if q.is_empty() {
            base.to_owned()
        } else {
            format!("{base}?{}", q.join("&"))
        }
    }

    fn stop_polling(&self) {
        if let Ok(mut p) = self.poller.lock() {
            if let Some(handle) = p.take() {
                handle.abort();
            }
        }
    }

    fn start_polling(&self) {
        self.stop_polling();
        let client = self.client.clone();
        let inner = self.inner.clone();
        let every = self.poll_every;
        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                match poll_once(&client, &inner).await {
                    Ok(true) => continue,
                    Ok(false) => break, // ended: stop polling
                    Err(err) => tracing::debug!(error = %err, "spotify poll failed"),
                }
            }
        });
        if let Ok(mut p) = self.poller.lock() {
            *p = Some(handle);
        }
    }
}

/// One poll of GET /me/player. Returns false once the item has ended.
async fn poll_once(client: &SpotifyClient, inner: &Mutex<Inner>) -> AppResult<bool> {
    let response = client.send(Method::GET, "/me/player", None).await?;
    let playback: Option<PlaybackJson> = if response.status() == StatusCode::NO_CONTENT {
        None
    } else {
        response.json().await.ok()
    };
    let mut i = inner.lock().map_err(|_| AppError::internal("spotify engine lock poisoned"))?;
    if i.track_id.is_none() {
        return Ok(false);
    }
    apply_poll(&mut i, playback.as_ref());
    Ok(!i.state.ended)
}

/// Updates state from a playback snapshot (spec §8.6 ended rules).
fn apply_poll(i: &mut Inner, playback: Option<&PlaybackJson>) {
    let Some(p) = playback else {
        // Nothing playing anywhere any more: treat as ended once we had progress.
        i.state.playing = false;
        i.state.ended = i.max_progress > 0;
        return;
    };
    let progress = p.progress_ms.unwrap_or(0);
    let same_item = p.item.as_ref().and_then(|it| it.id.as_deref()) == i.track_id.as_deref();
    if let Some(d) = p.item.as_ref().and_then(|it| it.duration_ms).filter(|_| same_item) {
        i.state.duration_ms = Some(d);
    }
    let duration = i.state.duration_ms.unwrap_or(u32::MAX);

    let moved_on = !same_item && i.max_progress > 0;
    let reset_to_zero = same_item && !p.is_playing && progress == 0 && i.max_progress > 0;
    let paused_at_end = same_item && !p.is_playing && progress.saturating_add(END_MARGIN_MS) >= duration;
    if moved_on || reset_to_zero || paused_at_end {
        i.state.ended = true;
        i.state.playing = false;
        return;
    }
    if same_item {
        i.state.position_ms = progress;
        i.max_progress = i.max_progress.max(progress);
        i.state.playing = p.is_playing;
    }
}

#[async_trait]
impl Engine for SpotifyConnectEngine {
    fn kinds(&self) -> &'static [SourceKind] {
        &[SourceKind::Spotify]
    }

    async fn can_play(&self, source: &PlaySource) -> Result<(), UnavailableReason> {
        if source.kind != SourceKind::Spotify || source.item_id.is_empty() {
            return Err(UnavailableReason::NoSource);
        }
        if self.premium.get() == Some(false) {
            return Err(UnavailableReason::PremiumRequired);
        }
        match self.target_device().await {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(UnavailableReason::NoDevice),
            Err(e) if e.code == ErrorCode::Offline => Err(UnavailableReason::Offline),
            Err(_) => Err(UnavailableReason::NotConnected),
        }
    }

    async fn load(&self, source: &PlaySource, start_ms: u32) -> AppResult<()> {
        let device = self
            .target_device()
            .await?
            .ok_or_else(|| AppError::new(ErrorCode::NoSpotifyDevice, "no Spotify device"))?;
        if let Ok(mut i) = self.inner.lock() {
            *i = Inner {
                state: EngineState { position_ms: start_ms, ..EngineState::default() },
                track_id: Some(source.item_id.clone()),
                device_id: Some(device),
                max_progress: 0,
            };
        }
        let body = serde_json::json!({
            "uris": [format!("spotify:track:{}", source.item_id)],
            "position_ms": start_ms,
        });
        let result = self.command(Method::PUT, self.path("/me/player/play", ""), Some(body)).await;
        match result {
            Ok(()) => {
                if let Ok(mut i) = self.inner.lock() {
                    i.state.playing = true;
                }
                self.start_polling();
                Ok(())
            }
            Err(e) => {
                if let Ok(mut i) = self.inner.lock() {
                    *i = Inner::default();
                }
                Err(e)
            }
        }
    }

    async fn play(&self) -> AppResult<()> {
        self.command(Method::PUT, self.path("/me/player/play", ""), None).await?;
        self.start_polling();
        Ok(())
    }

    async fn pause(&self) -> AppResult<()> {
        let result = self.command(Method::PUT, self.path("/me/player/pause", ""), None).await;
        if let Ok(mut i) = self.inner.lock() {
            i.state.playing = false;
        }
        result
    }

    async fn stop(&self) -> AppResult<()> {
        self.stop_polling();
        let loaded = self.inner.lock().map(|i| i.track_id.is_some()).unwrap_or(false);
        let result = if loaded { self.pause().await } else { Ok(()) };
        if let Ok(mut i) = self.inner.lock() {
            *i = Inner::default();
        }
        // Stopping must not fail because Spotify was already paused or closed.
        if let Err(e) = result {
            tracing::debug!(error = %e, "spotify pause on stop failed");
        }
        Ok(())
    }

    async fn seek(&self, position_ms: u32) -> AppResult<()> {
        self.command(Method::PUT, self.path("/me/player/seek", &format!("position_ms={position_ms}")), None)
            .await?;
        if let Ok(mut i) = self.inner.lock() {
            i.state.position_ms = position_ms;
            i.state.ended = false;
        }
        Ok(())
    }

    async fn set_volume(&self, volume: u8) -> AppResult<()> {
        let path = self.path("/me/player/volume", &format!("volume_percent={}", volume.min(100)));
        // Some devices (phones) refuse remote volume; that must not stop playback.
        if let Err(e) = self.command(Method::PUT, path, None).await {
            tracing::debug!(error = %e, "spotify volume not applied");
        }
        Ok(())
    }

    async fn state(&self) -> AppResult<EngineState> {
        self.inner.lock().map(|i| i.state).map_err(|_| AppError::internal("spotify engine lock poisoned"))
    }
}

impl Drop for SpotifyConnectEngine {
    fn drop(&mut self) {
        self.stop_polling();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::spotify::playlists::tests::setup;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn track(id: &str) -> PlaySource {
        PlaySource { kind: SourceKind::Spotify, item_id: id.into(), url: format!("https://open.spotify.com/track/{id}") }
    }

    async fn mount_devices(server: &MockServer, devices: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path("/me/player/devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "devices": devices })))
            .mount(server)
            .await;
    }

    fn dev(id: &str, active: bool, restricted: bool) -> serde_json::Value {
        json!({ "id": id, "is_active": active, "is_restricted": restricted, "name": id, "type": "computer", "volume_percent": 50, "supports_volume": true })
    }

    async fn engine(server: &MockServer, client: SpotifyClient) -> SpotifyConnectEngine {
        let _ = server;
        SpotifyConnectEngine::with_poll(Arc::new(client), Arc::new(PremiumCache::default()), Duration::from_millis(20))
    }

    #[tokio::test]
    async fn plays_the_track_uri_on_the_chosen_device_and_polls_progress() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([dev("phone", true, false), dev("laptop", false, false)])).await;
        Mock::given(method("PUT"))
            .and(path("/me/player/play"))
            .and(query_param("device_id", "laptop"))
            .and(body_json(json!({ "uris": ["spotify:track:t1"], "position_ms": 5000 })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me/player"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "is_playing": true, "progress_ms": 7000, "item": { "id": "t1", "duration_ms": 200000 }
            })))
            .mount(&server)
            .await;

        let e = engine(&server, client).await;
        e.set_device(Some("laptop".into()));
        assert_eq!(e.can_play(&track("t1")).await, Ok(()));
        e.load(&track("t1"), 5_000).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;
        let s = e.state().await.unwrap();
        assert!(s.playing);
        assert_eq!((s.position_ms, s.duration_ms), (7_000, Some(200_000)));
        e.stop().await.ok();
    }

    #[tokio::test]
    async fn controls_target_the_same_device() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([dev("pc", true, false)])).await;
        Mock::given(method("PUT")).and(path("/me/player/play")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
        Mock::given(method("GET")).and(path("/me/player")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
        for (p, q) in [("/me/player/pause", None), ("/me/player/seek", Some(("position_ms", "90000"))), ("/me/player/volume", Some(("volume_percent", "30")))] {
            let mut m = Mock::given(method("PUT")).and(path(p)).and(query_param("device_id", "pc"));
            if let Some((k, v)) = q {
                m = m.and(query_param(k, v));
            }
            m.respond_with(ResponseTemplate::new(204)).expect(1..).mount(&server).await;
        }
        let e = engine(&server, client).await;
        e.load(&track("t1"), 0).await.unwrap();
        e.pause().await.unwrap();
        e.seek(90_000).await.unwrap();
        e.set_volume(30).await.unwrap();
        assert_eq!(e.state().await.unwrap().position_ms, 90_000);
        e.stop().await.unwrap();
        assert_eq!(e.state().await.unwrap(), EngineState::default());
    }

    #[test]
    fn ended_rules() {
        let base = || Inner {
            state: EngineState { duration_ms: Some(200_000), ..EngineState::default() },
            track_id: Some("t1".into()),
            device_id: None,
            max_progress: 50_000,
        };
        let snap = |playing: bool, progress: u32, id: &str| PlaybackJson {
            is_playing: playing,
            progress_ms: Some(progress),
            item: Some(ItemJson { id: Some(id.into()), duration_ms: Some(200_000) }),
        };

        let mut i = base();
        apply_poll(&mut i, Some(&snap(true, 60_000, "t1")));
        assert!(i.state.playing && !i.state.ended && i.state.position_ms == 60_000);

        let mut i = base();
        apply_poll(&mut i, Some(&snap(true, 1_000, "other")));
        assert!(i.state.ended, "Spotify moved on to another track");

        let mut i = base();
        apply_poll(&mut i, Some(&snap(false, 0, "t1")));
        assert!(i.state.ended, "paused at 0 after progress");

        let mut i = base();
        apply_poll(&mut i, Some(&snap(false, 199_500, "t1")));
        assert!(i.state.ended, "paused at the end");

        let mut i = base();
        apply_poll(&mut i, Some(&snap(false, 120_000, "t1")));
        assert!(!i.state.ended && !i.state.playing, "a normal pause is not the end");

        let mut i = Inner { max_progress: 0, ..base() };
        apply_poll(&mut i, Some(&snap(false, 0, "t1")));
        assert!(!i.state.ended, "not started yet");
    }

    #[tokio::test]
    async fn premium_required_on_play_marks_the_account_and_blocks_can_play() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([dev("pc", true, false)])).await;
        Mock::given(method("PUT"))
            .and(path("/me/player/play"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "error": { "status": 403, "message": "Player command failed: Premium required", "reason": "PREMIUM_REQUIRED" }
            })))
            .mount(&server)
            .await;
        let e = engine(&server, client).await;
        let err = e.load(&track("t1"), 0).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::SpotifyPremiumRequired);
        assert_eq!(e.can_play(&track("t1")).await, Err(UnavailableReason::PremiumRequired));
    }

    #[tokio::test]
    async fn no_controllable_device_is_no_device() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([dev("speaker", false, true)])).await;
        let e = engine(&server, client).await;
        assert_eq!(e.can_play(&track("t1")).await, Err(UnavailableReason::NoDevice));
        assert_eq!(e.load(&track("t1"), 0).await.unwrap_err().code, ErrorCode::NoSpotifyDevice);
    }

    #[tokio::test]
    async fn play_404_is_no_spotify_device() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([dev("pc", false, false)])).await;
        Mock::given(method("PUT"))
            .and(path("/me/player/play"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": { "status": 404, "message": "Device not found", "reason": "NO_ACTIVE_DEVICE" } })))
            .mount(&server)
            .await;
        let e = engine(&server, client).await;
        assert_eq!(e.load(&track("t1"), 0).await.unwrap_err().code, ErrorCode::NoSpotifyDevice);
    }
}
