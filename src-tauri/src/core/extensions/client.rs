//! HTTP for extensions: redirects are followed by hand so every hop is checked against the
//! extension's host allowlist, and bodies are capped.

use std::time::Duration;

use reqwest::header::{ACCEPT_ENCODING, CONTENT_RANGE, CONTENT_TYPE, LOCATION, RANGE};
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use url::Url;

use super::manifest::{check_url, host_not_allowed};
use crate::error::{AppError, AppResult, ErrorCode};

/// Largest JSON answer read from an extension's service.
pub const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
/// Largest artwork image fetched into the cache.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;
/// A whole-file download may take this long.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub struct ExtHttp {
    client: Client,
}

pub fn network_error(err: &reqwest::Error) -> AppError {
    if err.is_timeout() || err.is_connect() {
        AppError::new(ErrorCode::Offline, format!("extension request: {err}"))
    } else {
        AppError::new(ErrorCode::ExtensionFailed, format!("extension request: {err}"))
    }
}

impl ExtHttp {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .user_agent(concat!("Unison/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::internal(format!("extension http client: {e}")))?;
        Ok(Self { client })
    }

    /// Sends a GET, following up to five redirects, each checked against `hosts`.
    /// Returns the final URL and response (2xx only).
    async fn get(&self, url: Url, hosts: &[String], range_probe: bool) -> AppResult<(Url, Response)> {
        self.get_checked(url, &|u| check_url(u, hosts), range_probe, &[], None).await
    }

    /// `headers` go only to the first URL's host; a redirect elsewhere drops them.
    async fn get_checked(
        &self,
        mut url: Url,
        check: &(dyn Fn(&Url) -> AppResult<()> + Sync),
        range_probe: bool,
        headers: &[(String, String)],
        timeout: Option<Duration>,
    ) -> AppResult<(Url, Response)> {
        let origin = url.host_str().map(str::to_ascii_lowercase);
        for _ in 0..=MAX_REDIRECTS {
            check(&url)?;
            let mut req = self.client.get(url.clone());
            if let Some(t) = timeout {
                req = req.timeout(t);
            }
            if range_probe {
                req = req.header(RANGE, "bytes=0-0").header(ACCEPT_ENCODING, "identity");
            }
            if url.host_str().map(str::to_ascii_lowercase) == origin {
                for (name, value) in headers {
                    req = req.header(name.as_str(), value.as_str());
                }
            }
            let resp = req.send().await.map_err(|e| network_error(&e))?;
            let status = resp.status();
            if status.is_redirection() {
                let location = resp
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "redirect without Location"))?;
                url = url
                    .join(location)
                    .map_err(|e| AppError::new(ErrorCode::ExtensionFailed, format!("bad redirect: {e}")))?;
                continue;
            }
            if status == StatusCode::NOT_FOUND {
                return Err(AppError::not_found(format!("{} returned 404", url.host_str().unwrap_or_default())));
            }
            if !status.is_success() {
                return Err(AppError::new(ErrorCode::Upstream { status: status.as_u16() }, "extension service error"));
            }
            return Ok((url, resp));
        }
        Err(host_not_allowed("too many redirects"))
    }

    async fn body(mut resp: Response, cap: usize) -> AppResult<Vec<u8>> {
        if resp.content_length().is_some_and(|n| n > cap as u64) {
            return Err(AppError::new(ErrorCode::ExtensionFailed, "response is too large"));
        }
        let mut out = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| network_error(&e))? {
            if out.len() + chunk.len() > cap {
                return Err(AppError::new(ErrorCode::ExtensionFailed, "response is too large"));
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    /// GETs a body of at most `cap` bytes.
    pub async fn bytes(&self, url: Url, hosts: &[String], cap: usize) -> AppResult<Vec<u8>> {
        let (_, resp) = self.get(url, hosts, false).await?;
        Self::body(resp, cap).await
    }

    /// Downloads a package the user chose (any https host, redirects included; GitHub release
    /// assets redirect to a CDN). Loopback http is allowed in debug builds for test fixtures.
    pub async fn package(&self, url: Url, cap: usize) -> AppResult<Vec<u8>> {
        let check = |u: &Url| {
            let loopback = matches!(u.host_str(), Some("127.0.0.1" | "localhost"));
            match u.scheme() {
                "https" => Ok(()),
                "http" if cfg!(debug_assertions) && loopback => Ok(()),
                other => Err(host_not_allowed(format!("{other} is not https"))),
            }
        };
        let (_, resp) = self.get_checked(url, &check, false, &[], None).await?;
        Self::body(resp, cap).await
    }

    /// GETs JSON (at most [`MAX_JSON_BYTES`]).
    pub async fn json(&self, url: Url, hosts: &[String]) -> AppResult<Value> {
        let bytes = self.bytes(url, hosts, MAX_JSON_BYTES).await?;
        serde_json::from_slice(&bytes).map_err(|e| AppError::new(ErrorCode::ExtensionFailed, format!("not JSON: {e}")))
    }

    /// Follows redirects of a stream URL (one-byte Range probe) and returns where the audio is,
    /// so the player only ever opens an allowlisted URL.
    pub async fn final_stream_url(&self, url: Url, hosts: &[String]) -> AppResult<Url> {
        Ok(self.probe_stream(url, hosts, &[]).await?.url)
    }

    /// The one-byte probe of a stream: final URL, total length and content type. `headers`
    /// are sent to the stream's own host only.
    pub async fn probe_stream(&self, url: Url, hosts: &[String], headers: &[(String, String)]) -> AppResult<StreamProbe> {
        let (url, resp) = self.get_checked(url, &|u| check_url(u, hosts), true, headers, None).await?;
        let len = resp
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.parse::<u64>().ok())
            .or_else(|| (resp.status() == StatusCode::OK).then(|| resp.content_length()).flatten());
        let content_type = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);
        Ok(StreamProbe { url, len, content_type })
    }
}

