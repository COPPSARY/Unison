//! Spotify Web API client (spec §6 rules): token refresh (pre-emptive + once on 401),
//! `429 Retry-After`, 5xx backoff, max 4 concurrent requests, and `next`-link paging.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::{Mutex, Semaphore};

use super::{oauth_config, API_BASE, TOKEN_URL};
use crate::core::auth::keychain::{get_json, keys, set_json, SecretStore, StoredTokens};
use crate::core::auth::oauth::{network_error, now_unix};
use crate::core::db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{AccountStatus, ProviderId};

/// Refresh this many seconds before the access token expires.
const REFRESH_MARGIN_S: i64 = 60;
const MAX_CONCURRENT: usize = 4;
const MAX_429_RETRIES: u32 = 3;
/// Longer waits are surfaced as `RateLimited` instead of blocking a sync.
const MAX_429_WAIT_S: u64 = 60;

/// Retry timing; tests shrink it.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Waits after consecutive 5xx responses; one retry per entry.
    pub backoff: Vec<Duration>,
    /// Multiplier for `Retry-After` seconds.
    pub retry_after_unit: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            backoff: [500, 1000, 2000, 4000].map(Duration::from_millis).to_vec(),
            retry_after_unit: Duration::from_secs(1),
        }
    }
}

/// Spotify paging object (`/me/tracks`, `/me/playlists`, …). Cursor-paged endpoints
/// (`/me/following`) wrap it in a key; see [`SpotifyClient::collect_pages`].
#[derive(Debug, Clone, Deserialize)]
pub struct Paging<T> {
    pub items: Vec<T>,
    pub next: Option<String>,
    pub total: Option<u32>,
}

pub struct SpotifyClient {
    http: reqwest::Client,
    secrets: Arc<dyn SecretStore>,
    db: SqlitePool,
    api_base: String,
    token_url: String,
    policy: RetryPolicy,
    limit: Semaphore,
    /// Cached tokens; the mutex also makes refresh single-flight.
    tokens: Mutex<Option<StoredTokens>>,
}

impl SpotifyClient {
    pub fn new(http: reqwest::Client, secrets: Arc<dyn SecretStore>, db: SqlitePool) -> Self {
        Self::with_endpoints(http, secrets, db, API_BASE, TOKEN_URL, RetryPolicy::default())
    }

    pub fn with_endpoints(
        http: reqwest::Client,
        secrets: Arc<dyn SecretStore>,
        db: SqlitePool,
        api_base: &str,
        token_url: &str,
        policy: RetryPolicy,
    ) -> Self {
        Self {
            http,
            secrets,
            db,
            api_base: api_base.trim_end_matches('/').to_owned(),
            token_url: token_url.to_owned(),
            policy,
            limit: Semaphore::new(MAX_CONCURRENT),
            tokens: Mutex::new(None),
        }
    }

    /// Full URL for an API path (`/me/tracks`) or passes absolute `next` links through.
    fn url(&self, path_or_url: &str) -> String {
        if path_or_url.starts_with("http") {
            path_or_url.to_owned()
        } else {
            format!("{}{}", self.api_base, path_or_url)
        }
    }

    /// A valid access token, refreshing first when it is (about to be) expired or `force`.
    async fn access_token(&self, force_refresh: bool) -> AppResult<String> {
        let mut guard = self.tokens.lock().await;
        if guard.is_none() {
            *guard = get_json::<StoredTokens>(self.secrets.as_ref(), keys::SPOTIFY_TOKENS)?;
        }
        let Some(current) = guard.clone() else {
            return Err(AppError::new(ErrorCode::NeedsReconnect, "spotify is not connected"));
        };
        if !force_refresh && current.expires_at - REFRESH_MARGIN_S > now_unix() {
            return Ok(current.access_token);
        }
        let refreshed = self.refresh(&current).await?;
        let token = refreshed.access_token.clone();
        *guard = Some(refreshed);
        Ok(token)
    }

