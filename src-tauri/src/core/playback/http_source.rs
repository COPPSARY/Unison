//! Seekable HTTP audio source for the NativeEngine (UP-06, Audius streams).
//!
//! A download thread fills a buffer with HTTP `Range` requests; the decoder reads from that
//! buffer. Seeking past the downloaded part restarts the download at the new offset. A failed
//! connection sets an error flag and makes reads fail, so playback ends cleanly instead of
//! panicking.
//!
//! The buffer is a temporary file in the app's cache folder when one is set
//! ([`set_buffer_dir`]), else memory. The temporary file only lives while the track is loaded:
//! it is deleted when the stream is dropped (track changed, stopped or ended), and leftovers
//! from a crash are deleted at the next launch. It is never added to the library.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// Buffer granularity: a block is readable once fully downloaded (or at end of file).
const BLOCK: u64 = 32 * 1024;
/// Refuse absurdly large streams buffered in memory.
pub const MAX_STREAM_BYTES: u64 = 300 * 1024 * 1024;
/// Largest stream buffered in a temporary file.
pub const MAX_DISK_STREAM_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const BUFFER_SUFFIX: &str = ".unison-buffer";

static BUFFER_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Deletes stream buffers an earlier run left in `dir` (e.g. after a crash).
fn purge(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(BUFFER_SUFFIX) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Streams buffer into temporary files in `dir` from now on, after deleting leftovers from
/// an earlier run. Call once at startup; without it streams buffer in memory.
pub fn set_buffer_dir(dir: PathBuf) {
    purge(&dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "stream buffer folder unavailable; buffering in memory");
        return;
    }
    let _ = BUFFER_DIR.set(dir);
}

/// Where a stream's bytes are kept while it plays.
#[derive(Debug)]
enum Buffer {
    Memory(Vec<u8>),
    /// A temporary file, deleted when dropped.
    Disk { file: Option<File>, path: PathBuf },
}

impl Buffer {
    fn new(len: u64, dir: Option<&Path>) -> Result<Self, StreamError> {
        let Some(dir) = dir else {
            if len > MAX_STREAM_BYTES {
                return Err(StreamError::new(format!("stream length {len} not supported")));
            }
            return Ok(Buffer::Memory(vec![0; usize::try_from(len).unwrap_or(0)]));
        };
        if len > MAX_DISK_STREAM_BYTES {
            return Err(StreamError::new(format!("stream length {len} not supported")));
        }
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = dir.join(format!("{}-{nanos:x}-{n}{BUFFER_SUFFIX}", std::process::id()));
        let opened = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|f| f.set_len(len).map(|()| f));
        match opened {
            Ok(file) => Ok(Buffer::Disk { file: Some(file), path }),
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(StreamError::new(format!("stream buffer: {e}")))
            }
        }
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        match self {
            Buffer::Memory(buf) => {
                let from = usize::try_from(offset).map_err(io::Error::other)?;
                buf[from..from + data.len()].copy_from_slice(data);
                Ok(())
            }
            Buffer::Disk { file: Some(f), .. } => {
                f.seek(SeekFrom::Start(offset))?;
                f.write_all(data)
            }
            Buffer::Disk { file: None, .. } => Err(io::Error::other("stream buffer closed")),
        }
    }

    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> io::Result<()> {
        match self {
            Buffer::Memory(buf) => {
                let from = usize::try_from(offset).map_err(io::Error::other)?;
                out.copy_from_slice(&buf[from..from + out.len()]);
                Ok(())
            }
            Buffer::Disk { file: Some(f), .. } => {
                f.seek(SeekFrom::Start(offset))?;
                f.read_exact(out)
            }
            Buffer::Disk { file: None, .. } => Err(io::Error::other("stream buffer closed")),
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Buffer::Disk { file, path } = self {
            // Close first: Windows refuses to delete an open file.
            drop(file.take());
            if let Err(e) = std::fs::remove_file(&*path) {
                tracing::warn!(error = %e, "stream buffer not deleted");
            }
        }
    }
}
/// How long a read may wait for data before giving up.
const READ_WAIT: Duration = Duration::from_secs(15);

