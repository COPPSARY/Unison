//! Spotify connect flow: keychain Client ID → PKCE loopback → token exchange → `/me` →
//! tokens to keychain, account row to SQLite.

use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::oneshot;

use super::{fetch_profile, oauth_config, validate_client_id, API_BASE, REDIRECT_PORTS};
use crate::core::auth::keychain::{keys, set_json, SecretStore};
use crate::core::auth::loopback::DEFAULT_TIMEOUT;
use crate::core::auth::oauth::now_unix;
use crate::core::auth::run_pkce_flow;
use crate::core::db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{Account, AccountStatus, ProviderId};

/// Everything the flow needs from the app, injectable for tests.
pub struct ConnectContext<'a> {
    pub http: &'a reqwest::Client,
    pub secrets: &'a dyn SecretStore,
    pub db: &'a SqlitePool,
    pub api_base: &'a str,
    pub token_url: Option<&'a str>,
    pub timeout: Duration,
}

impl<'a> ConnectContext<'a> {
    pub fn new(http: &'a reqwest::Client, secrets: &'a dyn SecretStore, db: &'a SqlitePool) -> Self {
        Self { http, secrets, db, api_base: API_BASE, token_url: None, timeout: DEFAULT_TIMEOUT }
    }
}

/// Validates and stores the user's Client ID (keychain only).
pub fn set_client_id(secrets: &dyn SecretStore, raw: &str) -> AppResult<()> {
    let id = validate_client_id(raw)?;
    secrets.set(keys::SPOTIFY_CLIENT_ID, &id)
}

pub fn has_client_id(secrets: &dyn SecretStore) -> AppResult<bool> {
    Ok(secrets.get(keys::SPOTIFY_CLIENT_ID)?.is_some())
}

/// Runs the interactive login and returns the connected account.
pub async fn connect(
    ctx: &ConnectContext<'_>,
    cancel: oneshot::Receiver<()>,
    open_browser: impl FnOnce(&str) -> AppResult<()>,
) -> AppResult<Account> {
    let client_id = ctx
        .secrets
        .get(keys::SPOTIFY_CLIENT_ID)?
        .ok_or_else(|| AppError::new(ErrorCode::InvalidClientId, "no Spotify Client ID saved"))?;
    let mut config = oauth_config(&client_id);
    if let Some(url) = ctx.token_url {
        config.token_url = url.to_owned();
    }

    let tokens = run_pkce_flow(ctx.http, &config, &REDIRECT_PORTS, ctx.timeout, cancel, open_browser).await?;
    let profile = fetch_profile(ctx.http, ctx.api_base, &tokens.access_token).await?;

    set_json(ctx.secrets, keys::SPOTIFY_TOKENS, &tokens)?;
    db::accounts::upsert_connected(
        ctx.db,
        ProviderId::Spotify,
        &profile.id,
        profile.display_name.as_deref(),
        now_unix(),
    )
    .await?;
    tracing::info!("spotify connected");

    Ok(Account {
        provider: ProviderId::Spotify,
        status: AccountStatus::Connected,
        display_name: profile.display_name.or(Some(profile.id)),
        last_synced_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::auth::keychain::{get_json, MemoryStore, StoredTokens};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fake_browser(url: &str) -> AppResult<()> {
        let parsed = url::Url::parse(url).unwrap();
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "0123456789abcdef0123456789abcdef");
        let port = url::Url::parse(&q["redirect_uri"]).unwrap().port().unwrap();
        assert!(REDIRECT_PORTS.contains(&port));
        let state = q["state"].clone();
        tokio::spawn(async move {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.write_all(format!("GET /callback?code=C&state={state} HTTP/1.1\r\n\r\n").as_bytes())
                .await
                .unwrap();
            let mut sink = Vec::new();
            let _ = s.read_to_end(&mut sink).await;
        });
        Ok(())
    }

    #[tokio::test]
    async fn connect_stores_tokens_and_account() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT", "token_type": "Bearer", "expires_in": 3600, "refresh_token": "RT"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/me"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                include_str!("../../../../tests/fixtures/spotify/me.json"),
                "application/json",
            ))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let secrets = MemoryStore::default();
        let http = reqwest::Client::new();
        set_client_id(&secrets, "0123456789ABCDEF0123456789abcdef").unwrap();

        let token_url = format!("{}/api/token", server.uri());
        let api = server.uri();
        let ctx = ConnectContext {
            api_base: &api,
            token_url: Some(&token_url),
            timeout: Duration::from_secs(5),
            ..ConnectContext::new(&http, &secrets, &pool)
        };
        let (_tx, rx) = oneshot::channel();
        let account = connect(&ctx, rx, fake_browser).await.unwrap();

        assert_eq!(account.display_name.as_deref(), Some("Demo Listener"));
        let stored: StoredTokens = get_json(&secrets, keys::SPOTIFY_TOKENS).unwrap().unwrap();
        assert_eq!(stored.refresh_token.as_deref(), Some("RT"));
        let accounts = db::accounts::list(&pool).await.unwrap();
        assert!(accounts.iter().any(|a| a.provider == ProviderId::Spotify && a.status == AccountStatus::Connected));
        // Tokens must never reach SQLite.
        let dump: Vec<String> = sqlx::query_scalar("SELECT COALESCE(display_name,'') || account_id FROM accounts")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(dump.iter().all(|row| !row.contains("AT") || row.contains("Demo")));
    }

    #[tokio::test]
    async fn connect_without_client_id_is_invalid_client_id() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let secrets = MemoryStore::default();
        let http = reqwest::Client::new();
        let (_tx, rx) = oneshot::channel();
        let err = connect(&ConnectContext::new(&http, &secrets, &pool), rx, |_| Ok(())).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidClientId);
    }

    #[test]
    fn client_id_is_validated_before_storing() {
        let secrets = MemoryStore::default();
        assert_eq!(set_client_id(&secrets, "nope").unwrap_err().code, ErrorCode::InvalidClientId);
        assert!(!has_client_id(&secrets).unwrap());
        set_client_id(&secrets, "0123456789abcdef0123456789abcdef").unwrap();
        assert!(has_client_id(&secrets).unwrap());
    }
}