    async fn refresh(&self, current: &StoredTokens) -> AppResult<StoredTokens> {
        let Some(refresh_token) = current.refresh_token.as_deref() else {
            return self.mark_needs_reconnect("no refresh token").await;
        };
        let client_id = self
            .secrets
            .get(keys::SPOTIFY_CLIENT_ID)?
            .ok_or_else(|| AppError::new(ErrorCode::InvalidClientId, "no Spotify Client ID saved"))?;
        let mut config = oauth_config(&client_id);
        config.token_url = self.token_url.clone();
        match config.refresh(&self.http, refresh_token).await {
            Ok(tokens) => {
                set_json(self.secrets.as_ref(), keys::SPOTIFY_TOKENS, &tokens)?;
                tracing::debug!("spotify token refreshed");
                Ok(tokens)
            }
            Err(err) if err.code == ErrorCode::NeedsReconnect => {
                self.mark_needs_reconnect("refresh rejected (invalid_grant)").await
            }
            Err(err) => Err(err),
        }
    }

    async fn mark_needs_reconnect<T>(&self, why: &str) -> AppResult<T> {
        tracing::warn!(reason = why, "spotify needs reconnect");
        db::accounts::set_status(&self.db, ProviderId::Spotify, AccountStatus::NeedsReconnect).await?;
        Err(AppError::new(ErrorCode::NeedsReconnect, why))
    }

    /// Sends a request with auth, refresh-on-401, 429 and 5xx handling. Returns the response
    /// for any 2xx status.
    pub async fn send(
        &self,
        method: Method,
        path_or_url: &str,
        body: Option<&serde_json::Value>,
    ) -> AppResult<reqwest::Response> {
        let url = self.url(path_or_url);
        let _permit = self
            .limit
            .acquire()
            .await
            .map_err(|_| AppError::internal("spotify client closed"))?;

        let mut refreshed_on_401 = false;
        let mut retries_429 = 0u32;
        let mut retries_5xx = 0usize;
        loop {
            let token = self.access_token(false).await?;
            let mut request = self.http.request(method.clone(), &url).bearer_auth(&token);
            if let Some(json) = body {
                request = request.json(json);
            }
            let response = request.send().await.map_err(network_error)?;
            let status = response.status();

            if status.is_success() {
                return Ok(response);
            }
            match status {
                StatusCode::UNAUTHORIZED if !refreshed_on_401 => {
                    refreshed_on_401 = true;
                    self.access_token(true).await?;
                }
                StatusCode::UNAUTHORIZED => {
                    return self.mark_needs_reconnect("401 after refresh").await;
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    let wait_s = retry_after_secs(&response);
                    if retries_429 >= MAX_429_RETRIES || wait_s > MAX_429_WAIT_S {
                        let retry_after = u32::try_from(wait_s).unwrap_or(u32::MAX);
                        return Err(AppError::new(
                            ErrorCode::RateLimited { retry_after },
                            "spotify rate limit",
                        ));
                    }
                    retries_429 += 1;
                    tracing::debug!(wait_s, "spotify 429, waiting");
                    tokio::time::sleep(self.policy.retry_after_unit * u32::try_from(wait_s).unwrap_or(1)).await;
                }
                s if s.is_server_error() => {
                    let Some(wait) = self.policy.backoff.get(retries_5xx) else {
                        return Err(AppError::new(
                            ErrorCode::Upstream { status: s.as_u16() },
                            "spotify server error",
                        ));
                    };
                    retries_5xx += 1;
                    tokio::time::sleep(*wait).await;
                }
                StatusCode::NOT_FOUND => {
                    return Err(AppError::not_found("spotify item not found"));
                }
                s => {
                    return Err(AppError::new(
                        ErrorCode::Upstream { status: s.as_u16() },
                        format!("spotify {} failed", method),
                    ));
                }
            }
        }
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path_or_url: &str) -> AppResult<T> {
        self.send(Method::GET, path_or_url, None)
            .await?
            .json::<T>()
            .await
            .map_err(|e| AppError::internal(format!("spotify response: {e}")))
    }

