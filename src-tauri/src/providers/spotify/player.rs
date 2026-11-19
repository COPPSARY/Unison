//! Spotify Connect capabilities (addendum UP-04): Premium detection and the device list.
//!
//! Documented facts used here (developer.spotify.com, checked 2026-11-19):
//! - `GET /me/player/devices` → `{ devices: [Device] }` (scope user-read-playback-state).
//! - `GET /me/player` → playback state incl. `device` and `repeat_state` (off|track|context).
//! - `PUT /me/player/repeat?state=` "only works for users who have Spotify Premium"; 204 on success.
//!
//! Not documented: the player error reason list and whether Premium or the active device is
//! checked first. So Premium is only probed when a device is active, by re-setting the repeat
//! state to its current value (no audible change), and anything ambiguous stays `None`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::client::SpotifyClient;
use crate::error::{AppError, AppResult};

/// Definite Premium answers are reused for an hour (addendum: "cache 1 h").
pub const PREMIUM_TTL: Duration = Duration::from_secs(3600);

/// A Spotify Connect device (subset of the documented DeviceObject).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyDevice {
    /// Nullable in the API; devices without an id can't be targeted.
    pub id: Option<String>,
    pub name: String,
    /// "computer", "smartphone", "speaker", …
    pub kind: String,
    pub is_active: bool,
    /// "If true then no Web API commands will be accepted by this device."
    pub is_restricted: bool,
    pub volume_percent: Option<u8>,
    pub supports_volume: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyCapabilities {
    /// `Some(true)` Premium, `Some(false)` not Premium, `None` unknown (no active device yet).
    pub premium: Option<bool>,
    pub devices: Vec<SpotifyDevice>,
}

#[derive(Deserialize)]
struct DeviceJson {
    id: Option<String>,
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_restricted: bool,
    volume_percent: Option<u8>,
    #[serde(default)]
    supports_volume: bool,
}

#[derive(Deserialize)]
struct DevicesJson {
    #[serde(default)]
    devices: Vec<DeviceJson>,
}

#[derive(Deserialize)]
struct PlaybackJson {
    device: Option<DeviceJson>,
    repeat_state: Option<String>,
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

impl From<DeviceJson> for SpotifyDevice {
    fn from(d: DeviceJson) -> Self {
        Self {
            id: d.id,
            name: d.name,
            kind: d.kind,
            is_active: d.is_active,
            is_restricted: d.is_restricted,
            volume_percent: d.volume_percent.map(|v| v.min(100)),
            supports_volume: d.supports_volume,
        }
    }
}

/// Current devices (always fresh: they come and go).
pub async fn devices(client: &SpotifyClient) -> AppResult<Vec<SpotifyDevice>> {
    let body: DevicesJson = client.get_json("/me/player/devices").await?;
    Ok(body.devices.into_iter().map(SpotifyDevice::from).collect())
}

/// Probes Premium without changing anything audible. `None` when it can't tell.
pub async fn probe_premium(client: &SpotifyClient) -> AppResult<Option<bool>> {
    // 204 (no body) or no active device: nothing safe to probe.
    let response = client.send_allowing(Method::GET, "/me/player", None, &[]).await?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let bytes = response.bytes().await.map_err(|e| AppError::internal(format!("spotify /me/player: {e}")))?;
    let playback: PlaybackJson = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let (Some(_device), Some(repeat)) = (playback.device, playback.repeat_state) else {
        return Ok(None);
    };
    if !matches!(repeat.as_str(), "off" | "track" | "context") {
        return Ok(None);
    }

    let path = format!("/me/player/repeat?state={repeat}");
    let response = client
        .send_allowing(Method::PUT, &path, None, &[StatusCode::FORBIDDEN, StatusCode::NOT_FOUND])
        .await?;
    match response.status() {
        s if s.is_success() => Ok(Some(true)),
        StatusCode::FORBIDDEN => {
            let body: ErrorBody = response.json().await.unwrap_or_default();
            let says_premium = body.error.reason.eq_ignore_ascii_case("PREMIUM_REQUIRED")
                || body.error.message.to_ascii_lowercase().contains("premium");
            Ok(says_premium.then_some(false))
        }
        _ => Ok(None),
    }
}

/// Caches definite Premium answers for [`PREMIUM_TTL`].
#[derive(Default)]
pub struct PremiumCache(Mutex<Option<(Instant, bool)>>);

impl PremiumCache {
    pub fn get(&self) -> Option<bool> {
        let slot = self.0.lock().ok()?;
        slot.and_then(|(at, value)| (at.elapsed() < PREMIUM_TTL).then_some(value))
    }

