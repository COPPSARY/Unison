//! "Copy dropped files into Unison's music folder" (addendum §4). Originals are only read; a
//! dropped folder keeps its name and layout under the target. Files already inside the target
//! are used where they are.

use std::path::{Path, PathBuf};

use super::{Issue, IssueKind};

/// Where a file lands: `target/<path relative to the dropped item's parent>`.
fn destination(file: &Path, roots: &[PathBuf], target: &Path) -> PathBuf {
    let rel = roots
        .iter()
        .filter(|root| file.starts_with(root))
        .max_by_key(|root| root.components().count())
        .and_then(|root| file.strip_prefix(root.parent().unwrap_or(root)).ok())
        .map(Path::to_path_buf)
        .or_else(|| file.file_name().map(PathBuf::from))
        .unwrap_or_default();
    target.join(rel)
}

/// `name.mp3` → `name (2).mp3` … for the first free name.
fn free_name(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

/// Copies `files` (found under `roots`) into `target`. Returns the paths to import: the copies,
/// or the originals for files already inside `target`. A destination that already holds a file
/// of the same size is taken to be the same file (the fingerprint check dedupes it anyway).
pub fn copy_into(files: &[PathBuf], roots: &[PathBuf], target: &Path) -> (Vec<PathBuf>, Vec<Issue>) {
    let target = super::scan::absolute(target);
    let mut out = Vec::with_capacity(files.len());
    let mut issues = Vec::new();
    for file in files {
        if file.starts_with(&target) {
            out.push(file.clone());
            continue;
        }
        let mut dest = destination(file, roots, &target);
        let size = std::fs::metadata(file).map(|m| m.len()).ok();
        if dest.exists() {
            let same = std::fs::metadata(&dest).ok().map(|m| m.len()) == size;
            if same {
                out.push(dest);
                continue;
            }
            dest = free_name(&dest);
        }
        let copied = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(file, &dest));
        match copied {
            Ok(_) => out.push(dest),
            Err(err) => {
                tracing::warn!(error = %err, "copy into music folder failed");
                issues.push(Issue::new(file, IssueKind::CopyFailed));
            }
        }
    }
    (out, issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_files_and_folders_keeping_layout() {
        let src = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let album = src.path().join("Album");
        std::fs::create_dir_all(album.join("CD1")).unwrap();
        std::fs::write(album.join("CD1/a.mp3"), b"aaa").unwrap();
        std::fs::write(src.path().join("single.mp3"), b"s").unwrap();

        let files = vec![album.join("CD1/a.mp3"), src.path().join("single.mp3")];
        let roots = vec![album.clone(), src.path().join("single.mp3")];
        let (out, issues) = copy_into(&files, &roots, target.path());
        assert!(issues.is_empty());
        assert_eq!(out, vec![target.path().join("Album/CD1/a.mp3"), target.path().join("single.mp3")]);
        assert_eq!(std::fs::read(&out[0]).unwrap(), b"aaa");
        // Originals untouched.
        assert_eq!(std::fs::read(album.join("CD1/a.mp3")).unwrap(), b"aaa");

        // Dropping the same files again reuses the copies.
        let (again, _) = copy_into(&files, &roots, target.path());
        assert_eq!(again, out);
        assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 2);

        // A different file with the same name gets a free name.
        std::fs::write(src.path().join("single.mp3"), b"other").unwrap();
        let (renamed, _) = copy_into(&files[1..], &roots[1..], target.path());
        assert_eq!(renamed, vec![target.path().join("single (2).mp3")]);
    }

    #[test]
    fn files_inside_the_target_stay_put() {
        let target = tempfile::tempdir().unwrap();
        let inside = target.path().join("x.mp3");
        std::fs::write(&inside, b"x").unwrap();
        let (out, _) = copy_into(std::slice::from_ref(&inside), std::slice::from_ref(&inside), target.path());
        assert_eq!(out, vec![inside]);
    }
}