    /// Follows `next` links from `first`, calling `on_page` for each page, up to `max_pages`.
    /// `extract` pulls the paging object out of the response (identity for most endpoints,
    /// `|r| r.artists` for cursor-paged `/me/following`). Stop early by returning `false`.
    pub async fn collect_pages<R, T>(
        &self,
        first: &str,
        max_pages: usize,
        extract: impl Fn(R) -> Paging<T>,
        mut on_page: impl FnMut(Vec<T>, Option<u32>) -> bool,
    ) -> AppResult<()>
    where
        R: DeserializeOwned,
    {
        let mut next = Some(first.to_owned());
        let mut pages = 0usize;
        while let Some(url) = next.take() {
            if pages >= max_pages {
                break;
            }
            let page = extract(self.get_json::<R>(&url).await?);
            pages += 1;
            if !on_page(page.items, page.total) {
                break;
            }
            next = page.next;
        }
        Ok(())
    }
}

fn retry_after_secs(response: &reqwest::Response) -> u64 {
    response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::auth::keychain::MemoryStore;
    use serde_json::{json, Value};
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const CLIENT_ID: &str = "0123456789abcdef0123456789abcdef";

    struct Harness {
        _dir: tempfile::TempDir,
        server: MockServer,
        secrets: Arc<MemoryStore>,
        db: SqlitePool,
        client: SpotifyClient,
    }

    async fn harness(expires_in: i64) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let db = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        db::accounts::upsert_connected(&db, ProviderId::Spotify, "u", Some("U"), 0).await.unwrap();
        let server = MockServer::start().await;
        let secrets = Arc::new(MemoryStore::default());
        secrets.set(keys::SPOTIFY_CLIENT_ID, CLIENT_ID).unwrap();
        set_json(
            secrets.as_ref(),
            keys::SPOTIFY_TOKENS,
            &StoredTokens {
                access_token: "OLD".into(),
                refresh_token: Some("RT".into()),
                expires_at: now_unix() + expires_in,
                scope: None,
            },
        )
        .unwrap();
        let policy = RetryPolicy {
            backoff: vec![Duration::from_millis(1); 4],
            retry_after_unit: Duration::from_millis(1),
        };
        let client = SpotifyClient::with_endpoints(
            reqwest::Client::new(),
            secrets.clone(),
            db.clone(),
            &server.uri(),
            &format!("{}/api/token", server.uri()),
            policy,
        );
        Harness { _dir: dir, server, secrets, db, client }
    }

    fn token_ok() -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "NEW", "token_type": "Bearer", "expires_in": 3600
        }))
    }

    #[tokio::test]
    async fn refreshes_once_on_401_and_persists_tokens() {
        let h = harness(3600).await;
        Mock::given(method("GET")).and(path("/me")).and(header("authorization", "Bearer OLD"))
            .respond_with(ResponseTemplate::new(401)).expect(1).mount(&h.server).await;
        Mock::given(method("POST")).and(path("/api/token")).respond_with(token_ok()).expect(1).mount(&h.server).await;
        Mock::given(method("GET")).and(path("/me")).and(header("authorization", "Bearer NEW"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "u"}))).expect(1).mount(&h.server).await;

        let me: Value = h.client.get_json("/me").await.unwrap();
        assert_eq!(me["id"], "u");
        let stored: StoredTokens = get_json(h.secrets.as_ref(), keys::SPOTIFY_TOKENS).unwrap().unwrap();
        assert_eq!(stored.access_token, "NEW");
        assert_eq!(stored.refresh_token.as_deref(), Some("RT"), "un-rotated refresh token kept");
    }

    #[tokio::test]
    async fn refreshes_before_expiry() {
        let h = harness(10).await; // inside the 60 s margin
        Mock::given(method("POST")).and(path("/api/token")).respond_with(token_ok()).expect(1).mount(&h.server).await;
        Mock::given(method("GET")).and(header("authorization", "Bearer NEW"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({}))).expect(2).mount(&h.server).await;
        let _: Value = h.client.get_json("/me").await.unwrap();
        let _: Value = h.client.get_json("/me").await.unwrap(); // cached, no second refresh
    }

    #[tokio::test]
    async fn invalid_grant_marks_needs_reconnect() {
        let h = harness(0).await;
        Mock::given(method("POST")).and(path("/api/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})))
            .mount(&h.server).await;
        let err = h.client.get_json::<Value>("/me").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NeedsReconnect);
        let accounts = db::accounts::list(&h.db).await.unwrap();
        assert!(accounts.iter().any(|a| a.provider == ProviderId::Spotify && a.status == AccountStatus::NeedsReconnect));
    }

    #[tokio::test]
    async fn waits_on_429_then_succeeds() {
        let h = harness(3600).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "2"))
            .up_to_n_times(2).mount(&h.server).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&h.server).await;
        let v: Value = h.client.get_json("/me").await.unwrap();
        assert_eq!(v["ok"], true);
    }

    #[tokio::test]
    async fn long_or_repeated_429_becomes_rate_limited() {
        let h = harness(3600).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "120"))
            .mount(&h.server).await;
        let err = h.client.get_json::<Value>("/me").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::RateLimited { retry_after: 120 });
        assert!(err.retryable);
    }

    #[tokio::test]
    async fn backs_off_on_5xx_then_gives_up() {
        let h = harness(3600).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(503)).up_to_n_times(2).mount(&h.server).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_json(json!({}))).up_to_n_times(1).mount(&h.server).await;
        let _: Value = h.client.get_json("/me").await.unwrap();

        let h = harness(3600).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(502)).expect(5).mount(&h.server).await;
        let err = h.client.get_json::<Value>("/me").await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Upstream { status: 502 });
    }

    #[tokio::test]
    async fn not_found_maps_to_not_found() {
        let h = harness(3600).await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).mount(&h.server).await;
        assert_eq!(h.client.get_json::<Value>("/x").await.unwrap_err().code, ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn follows_next_links_across_pages() {
        let h = harness(3600).await;
        let base = h.server.uri();
        let page = |n: u32, next: Option<String>| {
            json!({ "items": [{"n": n}], "next": next, "total": 3, "limit": 1, "offset": n })
        };
        Mock::given(method("GET")).and(path("/me/tracks")).and(query_param("offset", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(1, Some(format!("{base}/me/tracks?offset=2&limit=1")))))
            .mount(&h.server).await;
        Mock::given(method("GET")).and(path("/me/tracks")).and(query_param("offset", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(2, None)))
            .mount(&h.server).await;
        Mock::given(method("GET")).and(path("/me/tracks")).and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(0, Some(format!("{base}/me/tracks?offset=1&limit=1")))))
            .mount(&h.server).await;

        let mut seen = Vec::new();
        h.client
            .collect_pages::<Paging<Value>, Value>("/me/tracks?limit=50", 10, |p| p, |items, total| {
                assert_eq!(total, Some(3));
                seen.extend(items.into_iter().map(|v| v["n"].as_u64().unwrap()));
                true
            })
            .await
            .unwrap();
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[tokio::test]
    async fn cursor_paging_with_extract_and_early_stop() {
        #[derive(Deserialize)]
        struct Following {
            artists: Paging<Value>,
        }
        let h = harness(3600).await;
        let base = h.server.uri();
        Mock::given(method("GET")).and(path("/me/following"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "artists": { "items": [{"id": "a"}], "next": format!("{base}/me/following?type=artist&after=a"), "total": 9,
                             "cursors": {"after": "a"}, "limit": 1 }
            })))
            .expect(1)
            .mount(&h.server).await;
        let mut count = 0;
        h.client
            .collect_pages::<Following, Value>("/me/following?type=artist&limit=50", 10, |f| f.artists, |items, _| {
                count += items.len();
                false // stop after the first page
            })
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