    pub fn set(&self, value: bool) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some((Instant::now(), value));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None;
        }
    }
}

/// Premium (cached or probed) + fresh device list.
pub async fn capabilities(client: &SpotifyClient, cache: &PremiumCache) -> AppResult<SpotifyCapabilities> {
    let devices = devices(client).await?;
    let premium = match cache.get() {
        Some(p) => Some(p),
        None => {
            let probed = probe_premium(client).await?;
            if let Some(p) = probed {
                cache.set(p);
            }
            probed
        }
    };
    Ok(SpotifyCapabilities { premium, devices })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::spotify::playlists::tests::setup;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mount_devices(server: &MockServer, devices: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path("/me/player/devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "devices": devices })))
            .mount(server)
            .await;
    }

    fn device(active: bool) -> serde_json::Value {
        json!({ "id": "dev1", "is_active": active, "is_private_session": false, "is_restricted": false,
                "name": "ASUS laptop", "type": "computer", "volume_percent": 64, "supports_volume": true })
    }

    #[tokio::test]
    async fn premium_with_active_device_probes_by_resetting_repeat() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([device(true)])).await;
        Mock::given(method("GET"))
            .and(path("/me/player"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "device": device(true), "repeat_state": "context", "is_playing": true })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/me/player/repeat"))
            .and(query_param("state", "context"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let cache = PremiumCache::default();
        let caps = capabilities(&client, &cache).await.unwrap();
        assert_eq!(caps.premium, Some(true));
        assert_eq!(caps.devices.len(), 1);
        assert_eq!(caps.devices[0].kind, "computer");
        assert_eq!(caps.devices[0].volume_percent, Some(64));
        // Cached for an hour: no second probe (expect(1) above).
        assert_eq!(capabilities(&client, &cache).await.unwrap().premium, Some(true));
    }

    #[tokio::test]
    async fn free_account_is_detected_from_the_403_body() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([device(true)])).await;
        Mock::given(method("GET"))
            .and(path("/me/player"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "device": device(true), "repeat_state": "off" })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/me/player/repeat"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "error": { "status": 403, "message": "Player command failed: Premium required", "reason": "PREMIUM_REQUIRED" }
            })))
            .mount(&server)
            .await;
        let caps = capabilities(&client, &PremiumCache::default()).await.unwrap();
        assert_eq!(caps.premium, Some(false));
    }

    #[tokio::test]
    async fn no_spotify_app_open_means_unknown_and_no_devices() {
        let (_d, _pool, server, client, _id) = setup().await;
        mount_devices(&server, json!([])).await;
        Mock::given(method("GET")).and(path("/me/player")).respond_with(ResponseTemplate::new(204)).mount(&server).await;
        Mock::given(method("PUT")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&server).await;
        let cache = PremiumCache::default();
        let caps = capabilities(&client, &cache).await.unwrap();
        assert_eq!(caps, SpotifyCapabilities { premium: None, devices: vec![] });
        assert_eq!(cache.get(), None, "unknown is not cached");
    }

    #[tokio::test]
    async fn unexplained_403_stays_unknown() {
        let (_d, _pool, server, client, _id) = setup().await;
        Mock::given(method("GET"))
            .and(path("/me/player"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "device": device(true), "repeat_state": "track" })))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "error": { "status": 403, "message": "Restriction violated" } })))
            .mount(&server)
            .await;
        assert_eq!(probe_premium(&client).await.unwrap(), None);
    }
}
