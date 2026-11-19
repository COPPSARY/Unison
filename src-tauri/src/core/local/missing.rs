//! Missing-file detection (addendum §4 edge cases). A file that vanished from a watched folder
//! was deleted there, so its song leaves the library. Any other missing file (dropped from
//! Downloads, or a watched folder on a drive that's unplugged) stays, greyed out, with
//! "Locate file…". Files that come back are playable again.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::core::auth::oauth::now_unix;
use crate::core::db::local::{self as repo, KnownFile};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Checked {
    /// Deleted from a watched folder: removed from the library.
    pub removed: u32,
    /// Marked missing (can be located).
    pub missing: u32,
    /// Found again.
    pub restored: u32,
}

impl Checked {
    pub fn changed(&self) -> bool {
        self.removed + self.missing + self.restored > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Remove(String),
    Missing(String),
    Restored(String),
}

/// Decides what to do with each file. Blocking (checks the disk).
fn plan(files: Vec<KnownFile>, watched: &[PathBuf], scope: Option<&[PathBuf]>) -> Vec<Action> {
    // A watched folder that's gone itself (unplugged drive, renamed) doesn't delete anything.
    let live: Vec<&PathBuf> = watched.iter().filter(|root| root.is_dir()).collect();
    files
        .into_iter()
        .filter(|f| scope.is_none_or(|scope| scope.iter().any(|s| Path::new(&f.path).starts_with(s))))
        .filter_map(|f| {
            let path = Path::new(&f.path);
            if path.is_file() {
                return (!f.available).then_some(Action::Restored(f.fingerprint));
            }
            if live.iter().any(|root| path.starts_with(root)) {
                Some(Action::Remove(f.fingerprint))
            } else {
                f.available.then_some(Action::Missing(f.fingerprint))
            }
        })
        .collect()
}

/// Checks imported files against the disk. `watched`: the watched folders; `scope`: only files
/// at or under these paths (a watcher batch), `None` = every file.
pub async fn check(pool: &SqlitePool, watched: &[PathBuf], scope: Option<&[PathBuf]>) -> AppResult<Checked> {
    let files = {
        let mut conn = pool.acquire().await?;
        repo::all_files(&mut conn).await?
    };
    let watched = watched.to_vec();
    let scope = scope.map(<[PathBuf]>::to_vec);
    let actions = tokio::task::spawn_blocking(move || plan(files, &watched, scope.as_deref()))
        .await
        .map_err(|e| AppError::internal(format!("missing-file check: {e}")))?;
    if actions.is_empty() {
        return Ok(Checked::default());
    }

    let now = now_unix();
    let mut checked = Checked::default();
    let mut tx = pool.begin().await?;
    for action in actions {
        match action {
            Action::Remove(fp) => {
                repo::remove(&mut tx, &fp).await?;
                checked.removed += 1;
            }
            Action::Missing(fp) => {
                repo::set_missing(&mut tx, &fp, true, now).await?;
                checked.missing += 1;
            }
            Action::Restored(fp) => {
                repo::set_missing(&mut tx, &fp, false, now).await?;
                checked.restored += 1;
            }
        }
    }
    tx.commit().await?;
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(fp: &str, path: &Path, available: bool) -> KnownFile {
        KnownFile { fingerprint: fp.into(), path: path.to_string_lossy().into_owned(), available }
    }

    #[test]
    fn plans_remove_missing_and_restored() {
        let dir = tempfile::tempdir().unwrap();
        let watched = dir.path().join("Music");
        std::fs::create_dir(&watched).unwrap();
        let here = watched.join("here.mp3");
        std::fs::write(&here, b"x").unwrap();
        let back = dir.path().join("back.mp3");
        std::fs::write(&back, b"x").unwrap();
        let unplugged = dir.path().join("USB");

        let files = vec![
            known("here", &here, true),
            known("deleted", &watched.join("deleted.mp3"), true),
            known("dropped", &dir.path().join("Downloads/gone.mp3"), true),
            known("already", &dir.path().join("Downloads/old.mp3"), false),
            known("back", &back, false),
            known("usb", &unplugged.join("song.mp3"), true),
        ];
        let actions = plan(files.clone(), &[watched.clone(), unplugged], None);
        assert_eq!(
            actions,
            vec![
                Action::Remove("deleted".into()),
                Action::Missing("dropped".into()),
                Action::Restored("back".into()),
                Action::Missing("usb".into()),
            ]
        );

        // Scoped to one deleted path: nothing else is touched.
        let scoped = plan(files, std::slice::from_ref(&watched), Some(&[watched.join("deleted.mp3")]));
        assert_eq!(scoped, vec![Action::Remove("deleted".into())]);
    }
}
