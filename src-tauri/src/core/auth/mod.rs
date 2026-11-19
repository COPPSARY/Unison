//! Authentication building blocks shared by every provider (spec §9): PKCE, the loopback
//! listener, the OAuth client and keychain storage.

pub mod keychain;
pub mod loopback;
pub mod oauth;
pub mod pkce;

use std::time::Duration;

use tokio::sync::oneshot;

use crate::error::AppResult;
use keychain::StoredTokens;
use loopback::Loopback;
use oauth::OAuthConfig;
use pkce::Pkce;

/// Runs a full interactive login: bind loopback → open the browser → wait for the callback →
/// exchange the code. `open_browser` receives the authorize URL (tauri-plugin-opener in the app).
pub async fn run_pkce_flow(
    http: &reqwest::Client,
    config: &OAuthConfig,
    ports: &[u16],
    timeout: Duration,
    cancel: oneshot::Receiver<()>,
    open_browser: impl FnOnce(&str) -> AppResult<()>,
) -> AppResult<StoredTokens> {
    let pkce = Pkce::generate()?;
    let loopback = Loopback::bind(ports).await?;
    let redirect_uri = loopback.redirect_uri();
    let url = config.authorize_url(&pkce, &redirect_uri)?;
    tracing::info!(port = loopback.port, "waiting for OAuth callback");
    open_browser(&url)?;
    let code = loopback.wait_for_code(&pkce.state, timeout, cancel).await?;
    config.exchange_code(http, &code, &pkce.verifier, &redirect_uri).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// End to end with a fake browser: it reads the authorize URL and calls back.
    #[tokio::test]
    async fn full_flow_with_simulated_browser() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "AT", "expires_in": 3600, "refresh_token": "RT"
            })))
            .mount(&server)
            .await;
        let config = OAuthConfig {
            authorize_url: "https://accounts.example.com/authorize".into(),
            token_url: format!("{}/token", server.uri()),
            client_id: "cid".into(),
            client_secret: None,
            scopes: vec!["x"],
            extra_params: vec![],
        };
        let (_tx, rx) = oneshot::channel();
        let tokens = run_pkce_flow(&reqwest::Client::new(), &config, &[], Duration::from_secs(5), rx, |url| {
            let parsed = url::Url::parse(url).unwrap();
            let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
            let redirect = url::Url::parse(&q["redirect_uri"]).unwrap();
            let port = redirect.port().unwrap();
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
        })
        .await
        .unwrap();
        assert_eq!(tokens.access_token, "AT");
    }
}
