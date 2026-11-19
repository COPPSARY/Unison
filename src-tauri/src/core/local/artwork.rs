//! Embedded cover → `<app data>/artwork/<hash>.jpg`, at most 600 px (addendum §4 step 5).
//! Named by the hash of the embedded bytes, so an album's tracks share one file.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;

pub const MAX_SIDE: u32 = 600;
const JPEG_QUALITY: u8 = 85;

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
    fn small_covers_keep_their_size_and_junk_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_cover(&png(300, 300), dir.path()).unwrap();
        assert_eq!(image::open(&path).unwrap().width(), 300);
        assert!(save_cover(b"not an image", dir.path()).is_none());
    }
}
