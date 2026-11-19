//! Turns dropped / chosen paths into the list of audio files to import (addendum §4 step 1).
//! Folders are walked recursively; symlinked folders are not followed (no loops).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Issue, IssueKind};

/// DRM-protected iTunes purchases: always rejected with a reason.
const DRM_EXTENSIONS: [&str; 1] = ["m4p"];

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default()
}

fn is_hidden(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'))
}

/// Absolute form of a path, without resolving symlinks (keeps Windows paths readable).
pub fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Files to import plus per-path problems. Explicitly chosen files with another extension are
/// reported; unrelated files met while walking a folder (covers, playlists …) are ignored.
pub fn collect(paths: &[PathBuf], extensions: &[String]) -> (Vec<PathBuf>, Vec<Issue>) {
    let mut files = Vec::new();
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |path: PathBuf, files: &mut Vec<PathBuf>| {
        if seen.insert(path.clone()) {
            files.push(path);
        }
    };
    for raw in paths {
        let path = absolute(raw);
        let Ok(meta) = std::fs::metadata(&path) else {
            issues.push(Issue::new(&path, IssueKind::Missing));
            continue;
        };
        if meta.is_dir() {
            walk(&path, extensions, &mut |p| push(p, &mut files), &mut issues);
            continue;
        }
        let ext = extension(&path);
        if DRM_EXTENSIONS.contains(&ext.as_str()) {
            issues.push(Issue::new(&path, IssueKind::DrmProtected));
        } else if extensions.contains(&ext) {
            push(path, &mut files);
        } else {
            issues.push(Issue::new(&path, IssueKind::UnsupportedFormat));
        }
    }
    (files, issues)
}

fn walk(dir: &Path, extensions: &[String], found: &mut dyn FnMut(PathBuf), issues: &mut Vec<Issue>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        issues.push(Issue::new(dir, IssueKind::Unreadable));
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if is_hidden(&path) || kind.is_symlink() && path.is_dir() {
            continue;
        }
        if kind.is_dir() {
            walk(&path, extensions, found, issues);
        } else if path.is_file() {
            let ext = extension(&path);
            if DRM_EXTENSIONS.contains(&ext.as_str()) {
                issues.push(Issue::new(&path, IssueKind::DrmProtected));
            } else if extensions.contains(&ext) {
                found(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exts() -> Vec<String> {
        crate::model::LocalSettings::default().extensions
    }

    #[test]
    fn walks_folders_and_reports_chosen_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Album/CD1")).unwrap();
        for f in ["a.mp3", "Album/b.FLAC", "Album/CD1/c.ogg", "Album/cover.jpg", "Album/old.m4p", ".hidden.mp3"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        std::fs::write(root.join("notes.txt"), b"x").unwrap();

        let (files, issues) = collect(&[root.to_path_buf()], &exts());
        let mut names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, vec!["a.mp3", "b.FLAC", "c.ogg"]);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].reason, IssueKind::DrmProtected);

        // Chosen explicitly: unsupported and missing paths get a reason; duplicates collapse.
        let (files, issues) = collect(
            &[root.join("a.mp3"), root.join("notes.txt"), root.join("gone.mp3"), root.join("a.mp3")],
            &exts(),
        );
        assert_eq!(files.len(), 1);
        let reasons: Vec<_> = issues.iter().map(|i| i.reason).collect();
        assert_eq!(reasons, vec![IssueKind::UnsupportedFormat, IssueKind::Missing]);
    }

    #[test]
    fn respects_the_extension_setting() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("b.wav"), b"x").unwrap();
        let (files, _) = collect(&[dir.path().to_path_buf()], &["wav".to_owned()]);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("b.wav"));
    }
}
