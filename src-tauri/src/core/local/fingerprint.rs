//! Dedupe fingerprint (addendum §4): file size + BLAKE3 of the first 1 MB and the last 64 KB.
//! Cheap for large files, stable when only the path changes.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const HEAD: u64 = 1024 * 1024;
const TAIL: u64 = 64 * 1024;

/// Returns `(fingerprint, size)`. The fingerprint is `<size>-<blake3 hex>`.
pub fn fingerprint(path: &Path) -> std::io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; usize::try_from(HEAD.min(size)).unwrap_or(0)];
    file.read_exact(&mut buf)?;
    hasher.update(&buf);
    let tail = TAIL.min(size);
    file.seek(SeekFrom::Start(size - tail))?;
    buf.resize(usize::try_from(tail).unwrap_or(0), 0);
    file.read_exact(&mut buf)?;
    hasher.update(&buf);
    Ok((format!("{size}-{}", hasher.finalize().to_hex()), size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_same_fingerprint_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.path().join("a.mp3"), &data).unwrap();
        std::fs::create_dir(dir.path().join("moved")).unwrap();
        std::fs::write(dir.path().join("moved").join("b.mp3"), &data).unwrap();
        let (a, size) = fingerprint(&dir.path().join("a.mp3")).unwrap();
        let (b, _) = fingerprint(&dir.path().join("moved").join("b.mp3")).unwrap();
        assert_eq!(a, b);
        assert_eq!(size, 3_000_000);
        assert!(a.starts_with("3000000-"));
    }

    #[test]
    fn head_tail_and_size_changes_are_detected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.mp3");
        let mut data = vec![7u8; 2_000_000];
        std::fs::write(&path, &data).unwrap();
        let original = fingerprint(&path).unwrap().0;

        data[10] = 8; // head
        std::fs::write(&path, &data).unwrap();
        assert_ne!(fingerprint(&path).unwrap().0, original);

        data[10] = 7;
        data[1_999_999] = 9; // tail
        std::fs::write(&path, &data).unwrap();
        assert_ne!(fingerprint(&path).unwrap().0, original);

        data[1_999_999] = 7;
        data.push(7); // size
        std::fs::write(&path, &data).unwrap();
        assert_ne!(fingerprint(&path).unwrap().0, original);
    }

    #[test]
    fn small_and_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("s"), b"abc").unwrap();
        std::fs::write(dir.path().join("e"), b"").unwrap();
        assert!(fingerprint(&dir.path().join("s")).unwrap().0.starts_with("3-"));
        assert!(fingerprint(&dir.path().join("e")).unwrap().0.starts_with("0-"));
        assert!(fingerprint(&dir.path().join("missing")).is_err());
    }
}
