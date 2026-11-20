//! Runs a script extension's index.js in a fresh, sandboxed QuickJS context per call.
//!
//! The context gets only the API in `prelude.js`, backed by a few host functions: HTTP (https
//! only, every hop checked against the extension's `permissions.network`, bodies capped, a
//! per-extension cookie jar), logging, hashes and randomness. There is no file system,
//! process or module access. Each call has a memory limit and a deadline (the engine is
//! interrupted when it passes). Storage, credentials and cookies are handed in and the changed
//! copies handed back, so the caller persists them (credentials go to the OS keychain).
//!
//! Blocking: call this from `spawn_blocking`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use md5::Md5;
use reqwest::Client;
use reqwest::header::{HeaderMap, COOKIE, LOCATION, SET_COOKIE};
use reqwest::{Method, StatusCode};
use rquickjs::{CatchResultExt, Context, Function, Object, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use url::Url;

use crate::core::extensions::manifest::check_url;
use crate::error::{AppError, AppResult, ErrorCode};

const PRELUDE: &str = include_str!("prelude.js");
const MEMORY_LIMIT: usize = 128 * 1024 * 1024;
const STACK_LIMIT: usize = 512 * 1024;
/// Largest HTTP body handed to an extension.
const MAX_BODY: usize = 10 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Some services take over 10 s to accept a connection from far away.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Time to run index.js once when listing its functions.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// What persists between calls of one extension.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptState {
    #[serde(default)]
    pub storage: Map<String, Value>,
    /// Kept in the OS keychain by the caller, never in SQLite.
    #[serde(default)]
    pub credentials: Map<String, Value>,
    /// `domain → { name: value }`.
    #[serde(default)]
    pub cookies: Map<String, Value>,
}

/// One call's inputs.
pub struct Env {
    pub ext_id: String,
    pub hosts: Vec<String>,
    /// Passed to `initialize(config)`.
    pub settings: Map<String, Value>,
    pub state: ScriptState,
    pub timeout: Duration,
    /// Set when the caller no longer wants the answer (a newer search started): HTTP calls,
    /// sleeps and the engine stop soon after.
    pub cancel: Arc<AtomicBool>,
}

/// A call's answer plus the state to persist (also after a failure).
pub struct Outcome {
    /// `Ok(None)`: the extension registers none of the requested functions.
    pub result: AppResult<Option<Value>>,
    pub state: ScriptState,
}

fn failed(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionFailed, why)
}

fn engine(e: rquickjs::Error) -> AppError {
    AppError::internal(format!("script engine: {e}"))
}

// ---- Bytes as "binary strings" (one char per byte) ----

fn from_bin(s: &str) -> Vec<u8> {
    s.chars().map(|c| (u32::from(c) & 0xff) as u8).collect()
}

fn to_bin(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

fn digest(algo: &str, data: &[u8]) -> Vec<u8> {
    match algo {
        "md5" => Md5::digest(data).to_vec(),
        "sha1" => Sha1::digest(data).to_vec(),
        _ => Sha256::digest(data).to_vec(),
    }
}

fn hmac(algo: &str, key: &[u8], msg: &[u8]) -> Vec<u8> {
    fn run<M: Mac + hmac::digest::KeyInit>(key: &[u8], msg: &[u8]) -> Vec<u8> {
        match <M as hmac::digest::KeyInit>::new_from_slice(key) {
            Ok(mut mac) => {
                mac.update(msg);
                mac.finalize().into_bytes().to_vec()
            }
            Err(_) => Vec::new(),
        }
    }
    match algo {
        "sha1" => run::<Hmac<Sha1>>(key, msg),
        _ => run::<Hmac<Sha256>>(key, msg),
    }
}

fn random(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n.min(1024)];
    if getrandom::fill(&mut buf).is_err() {
        buf.clear();
    }
    buf
}

// ---- Cookies ----

