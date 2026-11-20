//! Downloads (EXT-07, owner decision 2026-11-21): a song can be saved to disk only from a
//! source whose extension is on (and needs no refused runtime feature) and whose result
//! declares an open licence Unison recognises (Creative Commons, CC0, public domain; see
//! [`super::license`]). [`ExtensionManager::download_license`] is that gate and every download
//! goes through it. The audio comes from the same allowlisted stream URL Unison plays; an
//! extension's own `download()` is never called. The saved file joins the same song as its
//! local source.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};
use tauri_specta::Event;
use url::Url;

use super::license::OpenLicense;
use super::{split_key, ExtensionManager};
use crate::core::db;
use crate::core::local::LocalChanged;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::state::AppState;

/// Largest file saved.
pub const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

/// `download://progress`: sent while a download runs (at most every 256 KB) and once at the
/// end with `finished: true` and either `path` or `error`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "download://progress")]
pub struct DownloadProgress {
    pub job_id: u32,
    pub track_id: String,
    pub received: f64,
    pub total: Option<f64>,
    pub finished: bool,
    /// Where the file was saved.
    pub path: Option<String>,
    pub error: Option<AppError>,
}

fn not_allowed(why: &str) -> AppError {
    AppError::new(ErrorCode::DownloadNotAllowed, why)
}

/// File name for a song: `Artist - Title`, without characters Windows forbids.
pub fn file_stem(artists: &[String], title: &str) -> String {
    let raw = match artists.first() {
        Some(a) if !a.trim().is_empty() => format!("{} - {}", a.trim(), title.trim()),
        _ => title.trim().to_owned(),
    };
    let clean: String = raw
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c })
        .take(120)
        .collect();
    let clean = clean.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_owned();
    if clean.is_empty() {
        "Download".to_owned()
    } else {
        clean
    }
}

/// File extension for the audio: from the answer's content type, else the source's hint, else
/// the URL.
/// `None` for anything this build can't play (Opus, playlists, HTML, …).
pub fn audio_extension(hint: Option<&str>, content_type: Option<&str>, url: &str) -> Option<&'static str> {
    let from = |text: &str| -> Option<&'static str> {
        let t = text.trim().to_ascii_lowercase();
        let t = t.split(';').next().unwrap_or_default().trim().trim_start_matches("audio/").trim_start_matches('.').to_owned();
        match t.as_str() {
            "mpeg" | "mp3" | "mpeg3" | "x-mpeg" => Some("mp3"),
            "flac" | "x-flac" => Some("flac"),
            "ogg" | "vorbis" | "x-vorbis+ogg" => Some("ogg"),
            "mp4" | "m4a" | "x-m4a" | "aac" | "aacp" | "x-aac" => Some("m4a"),
            "wav" | "wave" | "x-wav" | "vnd.wave" => Some("wav"),
            _ => None,
        }
    };
    let url_ext = Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(str::to_owned)))
        .and_then(|name| name.rsplit_once('.').map(|(_, e)| e.to_owned()));
    content_type.and_then(from).or_else(|| hint.and_then(from)).or_else(|| url_ext.as_deref().and_then(from))
}

impl ExtensionManager {
    /// The download gate: the source's extension is on and needs no refused runtime feature,
    /// it streams, and the source carries an open licence. Returns that licence.
    pub async fn download_license(&self, source_key: &str) -> AppResult<OpenLicense> {
        let (ext, _) = split_key(source_key).ok_or_else(|| not_allowed("bad source key"))?;
        let installed = self.enabled(ext).await?;
        if !self.streams(&installed.spec) {
            return Err(not_allowed("this extension gives Unison no audio for it"));
        }
        db::extensions::license(&self.db, source_key)
            .await?
            .ok_or_else(|| not_allowed("the source has no open licence"))
    }

