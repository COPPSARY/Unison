//! Spotify provider (spec §6). Bring-your-own Client ID (ADR 0001), PKCE, Development Mode
//! endpoints only. Sync, refresh/backoff and playback arrive in T-12 / T-13 / T-40.

pub mod connect;

use serde::Deserialize;

use crate::core::auth::oauth::{network_error, OAuthConfig};
use crate::error::{AppError, AppResult, ErrorCode};

pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
pub const API_BASE: &str = "https://api.spotify.com/v1";
pub const DASHBOARD_URL: &str = "https://developer.spotify.com/dashboard";

/// Pre-registered loopback ports (the user adds these redirect URIs in their dev app).
pub const REDIRECT_PORTS: [u16; 3] = [43821, 43822, 43823];

/// Scopes from the Plan ("Spotify specifics").
pub const SCOPES: [&str; 12] = [
    "user-library-read",
    "user-library-modify",
    "playlist-read-private",
    "playlist-read-collaborative",
    "playlist-modify-private",
    "playlist-modify-public",
    "user-top-read",
    "user-read-recently-played",
    "user-follow-read",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
];

/// Redirect URIs to show in onboarding, in port order.
pub fn redirect_uris() -> Vec<String> {
    REDIRECT_PORTS
        .iter()
        .map(|p| format!("http://127.0.0.1:{p}/callback"))
        .collect()
}

/// Spotify Client IDs are 32 hex characters. Returns the normalised (lowercase, trimmed) ID.
pub fn validate_client_id(raw: &str) -> AppResult<String> {
    let id = raw.trim().to_ascii_lowercase();
    if id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(id)
    } else {
        Err(AppError::new(ErrorCode::InvalidClientId, "client id must be 32 hex characters"))
    }
}

pub fn oauth_config(client_id: &str) -> OAuthConfig {
    OAuthConfig {
        authorize_url: AUTHORIZE_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        client_id: client_id.to_owned(),
        client_secret: None,
        scopes: SCOPES.to_vec(),
        extra_params: vec![],
    }
}

/// The parts of `GET /me` Unison uses (email/country are no longer returned, spec §6).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub display_name: Option<String>,
}

/// `GET /me`. A 403 right after login means the account isn't in the app's User Management.
pub async fn fetch_profile(http: &reqwest::Client, api_base: &str, access_token: &str) -> AppResult<Profile> {
    let response = http
        .get(format!("{api_base}/me"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(network_error)?;
    match response.status().as_u16() {
        200 => response
            .json::<Profile>()
            .await
            .map_err(|e| AppError::internal(format!("spotify /me: {e}"))),
        401 => Err(AppError::new(ErrorCode::NeedsReconnect, "spotify rejected the token")),
        403 => Err(AppError::new(
            ErrorCode::UserNotRegistered,
            "spotify /me returned 403 (user not in the app's User Management)",
        )),
        429 => {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(5);
            Err(AppError::new(ErrorCode::RateLimited { retry_after }, "spotify rate limit"))
        }
        status => Err(AppError::new(ErrorCode::Upstream { status }, "spotify /me failed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn client_id_validation() {
        let id = validate_client_id("  0123456789ABCDEF0123456789abcdef \n").unwrap();
        assert_eq!(id, "0123456789abcdef0123456789abcdef");
        for bad in ["", "123", "0123456789abcdef0123456789abcdeg", "0123456789abcdef0123456789abcdef0"] {
            assert_eq!(validate_client_id(bad).unwrap_err().code, ErrorCode::InvalidClientId, "{bad}");
        }
    }

    #[test]
    fn redirect_uris_match_onboarding() {
        assert_eq!(
            redirect_uris(),
            vec![
                "http://127.0.0.1:43821/callback",
                "http://127.0.0.1:43822/callback",
                "http://127.0.0.1:43823/callback"
            ]
        );
    }

    #[tokio::test]
    async fn profile_from_fixture() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../../../tests/fixtures/spotify/me.json");
        Mock::given(method("GET"))
            .and(path("/me"))
            .and(header("authorization", "Bearer AT"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(fixture, "application/json"))
            .mount(&server)
            .await;
        let p = fetch_profile(&reqwest::Client::new(), &server.uri(), "AT").await.unwrap();
        assert_eq!(p.id, "demo-user");
        assert_eq!(p.display_name.as_deref(), Some("Demo Listener"));
    }

    #[tokio::test]
    async fn profile_403_means_user_not_registered() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Check settings on developer.spotify.com/dashboard, the user may not be registered."))
            .mount(&server)
            .await;
        let err = fetch_profile(&reqwest::Client::new(), &server.uri(), "AT").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::UserNotRegistered);
    }

    #[tokio::test]
    async fn profile_429_carries_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "12"))
            .mount(&server)
            .await;
        let err = fetch_profile(&reqwest::Client::new(), &server.uri(), "AT").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited { retry_after: 12 });
    }
}
