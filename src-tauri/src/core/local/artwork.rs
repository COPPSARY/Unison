//! Embedded cover → `<app data>/artwork/<hash>.jpg`, at most 600 px (addendum §4 step 5).
//! Named by the hash of the embedded bytes, so an album's tracks share one file.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

pub const MAX_SIDE: u32 = 600;
const JPEG_QUALITY: u8 = 85;

/// Custom URI scheme that serves saved covers to the webview (and nothing else).
pub const SCHEME: &str = "artwork";

/// Base URL of the scheme as the webview sees it. Windows serves custom schemes as
/// `https://<scheme>.localhost` (the app uses https, UP-03); other platforms as `<scheme>://localhost`.
#[cfg(windows)]
pub const BASE_URL: &str = "https://artwork.localhost/";
#[cfg(not(windows))]
pub const BASE_URL: &str = "artwork://localhost/";

/// Saved cover names: 32 hex characters + `.jpg`.
pub fn is_cover_name(name: &str) -> bool {
    name.strip_suffix(".jpg")
        .is_some_and(|hash| hash.len() == 32 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Webview URL for a saved cover file.
pub fn url_for(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    is_cover_name(name).then(|| format!("{BASE_URL}{name}"))
}

/// Answers one `artwork://` request: the cover named by the URL path, from `dir` only.
pub fn serve(dir: &Path, uri_path: &str) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};
    let name = uri_path.trim_start_matches('/');
    let body = is_cover_name(name).then(|| std::fs::read(dir.join(name)).ok()).flatten();
    let built = match body {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            // Names are content hashes, so a cover never changes.
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
    };
    built.unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Saves `bytes` (any JPEG/PNG cover) into `dir`. Returns `None` if it can't be decoded or
/// written; a missing cover never fails an import.
pub fn save_cover(bytes: &[u8], dir: &Path) -> Option<PathBuf> {
    let hash = blake3::hash(bytes).to_hex();
    let path = dir.join(format!("{}.jpg", &hash.as_str()[..32]));
    if path.is_file() {
        return Some(path);
    }
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle)
    } else {
        img
    };
    let mut out = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&img.to_rgb8()).ok()?;
    std::fs::create_dir_all(dir).ok()?;
    // Write then rename so a half-written file is never picked up as a cover.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, out.get_ref()).ok()?;
    std::fs::rename(&tmp, &path).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::local::testing::png;

    #[test]
    fn resizes_large_covers_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = png(1200, 900);
        let path = save_cover(&bytes, dir.path()).unwrap();
        let saved = image::open(&path).unwrap();
        assert_eq!((saved.width(), saved.height()), (600, 450));
        assert_eq!(save_cover(&bytes, dir.path()).unwrap(), path);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn serves_only_saved_covers() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_cover(&png(10, 10), dir.path()).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        assert!(is_cover_name(&name));
        assert_eq!(url_for(&path), Some(format!("{BASE_URL}{name}")));

        let ok = serve(dir.path(), &format!("/{name}"));
        assert_eq!(ok.status(), 200);
        assert_eq!(ok.headers()["content-type"], "image/jpeg");
        assert!(!ok.body().is_empty());

        std::fs::write(dir.path().join("secret.txt"), b"x").unwrap();
        for bad in ["/secret.txt", "/../unison.db", "/%2e%2e/x.jpg", "/", &format!("/{}", name.to_uppercase())] {
            assert_eq!(serve(dir.path(), bad).status(), 404, "{bad}");
        }
        assert!(!is_cover_name("0123456789abcdef0123456789abcde.jpg"));
    }

    #[test]
    fn small_covers_keep_their_size_and_junk_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_cover(&png(300, 300), dir.path()).unwrap();
        assert_eq!(image::open(&path).unwrap().width(), 300);
        assert!(save_cover(b"not an image", dir.path()).is_none());
    }
}