    /// Saves one source's audio into `dir` as `Artist - Title.<ext>` (a free name). Writes to a
    /// hidden `.part` file first; nothing is left behind on failure.
    pub async fn download_source(
        &self,
        source_key: &str,
        artists: &[String],
        title: &str,
        dir: &Path,
        progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> AppResult<PathBuf> {
        self.download_license(source_key).await?;
        let (ext_id, _) = split_key(source_key).ok_or_else(|| not_allowed("bad source key"))?;
        let hosts = self.enabled(ext_id).await?.spec.hosts().to_vec();
        let target = self.resolve_stream(source_key).await?;
        let url = Url::parse(&target.url).map_err(|e| AppError::new(ErrorCode::ExtensionFailed, format!("bad stream URL: {e}")))?;
        std::fs::create_dir_all(dir)?;
        let stem = file_stem(artists, title);
        let part = dir.join(format!(".{stem}.{}.part", std::process::id()));
        let result = async {
            let mut file = std::io::BufWriter::new(std::fs::File::create(&part)?);
            let content_type = self.http.download(url, &hosts, &target.headers, &mut file, MAX_DOWNLOAD_BYTES, progress).await?;
            file.flush()?;
            drop(file);
            let known_type = target.known.as_ref().and_then(|(_, t)| t.clone());
            let ext = audio_extension(target.mime.as_deref(), content_type.as_deref().or(known_type.as_deref()), &target.url)
                .ok_or_else(|| AppError::new(ErrorCode::UnsupportedFormat, "the source sent audio Unison can't play"))?;
            let wanted = dir.join(format!("{stem}.{ext}"));
            let path = if wanted.exists() { crate::core::local::copy::free_name(&wanted) } else { wanted };
            std::fs::rename(&part, &path)?;
            Ok(path)
        }
        .await;
        if result.is_err() {
            // Best effort: a half-written file is never kept.
            let _ = std::fs::remove_file(&part);
        }
        result
    }

    /// Saves the song and checks the gate for `source_key`; returns the library track id.
    pub async fn prepare_download(&self, key: &str, source_key: &str) -> AppResult<String> {
        self.download_license(source_key).await?;
        self.save_song(key).await
    }
}

static NEXT_JOB: AtomicU32 = AtomicU32::new(1);

/// Starts downloading `source_key` of the song `key` in the background; returns the job id.
/// The file goes to the music folder (Settings → Local files → where copies go) and becomes
/// the song's local source; `download://progress` reports the outcome.
pub async fn start<R: Runtime>(app: &AppHandle<R>, key: &str, source_key: &str) -> AppResult<u32> {
    let state = app.state::<AppState>();
    let track_id = state.extensions.prepare_download(key, source_key).await?;
    let local = db::settings::load(&state.db).await?.local;
    let dir = crate::core::local::jobs::copy_target(app, local.copy_target.as_deref())?;
    let artwork = crate::core::local::jobs::artwork_dir(app)?;
    let (title, artists_json): (String, String) =
        sqlx::query_as("SELECT title, artists_json FROM tracks WHERE id = ?1").bind(&track_id).fetch_one(&state.db).await?;
    let artists: Vec<String> = serde_json::from_str(&artists_json).unwrap_or_default();
    let job_id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
    let (app, source_key) = (app.clone(), source_key.to_owned());
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let emit = |e: DownloadProgress| {
            if let Err(err) = e.emit(&app) {
                tracing::warn!(error = %err, "emit download progress failed");
            }
        };
        let mut last = 0u64;
        let mut report = |received: u64, total: Option<u64>| {
            if received - last >= 256 * 1024 || total == Some(received) {
                last = received;
                emit(DownloadProgress {
                    job_id,
                    track_id: track_id.clone(),
                    received: received as f64,
                    total: total.map(|t| t as f64),
                    finished: false,
                    path: None,
                    error: None,
                });
            }
        };
        tracing::info!(job_id, source = %source_key, "download started");
        let saved = state.extensions.download_source(&source_key, &artists, &title, &dir, &mut report).await;
        let outcome = match saved {
            Ok(path) => {
                let attached = crate::core::local::import::attach(&state.db, &track_id, &path, &local.extensions, artwork).await;
                match attached {
                    Ok(()) => Ok(path),
                    Err(e) => {
                        // The file can't join the library (e.g. unreadable): don't leave it.
                        let _ = std::fs::remove_file(&path);
                        Err(e)
                    }
                }
            }
            Err(e) => Err(e),
        };
        let (path, error) = match outcome {
            Ok(p) => {
                tracing::info!(job_id, "download finished");
                if let Err(err) = (LocalChanged { added: 1, removed: 0, updated: 0 }).emit(&app) {
                    tracing::warn!(error = %err, "emit local changed failed");
                }
                (Some(p.to_string_lossy().into_owned()), None)
            }
            Err(e) => {
                tracing::warn!(job_id, error = %e, "download failed");
                (None, Some(e))
            }
        };
        emit(DownloadProgress { job_id, track_id, received: last as f64, total: None, finished: true, path, error });
    });
    Ok(job_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_safely() {
        assert_eq!(file_stem(&["A/B".into()], "What? \"Now\""), "A_B - What_ _Now_");
        assert_eq!(file_stem(&[], " ... "), "Download");
        assert_eq!(file_stem(&["Artist".into()], "Song."), "Artist - Song");
    }

    #[test]
    fn picks_a_playable_extension() {
        assert_eq!(audio_extension(Some("flac"), None, "https://x/a"), Some("flac"));
        assert_eq!(audio_extension(Some("flac"), Some("audio/wav"), "https://x/a"), Some("wav"), "the server wins");
        assert_eq!(audio_extension(None, Some("audio/mpeg; charset=x"), "https://x/a"), Some("mp3"));
        assert_eq!(audio_extension(None, Some("application/octet-stream"), "https://x/a/song.ogg?x=1"), Some("ogg"));
        assert_eq!(audio_extension(None, Some("audio/mp4"), "https://x/a"), Some("m4a"));
        assert_eq!(audio_extension(None, Some("audio/opus"), "https://x/a.opus"), None);
        assert_eq!(audio_extension(None, Some("text/html"), "https://x/a"), None);
    }
}