/// A small cookie jar: name/value per domain (path, expiry and flags are not tracked, except
/// that `Max-Age=0` removes a cookie).
struct Jar(Map<String, Value>);

impl Jar {
    fn header_for(&self, url: &Url) -> Option<String> {
        let host = url.host_str()?.to_ascii_lowercase();
        let pairs: Vec<String> = self
            .0
            .iter()
            .filter(|(domain, _)| host == **domain || host.ends_with(&format!(".{domain}")))
            .filter_map(|(_, v)| v.as_object())
            .flat_map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|v| format!("{k}={v}"))))
            .collect();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    fn store(&mut self, url: &Url, headers: &HeaderMap) {
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else { return };
        for raw in headers.get_all(SET_COOKIE).iter().filter_map(|v| v.to_str().ok()) {
            let mut parts = raw.split(';').map(str::trim);
            let Some((name, value)) = parts.next().and_then(|p| p.split_once('=')) else { continue };
            let mut domain = host.clone();
            let mut expired = false;
            for attr in parts {
                let (k, v) = attr.split_once('=').unwrap_or((attr, ""));
                match k.to_ascii_lowercase().as_str() {
                    "domain" => {
                        let d = v.trim().trim_start_matches('.').to_ascii_lowercase();
                        // Only the host itself or a parent domain of it.
                        if !d.is_empty() && (host == d || host.ends_with(&format!(".{d}"))) {
                            domain = d;
                        }
                    }
                    "max-age" => expired = v.trim().parse::<i64>().is_ok_and(|n| n <= 0),
                    _ => {}
                }
            }
            let entry = self.0.entry(domain).or_insert_with(|| Value::Object(Map::new()));
            if let Some(map) = entry.as_object_mut() {
                if expired {
                    map.remove(name.trim());
                } else {
                    map.insert(name.trim().to_owned(), Value::String(value.trim().to_owned()));
                }
            }
        }
    }
}

// ---- HTTP ----

#[derive(Deserialize)]
struct HttpRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Map<String, Value>,
    body: Option<String>,
}

/// Where the HTTP futures run: the app's tokio runtime when there is one (the caller is a
/// `spawn_blocking` thread), otherwise a small runtime of our own.
enum Driver {
    Shared(tokio::runtime::Handle),
    Own(tokio::runtime::Runtime),
}

impl Driver {
    fn new() -> AppResult<Driver> {
        match tokio::runtime::Handle::try_current() {
            Ok(h) => Ok(Driver::Shared(h)),
            Err(_) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map(Driver::Own)
                .map_err(|e| AppError::internal(format!("extension http runtime: {e}"))),
        }
    }

    fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        match self {
            Driver::Shared(h) => h.block_on(f),
            Driver::Own(rt) => rt.block_on(f),
        }
    }
}

struct HttpHost {
    client: Option<Client>,
    driver: Option<Driver>,
    hosts: Vec<String>,
    jar: RefCell<Jar>,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
}

const CANCELLED: &str = "cancelled: a newer request replaced this one";

/// A reqwest error with its causes (`error sending request: … : connection timed out`).
fn describe(e: &reqwest::Error) -> String {
    let mut out = e.to_string();
    let mut source = std::error::Error::source(e);
    while let Some(cause) = source {
        out.push_str(": ");
        out.push_str(&cause.to_string());
        source = cause.source();
    }
    out.chars().take(400).collect()
}

fn thrown(why: impl Into<String>) -> String {
    json!({ "throw": why.into() }).to_string()
}

fn net_failure(why: impl Into<String>) -> String {
    let why = why.into();
    json!({ "statusCode": 0, "ok": false, "body": "", "headers": {}, "url": "", "error": why }).to_string()
}