#[derive(Debug)]
struct State {
    buf: Buffer,
    filled: Vec<bool>,
    /// Offset the reader needs that isn't downloaded yet (restart request).
    want: Option<u64>,
    error: Option<String>,
    closed: bool,
}

#[derive(Debug)]
struct Shared {
    len: u64,
    state: Mutex<State>,
    cond: Condvar,
}

/// Status shared with the engine (error flag, progress).
#[derive(Debug, Clone)]
pub struct StreamHandle(Arc<Shared>);

impl StreamHandle {
    pub fn error(&self) -> Option<String> {
        self.0.state.lock().ok().and_then(|s| s.error.clone())
    }

    /// Fraction of blocks downloaded (0..=1).
    pub fn buffered(&self) -> f32 {
        let Ok(s) = self.0.state.lock() else { return 0.0 };
        if s.filled.is_empty() {
            return 1.0;
        }
        s.filled.iter().filter(|f| **f).count() as f32 / s.filled.len() as f32
    }
}

pub struct HttpSource {
    shared: Arc<Shared>,
    pos: u64,
    /// File type hint from the URL / content type (e.g. "mp3").
    pub hint: Option<String>,
}

fn block_of(offset: u64) -> usize {
    usize::try_from(offset / BLOCK).unwrap_or(usize::MAX)
}

fn hint_from(content_type: Option<&str>, url: &str) -> Option<String> {
    let from_type = content_type.and_then(|ct| match ct.split(';').next()?.trim().to_ascii_lowercase().as_str() {
        "audio/mpeg" | "audio/mp3" | "mp3" => Some("mp3"),
        "audio/flac" | "audio/x-flac" | "flac" => Some("flac"),
        "audio/wav" | "audio/x-wav" | "audio/wave" | "wav" => Some("wav"),
        "audio/ogg" | "ogg" | "oga" => Some("ogg"),
        "audio/mp4" | "audio/aac" | "audio/x-m4a" | "m4a" | "mp4" | "aac" => Some("m4a"),
        _ => None,
    });
    from_type.map(str::to_owned).or_else(|| {
        let path = url.split(['?', '#']).next()?;
        let ext = path.rsplit('.').next()?.to_ascii_lowercase();
        (ext.len() <= 4 && path.contains('.')).then_some(ext)
    })
}

/// Where the audio is and how to ask for it.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct StreamTarget {
    pub url: String,
    /// Request headers the service needs (already allowlisted). Sent only to the URL's host.
    pub headers: Vec<(String, String)>,
    /// Type hint from the source: a MIME type ("audio/flac") or a file extension ("flac").
    pub mime: Option<String>,
    /// Total length and content type when a probe already learned them (skips a second probe).
    pub known: Option<(u64, Option<String>)>,
}

impl StreamTarget {
    pub fn url(url: impl Into<String>) -> Self {
        Self { url: url.into(), ..Self::default() }
    }
}

// Header values may hold credentials: never print them.
impl std::fmt::Debug for StreamTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(n, _)| n.as_str()).collect();
        f.debug_struct("StreamTarget")
            .field("host", &url::Url::parse(&self.url).ok().and_then(|u| u.host_str().map(str::to_owned)))
            .field("headers", &names)
            .field("mime", &self.mime)
            .field("known", &self.known)
            .finish()
    }
}

/// Why a stream didn't open. `status` is the HTTP status when the server refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamError {
    pub status: Option<u16>,
    pub message: String,
}

impl StreamError {
    fn new(message: impl Into<String>) -> Self {
        Self { status: None, message: message.into() }
    }
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A GET with the target's headers and no compression (byte offsets must be the file's own).
fn request(
    client: &reqwest::blocking::Client,
    url: &str,
    headers: &[(String, String)],
    range: &str,
) -> reqwest::blocking::RequestBuilder {
    let mut req = client.get(url);
    for (name, value) in headers {
        req = req.header(name.as_str(), value.as_str());
    }
    req.header(reqwest::header::ACCEPT_ENCODING, "identity").header(reqwest::header::RANGE, range)
}