impl ExtHttp {
    /// Saves a whole audio file into `out` (EXT-07): every hop checked against `hosts`,
    /// `headers` only to the first host, at most `cap` bytes, `progress(received, total)` after
    /// each chunk. Returns the content type.
    pub async fn download(
        &self,
        url: Url,
        hosts: &[String],
        headers: &[(String, String)],
        out: &mut (dyn std::io::Write + Send),
        cap: u64,
        progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> AppResult<Option<String>> {
        let mut headers = headers.to_vec();
        headers.push((ACCEPT_ENCODING.as_str().to_owned(), "identity".to_owned()));
        let (_, mut resp) =
            self.get_checked(url, &|u| check_url(u, hosts), false, &headers, Some(DOWNLOAD_TIMEOUT)).await?;
        let total = resp.content_length();
        if total.is_some_and(|n| n > cap) {
            return Err(AppError::new(ErrorCode::ExtensionFailed, "the file is too large"));
        }
        let content_type = resp.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);
        let mut received = 0u64;
        while let Some(chunk) = resp.chunk().await.map_err(|e| network_error(&e))? {
            received += chunk.len() as u64;
            if received > cap {
                return Err(AppError::new(ErrorCode::ExtensionFailed, "the file is too large"));
            }
            out.write_all(&chunk)?;
            progress(received, total);
        }
        if total.is_some_and(|n| n != received) {
            return Err(AppError::new(ErrorCode::Offline, "the download stopped early"));
        }
        Ok(content_type)
    }
}

/// What the probe of a stream learned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamProbe {
    pub url: Url,
    /// Total length; `None` when the server didn't say (the player probes again).
    pub len: Option<u64>,
    pub content_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn hosts() -> Vec<String> {
        vec!["127.0.0.1".to_owned()]
    }

    #[tokio::test]
    async fn follows_allowed_redirects_and_blocks_others() {
        let server = MockServer::start().await;
        let base = server.uri();
        Mock::given(method("GET"))
            .and(path("/hop"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/final"))
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/final")).respond_with(ResponseTemplate::new(206)).mount(&server).await;
        Mock::given(method("GET"))
            .and(path("/escape"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "https://evil.example/a.mp3"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/loop"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/loop"))
            .mount(&server)
            .await;

        let http = ExtHttp::new().unwrap();
        let url = |p: &str| Url::parse(&format!("{base}{p}")).unwrap();
        let fin = http.final_stream_url(url("/hop"), &hosts()).await.unwrap();
        assert_eq!(fin.path(), "/final");
        let err = http.final_stream_url(url("/escape"), &hosts()).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ExtensionHostNotAllowed);
        let err = http.final_stream_url(url("/loop"), &hosts()).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ExtensionHostNotAllowed);
        let err = http.json(url("/hop"), &["other.example".to_owned()]).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ExtensionHostNotAllowed);
    }

    #[tokio::test]
    async fn caps_body_size() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 2048]))
            .mount(&server)
            .await;
        let http = ExtHttp::new().unwrap();
        let url = Url::parse(&format!("{}/big", server.uri())).unwrap();
        let err = http.bytes(url.clone(), &hosts(), 1024).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ExtensionFailed);
        assert_eq!(http.bytes(url, &hosts(), 4096).await.unwrap().len(), 2048);
    }
}