impl HttpHost {
    fn handle(&self, raw: &str) -> String {
        let Ok(req) = serde_json::from_str::<HttpRequest>(raw) else { return thrown("bad http request") };
        let Some(client) = &self.client else { return net_failure("network is off while installing") };
        let Ok(mut url) = Url::parse(&req.url) else { return thrown(format!("invalid URL: {}", req.url)) };
        let Ok(mut method) = Method::from_bytes(req.method.as_bytes()) else { return thrown("invalid HTTP method") };
        let mut body = req.body;
        for _ in 0..=MAX_REDIRECTS {
            if self.cancel.load(Ordering::Relaxed) {
                return thrown(CANCELLED);
            }
            let left = self.deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return thrown("timeout: extension took too long to respond");
            }
            if let Err(e) = check_url(&url, &self.hosts) {
                let host = url.host_str().unwrap_or_default();
                return thrown(format!("Permission denied for domain {host} ({})", e.message));
            }
            // Never longer than what's left of the call.
            let mut rb = client.request(method.clone(), url.clone()).timeout(left.min(HTTP_TIMEOUT));
            let mut sent_cookie = false;
            for (k, v) in &req.headers {
                let value = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                sent_cookie |= k.eq_ignore_ascii_case("cookie");
                if body.is_none() && k.eq_ignore_ascii_case("content-type") {
                    continue;
                }
                // Unison asks for gzip/brotli itself and unpacks the answer; a hand-set
                // Accept-Encoding would hand the extension compressed bytes.
                if k.eq_ignore_ascii_case("accept-encoding") {
                    continue;
                }
                rb = rb.header(k.as_str(), value);
            }
            if !sent_cookie {
                if let Some(c) = self.jar.borrow().header_for(&url) {
                    rb = rb.header(COOKIE, c);
                }
            }
            if let Some(b) = &body {
                rb = rb.body(b.clone());
            }
            let resp = match self.wait(rb.send()) {
                Some(Ok(r)) => r,
                Some(Err(e)) => return net_failure(describe(&e)),
                None => return thrown(CANCELLED),
            };
            self.jar.borrow_mut().store(&url, resp.headers());
            let status = resp.status();
            if status.is_redirection() {
                if let Some(loc) = resp.headers().get(LOCATION).and_then(|v| v.to_str().ok()) {
                    let Ok(next) = url.join(loc) else { return net_failure("bad redirect") };
                    if status == StatusCode::SEE_OTHER
                        || (matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND) && method == Method::POST)
                    {
                        method = Method::GET;
                        body = None;
                    }
                    url = next;
                    continue;
                }
            }
            return self.answer(url, resp);
        }
        thrown("too many redirects")
    }

    /// Runs a request future, giving up (`None`) as soon as the call is cancelled.
    fn wait<T>(&self, fut: impl std::future::Future<Output = T>) -> Option<T> {
        let driver = self.driver.as_ref()?;
        let cancel = self.cancel.clone();
        driver.block_on(async move {
            let cancelled = async {
                while !cancel.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            };
            tokio::select! {
                out = fut => Some(out),
                () = cancelled => None,
            }
        })
    }

    fn answer(&self, url: Url, mut resp: reqwest::Response) -> String {
        let status = resp.status();
        let mut headers = Map::new();
        for (name, value) in resp.headers() {
            let v = Value::String(value.to_str().unwrap_or_default().to_owned());
            match headers.get_mut(name.as_str()) {
                Some(Value::Array(list)) => list.push(v),
                Some(existing) => *existing = Value::Array(vec![existing.take(), v]),
                None => {
                    headers.insert(name.as_str().to_owned(), v);
                }
            }
        }
        let mut buf = Vec::new();
        loop {
            match self.wait(resp.chunk()) {
                None => return thrown(CANCELLED),
                Some(Ok(None)) => break,
                Some(Ok(Some(bytes))) => buf.extend_from_slice(&bytes),
                Some(Err(e)) => {
                    let why = if Instant::now() >= self.deadline {
                        "timeout: the answer was still downloading when the extension ran out of time".to_owned()
                    } else {
                        format!("error reading the answer: {}", describe(&e))
                    };
                    return net_failure(why);
                }
            }
            if buf.len() > MAX_BODY {
                return net_failure("response is larger than 10 MB");
            }
        }
        json!({
            "statusCode": status.as_u16(),
            "ok": status.is_success(),
            "statusText": status.canonical_reason().unwrap_or_default(),
            "body": String::from_utf8_lossy(&buf),
            "headers": headers,
            "url": url.as_str(),
        })
        .to_string()
    }
}