fn same_host(a: &str, b: &str) -> bool {
    let host = |u: &str| url::Url::parse(u).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase));
    host(a).is_some() && host(a) == host(b)
}

/// The probe: final URL (after redirects), total length and content type.
fn probe(client: &reqwest::blocking::Client, target: &StreamTarget) -> Result<(String, u64, Option<String>), StreamError> {
    let probe = request(client, &target.url, &target.headers, "bytes=0-0")
        .send()
        .map_err(|e| StreamError::new(format!("stream request failed: {}", e.without_url())))?;
    let status = probe.status();
    if !status.is_success() {
        return Err(StreamError { status: Some(status.as_u16()), message: format!("stream returned HTTP {}", status.as_u16()) });
    }
    let content_type = probe
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    // 206 → total length is after the slash in Content-Range; 200 → Content-Length.
    let len = probe
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| probe.content_length())
        .ok_or_else(|| StreamError::new("stream has no length (seeking unsupported)"))?;
    // The probe may have been redirected (Audius → content node); keep the final URL.
    Ok((probe.url().to_string(), len, content_type))
}

impl HttpSource {
    /// Opens `url` (following redirects) and starts downloading in the background.
    pub fn open(client: reqwest::blocking::Client, url: &str) -> Result<(Self, StreamHandle), StreamError> {
        Self::open_target(client, &StreamTarget::url(url))
    }

    /// Opens a target; probes it first unless its length is already known. Buffers in the
    /// folder set with [`set_buffer_dir`], else in memory.
    pub fn open_target(client: reqwest::blocking::Client, target: &StreamTarget) -> Result<(Self, StreamHandle), StreamError> {
        Self::open_in(client, target, BUFFER_DIR.get().map(PathBuf::as_path))
    }

