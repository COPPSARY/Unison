//! Generic OAuth 2.0 authorization-code + PKCE client (Spotify now, Google in T-20).

use serde::Deserialize;

use super::keychain::StoredTokens;
use super::pkce::Pkce;
use crate::error::{AppError, AppResult, ErrorCode};

/// Static description of a provider's OAuth endpoints.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    /// Desktop Google clients have a non-secret "secret"; Spotify PKCE has none.
    pub client_secret: Option<String>,
    pub scopes: Vec<&'static str>,
    /// Extra authorize parameters (e.g. Google `access_type=offline`).
    pub extra_params: Vec<(&'static str, &'static str)>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

impl OAuthConfig {
    /// URL the user's browser opens.
    pub fn authorize_url(&self, pkce: &Pkce, redirect_uri: &str) -> AppResult<String> {
        let mut url = url::Url::parse(&self.authorize_url)
            .map_err(|e| AppError::internal(format!("authorize url: {e}")))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("client_id", &self.client_id)
                .append_pair("response_type", "code")
                .append_pair("redirect_uri", redirect_uri)
                .append_pair("code_challenge_method", "S256")
                .append_pair("code_challenge", &pkce.challenge)
                .append_pair("state", &pkce.state)
                .append_pair("scope", &self.scopes.join(" "));
            for (k, v) in &self.extra_params {
                q.append_pair(k, v);
            }
        }
        Ok(url.into())
    }

    /// Exchanges the callback `code` for tokens.
    pub async fn exchange_code(
        &self,
        http: &reqwest::Client,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> AppResult<StoredTokens> {
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", self.client_id.as_str()),
            ("code_verifier", verifier),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.as_str()));
        }
        self.token_request(http, &form, None).await
    }

    /// Refreshes an access token. `invalid_grant` (revoked/expired refresh token) becomes
    /// `NeedsReconnect`. Keeps the old refresh token when the provider doesn't rotate it.
    pub async fn refresh(&self, http: &reqwest::Client, refresh_token: &str) -> AppResult<StoredTokens> {
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", self.client_id.as_str()),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret.as_str()));
        }
        self.token_request(http, &form, Some(refresh_token)).await
    }

    async fn token_request(
        &self,
        http: &reqwest::Client,
        form: &[(&str, &str)],
        previous_refresh: Option<&str>,
    ) -> AppResult<StoredTokens> {
        let response = http
            .post(&self.token_url)
            .form(form)
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status();
        if status.is_success() {
            let body: TokenResponse = response
                .json()
                .await
                .map_err(|e| AppError::internal(format!("token response: {e}")))?;
            return Ok(StoredTokens {
                access_token: body.access_token,
                refresh_token: body.refresh_token.or_else(|| previous_refresh.map(str::to_owned)),
                expires_at: now_unix() + body.expires_in.unwrap_or(3600),
                scope: body.scope,
            });
        }
        let error = response
            .json::<TokenErrorResponse>()
            .await
            .map(|b| b.error)
            .unwrap_or_default();
        Err(match error.as_str() {
            "invalid_grant" if previous_refresh.is_some() => {
                AppError::new(ErrorCode::NeedsReconnect, "refresh token rejected (invalid_grant)")
            }
            "invalid_client" => AppError::new(ErrorCode::InvalidClientId, "client rejected"),
            _ => AppError::new(
                ErrorCode::Upstream { status: status.as_u16() },
                format!("token endpoint error: {error}"),
            ),
        })
    }
}

/// Maps reqwest transport failures (no response) to `Offline`; never includes URLs with params.
pub fn network_error(err: reqwest::Error) -> AppError {
    if err.is_connect() || err.is_timeout() {
        AppError::new(ErrorCode::Offline, "network unreachable")
    } else {
        AppError::internal(format!("http: {}", err.without_url()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config(server: &MockServer) -> OAuthConfig {
        OAuthConfig {
            authorize_url: "https://accounts.example.com/authorize".into(),
            token_url: format!("{}/api/token", server.uri()),
            client_id: "cid".into(),
            client_secret: None,
            scopes: vec!["a", "b"],
            extra_params: vec![("prompt", "consent")],
        }
    }

    #[tokio::test]
    async fn authorize_url_has_pkce_params() {
        let server = MockServer::start().await;
        let pkce = Pkce::generate().unwrap();
        let url = config(&server).authorize_url(&pkce, "http://127.0.0.1:43821/callback").unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], pkce.challenge);
        assert_eq!(q["state"], pkce.state);
        assert_eq!(q["scope"], "a b");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:43821/callback");
        assert_eq!(q["prompt"], "consent");
        assert!(!url.contains(&pkce.verifier));
    }

    #[tokio::test]
    async fn exchanges_code_with_verifier() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("code_verifier=VER"))
            .and(body_string_contains("client_id=cid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT", "token_type": "Bearer", "expires_in": 3600,
                "refresh_token": "RT", "scope": "a b"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let t = config(&server)
            .exchange_code(&reqwest::Client::new(), "CODE", "VER", "http://127.0.0.1:1/callback")
            .await
            .unwrap();
        assert_eq!(t.access_token, "AT");
        assert_eq!(t.refresh_token.as_deref(), Some("RT"));
        assert!(t.expires_at > now_unix() + 3500);
    }

    #[tokio::test]
    async fn refresh_keeps_old_refresh_token_when_not_rotated() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT2", "token_type": "Bearer", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        let t = config(&server).refresh(&reqwest::Client::new(), "RT1").await.unwrap();
        assert_eq!(t.access_token, "AT2");
        assert_eq!(t.refresh_token.as_deref(), Some("RT1"));
    }

    #[tokio::test]
    async fn invalid_grant_on_refresh_needs_reconnect() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant", "error_description": "Refresh token revoked"
            })))
            .mount(&server)
            .await;
        let err = config(&server).refresh(&reqwest::Client::new(), "RT").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NeedsReconnect);
    }

    #[tokio::test]
    async fn invalid_client_maps_to_invalid_client_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({ "error": "invalid_client" })))
            .mount(&server)
            .await;
        let err = config(&server)
            .exchange_code(&reqwest::Client::new(), "C", "V", "http://127.0.0.1:1/callback")
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidClientId);
    }

    #[tokio::test]
    async fn unreachable_server_is_offline() {
        let cfg = OAuthConfig { token_url: "http://127.0.0.1:9/token".into(), ..config(&MockServer::start().await) };
        let err = cfg.refresh(&reqwest::Client::new(), "RT").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Offline);
    }
}