// ---- Sessions ----

struct Session {
    rt: Runtime,
    ctx: Context,
    http: Rc<HttpHost>,
    deadline: Instant,
}

fn open(code: &str, env: &Env, network: bool) -> AppResult<Session> {
    let deadline = Instant::now() + env.timeout;
    let rt = Runtime::new().map_err(engine)?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    let stop = env.cancel.clone();
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline || stop.load(Ordering::Relaxed))));
    let ctx = Context::full(&rt).map_err(engine)?;
    let client = if network {
        Some(
            Client::builder()
                .user_agent(concat!("Unison/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(HTTP_TIMEOUT.min(env.timeout))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| AppError::internal(format!("extension http client: {e}")))?,
        )
    } else {
        None
    };
    let driver = if client.is_some() { Some(Driver::new()?) } else { None };
    let http = Rc::new(HttpHost {
        client,
        driver,
        hosts: env.hosts.clone(),
        jar: RefCell::new(Jar(env.state.cookies.clone())),
        deadline,
        cancel: env.cancel.clone(),
    });
    let init = json!({
        "settings": env.settings,
        "storage": env.state.storage,
        "credentials": env.state.credentials,
    })
    .to_string();
    let ext_id = env.ext_id.clone();
    ctx.with(|ctx| -> AppResult<()> {
        let host = Object::new(ctx.clone()).map_err(engine)?;
        let h = http.clone();
        host.set("http", Function::new(ctx.clone(), move |req: String| h.handle(&req)).map_err(engine)?)
            .map_err(engine)?;
        let h = http.clone();
        host.set(
            "clearCookies",
            Function::new(ctx.clone(), move || {
                h.jar.borrow_mut().0.clear();
            })
            .map_err(engine)?,
        )
        .map_err(engine)?;
        host.set(
            "log",
            Function::new(ctx.clone(), move |level: String, msg: String| {
                let msg: String = msg.chars().take(2000).collect();
                match level.as_str() {
                    "error" => tracing::warn!(extension = %ext_id, "{msg}"),
                    "warn" => tracing::info!(extension = %ext_id, "{msg}"),
                    _ => tracing::debug!(extension = %ext_id, "{msg}"),
                }
            })
            .map_err(engine)?,
        )
        .map_err(engine)?;
        host.set(
            "digest",
            Function::new(ctx.clone(), |algo: String, data: String| to_bin(&digest(&algo, &from_bin(&data))))
                .map_err(engine)?,
        )
        .map_err(engine)?;
        host.set(
            "hmac",
            Function::new(ctx.clone(), |algo: String, key: String, msg: String| {
                to_bin(&hmac(&algo, &from_bin(&key), &from_bin(&msg)))
            })
            .map_err(engine)?,
        )
        .map_err(engine)?;
        let stop = env.cancel.clone();
        host.set(
            "sleep",
            Function::new(ctx.clone(), move |ms: f64| {
                let wanted = if ms.is_finite() && ms > 0.0 { Duration::from_millis(ms as u64) } else { Duration::ZERO };
                let until = (Instant::now() + wanted).min(deadline);
                // Short steps so a cancel ends the wait.
                while !stop.load(Ordering::Relaxed) {
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        break;
                    }
                    std::thread::sleep(left.min(Duration::from_millis(100)));
                }
            })
            .map_err(engine)?,
        )
        .map_err(engine)?;
        let stop = env.cancel.clone();
        host.set(
            "cancelled",
            Function::new(ctx.clone(), move || stop.load(Ordering::Relaxed)).map_err(engine)?,
        )
        .map_err(engine)?;
        host.set(
            "random",
            Function::new(ctx.clone(), |n: f64| to_bin(&random(if n.is_finite() && n > 0.0 { n as usize } else { 0 })))
                .map_err(engine)?,
        )
        .map_err(engine)?;
        let globals = ctx.globals();
        globals.set("__host", host).map_err(engine)?;
        globals.set("__unisonInit", init).map_err(engine)?;
        ctx.eval::<(), _>(PRELUDE).catch(&ctx).map_err(|e| AppError::internal(format!("prelude: {e}")))?;
        ctx.eval::<(), _>(code).catch(&ctx).map_err(|e| failed(format!("index.js: {e}")))?;
        Ok(())
    })?;
    Ok(Session { rt, ctx, http, deadline })
}

