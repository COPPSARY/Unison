//! One-shot OAuth loopback listener (spec §9): binds 127.0.0.1 only, accepts the single
//! `/callback` request, checks `state`, shows a "you can close this tab" page, and stops.
//! Times out after 5 minutes; can be cancelled.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use super::pkce::state_matches;
use crate::error::{AppError, AppResult, ErrorCode};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// A bound listener and the redirect URI that points at it.
pub struct Loopback {
    listener: TcpListener,
    pub port: u16,
}

impl Loopback {
    /// Binds the first free port from `ports` (Spotify needs pre-registered ports), or an
    /// OS-assigned port when `ports` is empty (Google accepts any loopback port).
    pub async fn bind(ports: &[u16]) -> AppResult<Self> {
        if ports.is_empty() {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
            let port = listener.local_addr()?.port();
            return Ok(Self { listener, port });
        }
        for &port in ports {
            if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
                return Ok(Self { listener, port });
            }
        }
        Err(AppError::internal(format!("no free loopback port in {ports:?}")))
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    /// Waits for the provider to redirect back. Returns the authorization `code`.
    pub async fn wait_for_code(
        self,
        expected_state: &str,
        timeout: Duration,
        cancel: oneshot::Receiver<()>,
    ) -> AppResult<String> {
        let accept_loop = async {
            loop {
                let (stream, _) = self.listener.accept().await?;
                match handle(stream, expected_state).await {
                    Handled::Done(result) => return result,
                    Handled::Ignored => continue,
                }
            }
        };
        tokio::select! {
            result = accept_loop => result,
            _ = tokio::time::sleep(timeout) => {
                Err(AppError::new(ErrorCode::AuthTimeout, "no callback within the time limit"))
            }
            _ = cancel => Err(AppError::new(ErrorCode::AuthCancelled, "cancelled in app")),
        }
    }
}

enum Handled {
    Done(AppResult<String>),
    Ignored,
}

async fn handle(mut stream: TcpStream, expected_state: &str) -> Handled {
    let Some(target) = read_request_target(&mut stream).await else {
        return Handled::Ignored;
    };
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
        let _ = respond(&mut stream, 400, PAGE_ERROR).await;
        return Handled::Ignored;
    };
    if url.path() != "/callback" {
        // Browsers also ask for /favicon.ico etc.
        let _ = respond(&mut stream, 404, "").await;
        return Handled::Ignored;
    }

    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };

    let result = match (param("state"), param("error"), param("code")) {
        (Some(state), _, _) if !state_matches(expected_state, &state) => Err(AppError::internal(
            "OAuth state mismatch; ignoring callback",
        )),
        (_, Some(error), _) => {
            let code = if error == "access_denied" {
                ErrorCode::AuthCancelled
            } else {
                ErrorCode::Upstream { status: 400 }
            };
            Err(AppError::new(code, format!("authorization error: {error}")))
        }
        (Some(_), None, Some(code)) if !code.is_empty() => Ok(code),
        _ => Err(AppError::internal("callback without code or state")),
    };

    // A forged request with a wrong state must not end the real login.
    if matches!(&result, Err(e) if e.code == ErrorCode::Internal) {
        let _ = respond(&mut stream, 400, PAGE_ERROR).await;
        return Handled::Ignored;
    }
    let page = if result.is_ok() { PAGE_OK } else { PAGE_ERROR };
    let _ = respond(&mut stream, 200, page).await;
    Handled::Done(result)
}

/// Reads the request line and returns the target (`/callback?...`) for GET requests.
async fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > MAX_REQUEST_BYTES {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let mut parts = head.lines().next()?.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) if target.starts_with('/') => Some(target.to_owned()),
        _ => None,
    }
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Bad Request",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

const PAGE_OK: &str = concat!(
    "<!doctype html><html lang=en><meta charset=utf-8><title>Unison</title><style>",
    "body{margin:0;height:100vh;display:grid;place-items:center;background:#121216;color:#ececf3;font:15px/1.5 system-ui,'Segoe UI',sans-serif}main{text-align:center}h1{font-size:22px;font-weight:600;margin:0 0 6px}p{margin:0;color:#9a98ad}",
    "</style><main><h1>You're connected</h1><p>You can close this tab and return to Unison.</p></main></html>"
);

const PAGE_ERROR: &str = concat!(
    "<!doctype html><html lang=en><meta charset=utf-8><title>Unison</title><style>",
    "body{margin:0;height:100vh;display:grid;place-items:center;background:#121216;color:#ececf3;font:15px/1.5 system-ui,'Segoe UI',sans-serif}main{text-align:center}h1{font-size:22px;font-weight:600;margin:0 0 6px}p{margin:0;color:#9a98ad}",
    "</style><main><h1>Sign-in didn't finish</h1><p>Return to Unison to see what happened and try again.</p></main></html>"
);

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    async fn start(state: &'static str, timeout: Duration) -> (u16, tokio::task::JoinHandle<AppResult<String>>, oneshot::Sender<()>) {
        let lb = Loopback::bind(&[]).await.unwrap();
        let port = lb.port;
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move { lb.wait_for_code(state, timeout, rx).await });
        (port, task, tx)
    }

    #[tokio::test]
    async fn returns_code_and_shows_close_page() {
        let (port, task, _tx) = start("s1", DEFAULT_TIMEOUT).await;
        let favicon = get(port, "/favicon.ico").await;
        assert!(favicon.starts_with("HTTP/1.1 404"));
        let page = get(port, "/callback?code=abc&state=s1").await;
        assert!(page.contains("You can close this tab"));
        assert_eq!(task.await.unwrap().unwrap(), "abc");
    }

    #[tokio::test]
    async fn ignores_wrong_state_and_keeps_waiting() {
        let (port, task, _tx) = start("good", DEFAULT_TIMEOUT).await;
        let forged = get(port, "/callback?code=evil&state=bad").await;
        assert!(forged.starts_with("HTTP/1.1 400"));
        get(port, "/callback?code=real&state=good").await;
        assert_eq!(task.await.unwrap().unwrap(), "real");
    }

    #[tokio::test]
    async fn access_denied_is_auth_cancelled() {
        let (port, task, _tx) = start("s", DEFAULT_TIMEOUT).await;
        let page = get(port, "/callback?error=access_denied&state=s").await;
        assert!(page.contains("didn't finish"));
        assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::AuthCancelled);
    }

    #[tokio::test]
    async fn times_out() {
        let (_port, task, _tx) = start("s", Duration::from_millis(50)).await;
        assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::AuthTimeout);
    }

    #[tokio::test]
    async fn can_be_cancelled() {
        let (_port, task, tx) = start("s", DEFAULT_TIMEOUT).await;
        tx.send(()).unwrap();
        assert_eq!(task.await.unwrap().unwrap_err().code, ErrorCode::AuthCancelled);
    }

    #[tokio::test]
    async fn binds_first_free_preferred_port() {
        let first = Loopback::bind(&[]).await.unwrap();
        let taken = first.port;
        let lb = Loopback::bind(&[taken, 0]).await;
        // `taken` is busy, so it must not be chosen.
        if let Ok(lb) = lb {
            assert_ne!(lb.port, taken);
        }
        assert!(first.redirect_uri().ends_with("/callback"));
    }
}