    /// Like [`Self::open_target`], buffering in a temporary file in `dir` (`None` = memory).
    pub fn open_in(
        client: reqwest::blocking::Client,
        target: &StreamTarget,
        dir: Option<&Path>,
    ) -> Result<(Self, StreamHandle), StreamError> {
        let (final_url, len, content_type) = match &target.known {
            Some((len, content_type)) => (target.url.clone(), *len, content_type.clone()),
            None => probe(&client, target)?,
        };
        if len == 0 {
            return Err(StreamError::new("stream length 0 not supported"));
        }
        let buf = Buffer::new(len, dir)?;
        // Headers belong to the host the source named, not to a CDN it redirected to.
        let headers = if same_host(&target.url, &final_url) { target.headers.clone() } else { Vec::new() };

        let blocks = block_of(len - 1) + 1;
        let shared = Arc::new(Shared {
            len,
            state: Mutex::new(State {
                buf,
                filled: vec![false; blocks],
                want: None,
                error: None,
                closed: false,
            }),
            cond: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("unison-stream".into())
            .spawn(move || download(client, final_url, headers, worker))
            .map_err(|e| StreamError::new(format!("spawn stream thread: {e}")))?;
        let hint = target
            .mime
            .as_deref()
            .and_then(|m| hint_from(Some(m), ""))
            .or_else(|| hint_from(content_type.as_deref(), &target.url));
        Ok((Self { shared: shared.clone(), pos: 0, hint }, StreamHandle(shared)))
    }

    pub fn len(&self) -> u64 {
        self.shared.len
    }

    pub fn is_empty(&self) -> bool {
        self.shared.len == 0
    }
}

impl Drop for HttpSource {
    fn drop(&mut self) {
        if let Ok(mut s) = self.shared.state.lock() {
            s.closed = true;
        }
        self.shared.cond.notify_all();
    }
}

/// Download loop: sequential ranged reads from `start`, restarting when the reader wants an
/// offset that isn't buffered; idles at the end until asked again or closed.
fn download(client: reqwest::blocking::Client, url: String, headers: Vec<(String, String)>, shared: Arc<Shared>) {
    let mut start = 0u64;
    loop {
        // Find where to (re)start: the requested offset, else the first missing block from `start`.
        {
            let Ok(mut s) = shared.state.lock() else { return };
            loop {
                if s.closed || s.error.is_some() {
                    return;
                }
                if let Some(w) = s.want.take() {
                    start = (w / BLOCK) * BLOCK;
                }
                let first = block_of(start);
                if let Some(missing) = (first..s.filled.len()).find(|b| !s.filled[*b]) {
                    start = missing as u64 * BLOCK;
                    break;
                }
                // Everything from `start` is buffered: wait for a seek elsewhere or close.
                let Ok(next) = shared.cond.wait(s) else { return };
                s = next;
            }
        }

        let range = format!("bytes={start}-");
        let response = request(&client, &url, &headers, &range).send();
        let mut response = match response {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => return fail(&shared, format!("stream HTTP {}", r.status().as_u16())),
            Err(e) => return fail(&shared, format!("stream: {}", e.without_url())),
        };
        // A server that ignores Range sends the whole file from 0: skip to `start`.
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT && start > 0 {
            let skipped = io::copy(&mut (&mut response).take(start), &mut io::sink());
            match skipped {
                Ok(n) if n == start => {}
                Ok(_) => return fail(&shared, "stream ended early".into()),
                Err(e) => return fail(&shared, format!("stream read: {e}")),
            }
        }
        let mut offset = start;
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = match response.read(&mut chunk) {
                Ok(0) => {
                    if offset < shared.len {
                        return fail(&shared, "stream ended early".into());
                    }
                    break;
                }
                Ok(n) => n,
                Err(e) => return fail(&shared, format!("stream read: {e}")),
            };
            let Ok(mut s) = shared.state.lock() else { return };
            if s.closed {
                return;
            }
            let end = (offset + n as u64).min(shared.len);
            let count = usize::try_from(end - offset).unwrap_or(0);
            if let Err(e) = s.buf.write_at(offset, &chunk[..count]) {
                drop(s);
                return fail(&shared, format!("stream buffer: {e}"));
            }
            // A block is complete once this run (which began block-aligned at `start`) has
            // written all of it; the last block ends at the end of the file.
            let first = block_of(offset);
            for (b, done) in s.filled.iter_mut().enumerate().skip(first).take(block_of(end - 1) + 1 - first) {
                let block_start = b as u64 * BLOCK;
                let block_end = (block_start + BLOCK).min(shared.len);
                if block_start >= start && block_end <= end {
                    *done = true;
                }
            }
            offset = end;
            // A seek to a block we won't reach soon restarts the download there.
            let restart = s.want.is_some_and(|w| {
                let wb = block_of(w);
                !s.filled[wb] && (w < offset || w > offset + 8 * BLOCK)
            });
            drop(s);
            shared.cond.notify_all();
            if restart || offset >= shared.len {
                break;
            }
        }
        start = offset.min(shared.len.saturating_sub(1));
    }
}

fn fail(shared: &Shared, message: String) {
    tracing::warn!(error = %message, "audio stream failed");
    if let Ok(mut s) = shared.state.lock() {
        s.error = Some(message);
    }
    shared.cond.notify_all();
}

impl Read for HttpSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.shared.len || out.is_empty() {
            return Ok(0);
        }
        let block = block_of(self.pos);
        let mut s = self.shared.state.lock().map_err(|_| io::Error::other("stream lock poisoned"))?;
        let deadline = std::time::Instant::now() + READ_WAIT;
        while !s.filled[block] {
            if let Some(err) = &s.error {
                return Err(io::Error::other(err.clone()));
            }
            if s.want.is_none() {
                s.want = Some(self.pos);
                self.shared.cond.notify_all();
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "stream stalled"));
            }
            let (next, _) = self
                .shared
                .cond
                .wait_timeout(s, left)
                .map_err(|_| io::Error::other("stream lock poisoned"))?;
            s = next;
        }
        // Serve up to the end of this block (always fully available once filled).
        let block_end = ((block as u64 + 1) * BLOCK).min(self.shared.len);
        let n = usize::try_from((block_end - self.pos).min(out.len() as u64)).unwrap_or(0);
        s.buf.read_at(self.pos, &mut out[..n])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for HttpSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let len = self.shared.len as i128;
        let target = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => len + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if target < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = u64::try_from(target).unwrap_or(u64::MAX).min(self.shared.len);
        if self.pos < self.shared.len {
            if let Ok(mut s) = self.shared.state.lock() {
                if !s.filled[block_of(self.pos)] {
                    s.want = Some(self.pos);
                }
            }
            self.shared.cond.notify_all();
        }
        Ok(self.pos)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    /// Minimal Range-capable HTTP server for tests. `drop_after` closes connections after that
    /// many body bytes (simulates a network drop). Returns the base URL.
    pub fn serve(body: Vec<u8>, drop_after: Option<usize>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = Arc::new(body);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let body = body.clone();
                std::thread::spawn(move || {
                    let mut req = [0u8; 2048];
                    let n = stream.read(&mut req).unwrap_or(0);
                    let head = String::from_utf8_lossy(&req[..n]).to_string();
                    let range = head
                        .lines()
                        .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                        .and_then(|l| l.split('=').nth(1))
                        .map(|r| r.trim().to_owned());
                    let total = body.len();
                    let (start, end) = match range {
                        Some(r) => {
                            let mut parts = r.split('-');
                            let s: usize = parts.next().unwrap().parse().unwrap();
                            let e: usize = parts.next().filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).unwrap_or(total - 1);
                            (s, e.min(total - 1))
                        }
                        None => (0, total - 1),
                    };
                    let slice = &body[start..=end];
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{total}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        slice.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let limit = drop_after.map_or(slice.len(), |d| d.min(slice.len()));
                    let _ = stream.write_all(&slice[..limit]);
                    // Dropping the stream closes the connection (early if `drop_after`).
                });
            }
        });
        format!("http://{addr}/audio.wav")
    }

    /// A server that ignores Range (always 200 with the whole file) and refuses requests
    /// without `referer: ok` (403) or that don't ask for an uncompressed body.
    fn serve_whole_file(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = Arc::new(body);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let body = body.clone();
                std::thread::spawn(move || {
                    let mut req = [0u8; 2048];
                    let n = stream.read(&mut req).unwrap_or(0);
                    let head = String::from_utf8_lossy(&req[..n]).to_ascii_lowercase();
                    if !head.contains("referer: ok") || !head.contains("accept-encoding: identity") {
                        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        return;
                    }
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                });
            }
        });
        format!("http://{addr}/stream")
    }

    #[test]
    fn sends_headers_and_skips_ahead_when_range_is_ignored() {
        let body = data(400_000);
        let url = serve_whole_file(body.clone());
        let refused = HttpSource::open(client(), &url).err().unwrap();
        assert_eq!(refused.status, Some(403), "the status is kept for a re-resolve");

        let target = StreamTarget {
            url: url.clone(),
            headers: vec![("referer".into(), "ok".into())],
            mime: Some("flac".into()),
            known: None,
        };
        let (mut src, handle) = HttpSource::open_target(client(), &target).unwrap();
        assert_eq!(src.len(), 400_000);
        assert_eq!(src.hint.as_deref(), Some("flac"), "the source's type wins over the server's");
        src.seek(SeekFrom::Start(350_000)).unwrap();
        let mut buf = [0u8; 100];
        src.read_exact(&mut buf).unwrap();
        assert_eq!(&buf[..], &body[350_000..350_100]);
        assert!(handle.error().is_none());
        assert!(!format!("{target:?}").contains("\"ok\""), "header values are never printed");
    }

    pub fn client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(5)).build().unwrap()
    }

    fn data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn reads_byte_exact_and_seeks() {
        let body = data(300_000);
        let url = serve(body.clone(), None);
        let (mut src, handle) = HttpSource::open(client(), &url).unwrap();
        assert_eq!(src.len(), 300_000);
        assert_eq!(src.hint.as_deref(), Some("wav"));

        let mut all = Vec::new();
        src.read_to_end(&mut all).unwrap();
        assert_eq!(all, body);

        src.seek(SeekFrom::Start(250_001)).unwrap();
        let mut ten = [0u8; 10];
        src.read_exact(&mut ten).unwrap();
        assert_eq!(&ten, &body[250_001..250_011]);
        src.seek(SeekFrom::End(-5)).unwrap();
        let mut tail = Vec::new();
        src.read_to_end(&mut tail).unwrap();
        assert_eq!(tail, body[299_995..]);
        assert!(handle.error().is_none());
    }

    #[test]
    fn disk_buffer_reads_and_seeks_then_is_deleted_when_the_stream_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let body = data(400_000);
        let url = serve(body.clone(), None);
        let (mut src, handle) = HttpSource::open_in(client(), &StreamTarget::url(&url), Some(dir.path())).unwrap();
        let buffers = || std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(buffers(), 1, "one temporary buffer while it plays");

        src.seek(SeekFrom::Start(350_000)).unwrap();
        let mut ten = [0u8; 10];
        src.read_exact(&mut ten).unwrap();
        assert_eq!(&ten, &body[350_000..350_010]);
        src.seek(SeekFrom::Start(0)).unwrap();
        let mut all = Vec::new();
        src.read_to_end(&mut all).unwrap();
        assert_eq!(all, body);
        assert!(handle.error().is_none());

        drop(src);
        drop(handle);
        // The download thread holds the buffer until it notices the stream closed.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while buffers() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(buffers(), 0, "deleted once the stream is gone");
    }

    #[test]
    fn leftover_buffers_are_purged_and_other_files_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(format!("old{BUFFER_SUFFIX}")), b"x").unwrap();
        std::fs::write(dir.path().join("keep.txt"), b"x").unwrap();
        purge(dir.path());
        let left: Vec<String> =
            std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["keep.txt"]);
    }

    #[test]
    fn seeking_far_ahead_restarts_the_download() {
        let body = data(2_000_000);
        let url = serve(body.clone(), None);
        let (mut src, _handle) = HttpSource::open(client(), &url).unwrap();
        src.seek(SeekFrom::Start(1_900_000)).unwrap();
        let mut buf = [0u8; 64];
        src.read_exact(&mut buf).unwrap();
        assert_eq!(&buf[..], &body[1_900_000..1_900_064]);
    }

    #[test]
    fn connection_drop_is_an_error_not_a_panic() {
        let body = data(500_000);
        let url = serve(body, Some(40_000));
        let (mut src, handle) = HttpSource::open(client(), &url).unwrap();
        let mut all = Vec::new();
        let err = src.read_to_end(&mut all).unwrap_err();
        assert!(err.to_string().contains("ended early") || err.to_string().contains("stream"), "{err}");
        assert!(handle.error().is_some());
        assert!(all.len() < 500_000);
    }

    #[test]
    fn rejects_missing_streams() {
        let err = HttpSource::open(client(), "http://127.0.0.1:9/nothing.mp3").err().unwrap();
        assert!(err.to_string().contains("stream request failed"), "{err}");
    }

    #[test]
    fn hints_from_type_or_url() {
        assert_eq!(hint_from(Some("audio/mpeg"), "https://x/stream"), Some("mp3".into()));
        assert_eq!(hint_from(None, "https://x/a/track.FLAC?sig=1"), Some("flac".into()));
        assert_eq!(hint_from(Some("application/octet-stream"), "https://x/stream"), None);
    }
}