impl Session {
    fn eval_string(&self, src: &str) -> AppResult<String> {
        self.ctx.with(|ctx| ctx.eval::<String, _>(src).catch(&ctx).map_err(|e| failed(e.to_string())))
    }

    /// Calls the first of `names` the extension registers (after `initialize(settings)`).
    fn call(&self, names: &[&str], args: &[Value]) -> AppResult<Option<Value>> {
        let names = serde_json::to_string(names).map_err(|e| AppError::internal(e.to_string()))?;
        let args = Value::Array(args.to_vec()).to_string();
        self.ctx.with(|ctx| -> AppResult<()> {
            let globals = ctx.globals();
            globals.set("__unisonNames", names).map_err(engine)?;
            globals.set("__unisonArgs", args).map_err(engine)?;
            ctx.eval::<(), _>("__unisonCall(JSON.parse(__unisonNames), JSON.parse(__unisonArgs))")
                .catch(&ctx)
                .map_err(|e| failed(e.to_string()))
        })?;
        // Promises (async functions, fetch().then, timers) settle here.
        while self.rt.is_job_pending() && Instant::now() <= self.deadline && !self.http.cancel.load(Ordering::Relaxed) {
            // An exception inside a job is recorded by the promise handlers in the prelude.
            let _ = self.rt.execute_pending_job();
        }
        let out: Value = serde_json::from_str(&self.eval_string("__unisonTake()")?)
            .map_err(|e| AppError::internal(format!("script result: {e}")))?;
        match out.get("state").and_then(Value::as_str) {
            Some("ok") => Ok(Some(out.get("value").cloned().unwrap_or(Value::Null))),
            Some("missing") => Ok(None),
            Some("error") => {
                let why = out.get("error").and_then(Value::as_str).unwrap_or("extension error");
                let why: String = why.chars().take(500).collect();
                if why.starts_with("timeout") || why.contains("interrupted") {
                    Err(AppError::new(ErrorCode::Offline, why))
                } else {
                    Err(failed(why))
                }
            }
            _ => Err(AppError::new(ErrorCode::Offline, "timeout: extension took too long to respond")),
        }
    }

    fn state(&self) -> Option<ScriptState> {
        let raw = self.eval_string("__unisonState()").ok()?;
        let mut state: ScriptState = serde_json::from_str(&raw).ok()?;
        state.cookies = self.http.jar.borrow().0.clone();
        Some(state)
    }
}

/// Runs index.js once without network and lists the functions it registers.
pub fn registered_functions(code: &str) -> AppResult<Vec<String>> {
    let env = Env {
        ext_id: "install-check".into(),
        hosts: Vec::new(),
        settings: Map::new(),
        state: ScriptState::default(),
        timeout: PROBE_TIMEOUT,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let session = open(code, &env, false).map_err(|e| AppError::new(ErrorCode::ExtensionInvalid, e.message))?;
    let names: Option<Vec<String>> = serde_json::from_str(&session.eval_string("__unisonFunctions()")?)
        .map_err(|e| AppError::internal(format!("script functions: {e}")))?;
    names.ok_or_else(|| AppError::new(ErrorCode::ExtensionInvalid, "extension did not call registerExtension()"))
}

/// Calls the first of `names` the extension registers with `args`.
pub fn call(code: &str, env: Env, names: &[&str], args: &[Value]) -> Outcome {
    let session = match open(code, &env, true) {
        Ok(s) => s,
        Err(e) => return Outcome { result: Err(e), state: env.state },
    };
    let result = session.call(names, args);
    let state = session.state().unwrap_or(env.state);
    Outcome { result, state }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn env(hosts: &[&str]) -> Env {
        Env {
            ext_id: "test".into(),
            hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            settings: serde_json::from_value(json!({ "region": "US" })).unwrap(),
            state: ScriptState::default(),
            timeout: Duration::from_secs(10),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Runs a call on a blocking thread, as the app does.
    async fn run(code: &str, env: Env, names: &[&'static str], args: Vec<Value>) -> Outcome {
        let code = code.to_owned();
        let names = names.to_vec();
        tokio::task::spawn_blocking(move || call(&code, env, &names, &args)).await.unwrap()
    }

    #[test]
    fn lists_registered_functions_without_network() {
        let code = r#"
            const ext = { initialize(c) { return true; }, searchTracks(q, n) { return []; }, name: "x" };
            class Extra { getDownloadUrl() {} }
            registerExtension(Object.assign(new Extra(), ext));
        "#;
        assert_eq!(registered_functions(code).unwrap(), vec!["initialize", "searchTracks", "getDownloadUrl"]);
        assert_eq!(registered_functions("var a = 1;").unwrap_err().code, ErrorCode::ExtensionInvalid);
        assert_eq!(registered_functions("this is not js").unwrap_err().code, ErrorCode::ExtensionInvalid);
        let net = "http.get('https://example.com'); registerExtension({})";
        assert!(registered_functions(net).is_ok(), "network calls while listing fail softly");
    }

    #[tokio::test]
    async fn calls_functions_with_settings_storage_and_credentials() {
        let code = r#"
            let settings = {};
            registerExtension({
              initialize(config) { settings = config; return true; },
              echo(a, b) {
                storage.set("count", (storage.get("count") || 0) + 1);
                credentials.store("token", "secret-" + a);
                return { a, b, region: settings.region, count: storage.get("count"), has: credentials.has("token") };
              },
              async later(x) { await null; return x * 2; },
              boom() { throw new Error("nope"); },
            });
        "#;
        let first = run(code, env(&[]), &["echo"], vec![json!(1), json!("two")]).await;
        assert_eq!(first.result.unwrap(), Some(json!({ "a": 1, "b": "two", "region": "US", "count": 1, "has": true })));
        assert_eq!(first.state.credentials.get("token"), Some(&json!("secret-1")));

        let mut again = env(&[]);
        again.state = first.state;
        let second = run(code, again, &["echo"], vec![json!(2), json!(null)]).await;
        assert_eq!(second.result.unwrap().unwrap()["count"], json!(2), "storage persists between calls");

        assert_eq!(run(code, env(&[]), &["later"], vec![json!(21)]).await.result.unwrap(), Some(json!(42)));
        assert_eq!(run(code, env(&[]), &["missing", "later"], vec![json!(1)]).await.result.unwrap(), Some(json!(2)));
        assert_eq!(run(code, env(&[]), &["missing"], vec![]).await.result.unwrap(), None);
        let err = run(code, env(&[]), &["boom"], vec![]).await.result.unwrap_err();
        assert_eq!((err.code, err.message.as_str()), (ErrorCode::ExtensionFailed, "nope"));
    }

    #[tokio::test]
    async fn stops_runaway_scripts() {
        let mut e = env(&[]);
        e.timeout = Duration::from_millis(300);
        let started = Instant::now();
        let out = run("registerExtension({ spin() { for (;;) {} } })", e, &["spin"], vec![]).await;
        assert!(out.result.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));

        let hog = "registerExtension({ hog() { const a = []; for (;;) a.push(new Array(1e6).fill(1)); } })";
        assert!(run(hog, env(&[]), &["hog"], vec![]).await.result.is_err(), "memory limit");
    }

    #[tokio::test]
    async fn polyfills_behave() {
        let code = r#"
            registerExtension({ check() {
              const u = new URL("../b/c?x=1#h", "https://Example.com:443/a/z/y");
              u.searchParams.set("q", "a b");
              const p = new URLSearchParams({ k: "v&w" });
              p.append("k", "2");
              return {
                href: u.href, host: u.host, path: u.pathname, q: u.searchParams.get("x"),
                form: p.toString(), all: p.getAll("k"),
                b64: btoa("Hello, World!"), un: atob("SGVsbG8sIFdvcmxkIQ=="),
                utf: utils.base64Decode(utils.base64Encode("héllo")),
                enc: Array.from(new TextEncoder().encode("é")),
                dec: new TextDecoder().decode(new Uint8Array([104, 195, 169])),
                md5: utils.md5("abc"), sha: utils.sha256("abc"),
                hmac: utils.hmacSHA256("message", "key"),
                hmac1: utils.hmacSHA1([107, 101, 121], [109]).length,
                uuid: /^[0-9a-f-]{36}$/.test(crypto.randomUUID()),
                sim: matching.compareStrings("Hello", "hello!"),
                file: file.download("x").success,
                time: typeof gobackend.getLocalTime().timestamp,
                ua: /^Mozilla\/5\.0 /.test(utils.randomUserAgent()),
                slept: (() => { const t = Date.now(); utils.sleep(50); return Date.now() - t >= 40; })(),
                cancelled: utils.isRequestCancelled(),
              };
            } });
        "#;
        let v = run(code, env(&[]), &["check"], vec![]).await.result.unwrap().unwrap();
        assert_eq!(v["href"], "https://example.com/a/b/c?x=1&q=a+b#h");
        assert_eq!(v["host"], "example.com");
        assert_eq!(v["path"], "/a/b/c");
        assert_eq!(v["q"], "1");
        assert_eq!(v["form"], "k=v%26w&k=2");
        assert_eq!(v["all"], json!(["v&w", "2"]));
        assert_eq!(v["b64"], "SGVsbG8sIFdvcmxkIQ==");
        assert_eq!(v["un"], "Hello, World!");
        assert_eq!(v["utf"], "héllo");
        assert_eq!(v["enc"], json!([195, 169]));
        assert_eq!(v["dec"], "hé");
        assert_eq!(v["md5"], "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(v["sha"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(v["hmac"], "6e9ef29b75fffc5b7abae527d58fdadb2fe42e7219011976917343065f58ed4a");
        assert_eq!(v["hmac1"], 20);
        assert_eq!(v["uuid"], true);
        assert_eq!(v["sim"], 1.0);
        assert_eq!(v["file"], false);
        assert_eq!(v["time"], "number");
        assert_eq!((v["ua"].clone(), v["slept"].clone(), v["cancelled"].clone()), (json!(true), json!(true), json!(false)));
    }

    #[tokio::test]
    async fn http_is_allowlisted_and_keeps_cookies() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/login"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("Location", "/home").insert_header("Set-Cookie", "sid=abc; Path=/"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/home"))
            .and(header("cookie", "sid=abc"))
            .respond_with(ResponseTemplate::new(200).set_body_string("welcome"))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api"))
            .and(query_param("v", "1"))
            .and(header("content-type", "application/json"))
            .and(body_string(r#"{"a":1}"#))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/escape"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "https://evil.example/x"))
            .mount(&server)
            .await;

        let base = server.uri();
        let code = r#"
            registerExtension({
              flow(base) {
                const home = http.get(base + "/login");
                const api = http.post(base + "/api?v=1", { a: 1 });
                const f = fetch(base + "/api?v=1", { method: "POST", body: JSON.stringify({ a: 1 }) });
                let denied = "";
                try { http.get("https://evil.example/"); } catch (e) { denied = e.message; }
                let redirect = "";
                try { http.get(base + "/escape"); } catch (e) { redirect = e.message; }
                return { home: home.body, status: home.statusCode, ok: home.ok, api: JSON.parse(api.body),
                         fetched: f.json(), fetchOk: f.ok, denied, redirect };
              },
              async viaThen(base) { const r = await fetch(base + "/home"); return r.text(); },
            });
        "#;
        let out = run(code, env(&["127.0.0.1"]), &["flow"], vec![json!(base)]).await;
        let v = out.result.unwrap().unwrap();
        assert_eq!(v["home"], "welcome");
        assert_eq!((v["status"].clone(), v["ok"].clone()), (json!(200), json!(true)));
        assert_eq!(v["api"], json!({ "ok": true }));
        assert_eq!((v["fetched"].clone(), v["fetchOk"].clone()), (json!({ "ok": true }), json!(true)));
        assert!(v["denied"].as_str().unwrap().starts_with("Permission denied for domain evil.example"), "{v}");
        assert!(v["redirect"].as_str().unwrap().starts_with("Permission denied for domain evil.example"), "{v}");
        assert_eq!(out.state.cookies, serde_json::from_value::<Map<String, Value>>(json!({ "127.0.0.1": { "sid": "abc" } })).unwrap());

        let mut with_jar = env(&["127.0.0.1"]);
        with_jar.state = out.state;
        let r = run(code, with_jar, &["viaThen"], vec![json!(base)]).await;
        assert_eq!(r.result.unwrap(), Some(json!("welcome")), "saved cookies are sent again");
    }

    #[tokio::test]
    async fn asks_for_compressed_answers_and_unpacks_them() {
        use std::io::Write;
        let server = MockServer::start().await;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(br#"{"items":["a","b"]}"#).unwrap();
        Mock::given(method("GET"))
            .and(path("/big"))
            .and(wiremock::matchers::header_regex("accept-encoding", "gzip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-encoding", "gzip")
                    .set_body_raw(gz.finish().unwrap(), "application/json"),
            )
            .mount(&server)
            .await;
        // The extension's own Accept-Encoding is replaced, so the body still arrives unpacked.
        let code = r#"registerExtension({ go(base) {
            return JSON.parse(http.get(base + "/big", { "Accept-Encoding": "identity" }).body);
        } })"#;
        let out = run(code, env(&["127.0.0.1"]), &["go"], vec![json!(server.uri())]).await;
        assert_eq!(out.result.unwrap(), Some(json!({ "items": ["a", "b"] })));
    }

    #[tokio::test]
    async fn a_cancelled_call_stops_waiting_and_downloading() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;
        let code = r#"registerExtension({
            nap() { utils.sleep(5000); return utils.isRequestCancelled(); },
            slow(base) { return http.get(base + "/slow").statusCode; },
        })"#;
        for (name, args) in [("nap", vec![]), ("slow", vec![json!(server.uri())])] {
            let e = env(&["127.0.0.1"]);
            let cancel = e.cancel.clone();
            let started = Instant::now();
            let stopper = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                cancel.store(true, Ordering::Relaxed);
            });
            let out = run(code, e, &[name], args).await;
            stopper.await.unwrap();
            assert!(started.elapsed() < Duration::from_secs(3), "{name} stopped early");
            match name {
                "nap" => assert!(out.result.is_err() || out.result.unwrap() == Some(json!(true))),
                _ => assert!(out.result.is_err(), "the slow request was abandoned"),
            }
        }
    }
}
