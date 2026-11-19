//! The import job (addendum §4): walk → read (fingerprint, tags, cover) → match → store, in
//! batches of `BATCH_SIZE` with a progress event after each. Disk work runs on the blocking
//! pool and each batch is one transaction, so a 5,000-file folder never stalls the app.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sqlx::SqlitePool;

use super::{fingerprint, scan, tags, ImportReporter, Issue, LocalChanged, LocalImportProgress, BATCH_SIZE, MAX_ISSUES};
use crate::core::auth::oauth::now_unix;
use crate::core::db::local::{self as repo, LocalFileInput, Stored};
use crate::core::matching::matcher::MatchIndex;
use crate::error::{AppError, AppResult};

pub struct ImportConfig {
    /// Lowercase extensions without dots (Settings → Local files).
    pub extensions: Vec<String>,
    /// `<app data>/artwork`.
    pub artwork_dir: PathBuf,
    /// Copy-on-drop: copy files here first and import the copies.
    pub copy_to: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: u32,
    pub updated: u32,
    pub skipped: u32,
    pub failed: u32,
}

enum Prepared {
    /// Same path, size and mtime as last time: not read again.
    Unchanged,
    File(Box<LocalFileInput>),
    Failed(Issue),
}

fn mtime_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}

/// Reads one file. Blocking (disk + decode of the cover).
fn prepare(path: &Path, known: Option<(i64, i64)>, artwork_dir: &Path) -> Prepared {
    let Ok(meta) = std::fs::metadata(path) else {
        return Prepared::Failed(Issue::new(path, super::IssueKind::Missing));
    };
    let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
    let mtime = mtime_of(&meta);
    if known == Some((size, mtime)) {
        return Prepared::Unchanged;
    }
    let file_meta = match tags::read(path) {
        Ok(m) => m,
        Err(reason) => return Prepared::Failed(Issue::new(path, reason)),
    };
    let Ok((fingerprint, size)) = fingerprint::fingerprint(path) else {
        return Prepared::Failed(Issue::new(path, super::IssueKind::Unreadable));
    };
    let cover = file_meta.cover.as_deref().and_then(|bytes| super::artwork::save_cover(bytes, artwork_dir));
    let artwork_url = cover.as_deref().and_then(super::artwork::url_for);
    let artwork_path = cover.map(|p| p.to_string_lossy().into_owned());
    Prepared::File(Box::new(LocalFileInput {
        fingerprint,
        path: path.to_string_lossy().into_owned(),
        size,
        mtime,
        meta: tags::FileMeta { cover: None, ..file_meta },
        artwork_path,
        artwork_url,
    }))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| AppError::internal(format!("import task: {e}")))
}

/// Runs an import and reports progress. Returns the summary; on a database error the last
/// progress event carries the error.
pub async fn run(
    pool: &SqlitePool,
    job_id: u32,
    paths: Vec<PathBuf>,
    config: ImportConfig,
    reporter: &dyn ImportReporter,
) -> AppResult<ImportSummary> {
    let mut progress = LocalImportProgress {
        job_id,
        scanned: 0,
        total: None,
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
        finished: false,
        error: None,
    };
    let mut summary = ImportSummary::default();
    let result = run_inner(pool, paths, config, reporter, &mut progress, &mut summary).await;
    progress.finished = true;
    progress.error = result.as_ref().err().cloned();
    reporter.progress(progress);
    if summary.imported + summary.updated > 0 {
        reporter.changed(LocalChanged { added: summary.imported, removed: 0, updated: summary.updated });
    }
    result.map(|()| summary)
}

fn note(progress: &mut LocalImportProgress, issue: Issue) {
    if progress.errors.len() < MAX_ISSUES {
        progress.errors.push(issue);
    }
}

async fn run_inner(
    pool: &SqlitePool,
    paths: Vec<PathBuf>,
    config: ImportConfig,
    reporter: &dyn ImportReporter,
    progress: &mut LocalImportProgress,
    summary: &mut ImportSummary,
) -> AppResult<()> {
    let extensions = config.extensions.clone();
    let copy_to = config.copy_to.clone();
    let (files, issues) = blocking(move || {
        let (files, mut issues) = scan::collect(&paths, &extensions);
        let Some(target) = copy_to else { return (files, issues) };
        let roots: Vec<PathBuf> = paths.iter().map(|p| scan::absolute(p)).collect();
        let (copies, copy_issues) = super::copy::copy_into(&files, &roots, &target);
        issues.extend(copy_issues);
        (copies, issues)
    })
    .await?;
    summary.failed += u32::try_from(issues.len()).unwrap_or(u32::MAX);
    for issue in issues {
        note(progress, issue);
    }
    progress.total = Some(u32::try_from(files.len()).unwrap_or(u32::MAX));
    reporter.progress(progress.clone());

    let (known, mut index): (HashMap<String, (i64, i64)>, MatchIndex) = {
        let mut conn = pool.acquire().await?;
        (repo::known_files(&mut conn).await?, MatchIndex::load(&mut conn).await?)
    };
    let artwork_dir = config.artwork_dir;

    for chunk in files.chunks(BATCH_SIZE) {
        let batch: Vec<(PathBuf, Option<(i64, i64)>)> = chunk
            .iter()
            .map(|p| (p.clone(), known.get(p.to_string_lossy().as_ref()).copied()))
            .collect();
        let dir = artwork_dir.clone();
        let prepared = blocking(move || {
            batch.iter().map(|(path, known)| prepare(path, *known, &dir)).collect::<Vec<_>>()
        })
        .await?;

        let now = now_unix();
        let mut tx = pool.begin().await?;
        for item in prepared {
            progress.scanned += 1;
            match item {
                Prepared::Unchanged => {
                    summary.skipped += 1;
                    progress.skipped += 1;
                }
                Prepared::Failed(issue) => {
                    summary.failed += 1;
                    note(progress, issue);
                }
                Prepared::File(file) => match repo::store(&mut tx, &mut index, &file, now).await? {
                    Stored::Imported => {
                        summary.imported += 1;
                        progress.imported += 1;
                    }
                    Stored::Updated => {
                        summary.updated += 1;
                        progress.imported += 1;
                    }
                    Stored::Unchanged => {
                        summary.skipped += 1;
                        progress.skipped += 1;
                    }
                },
            }
        }
        tx.commit().await?;
        reporter.progress(progress.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::core::db;
    use crate::core::db::library::{upsert_library, upsert_track, LibraryKind, TrackInput};
    use crate::core::local::testing::{png, tag, write_wav};
    use crate::core::local::IssueKind;
    use crate::model::ProviderId;

    #[derive(Default)]
    struct Recorder {
        progress: Mutex<Vec<LocalImportProgress>>,
        changed: Mutex<Vec<LocalChanged>>,
    }

    impl ImportReporter for Recorder {
        fn progress(&self, event: LocalImportProgress) {
            self.progress.lock().unwrap().push(event);
        }
        fn changed(&self, event: LocalChanged) {
            self.changed.lock().unwrap().push(event);
        }
    }

    struct Env {
        dir: tempfile::TempDir,
        pool: SqlitePool,
        copy_to: Option<PathBuf>,
    }

    impl Env {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let pool = db::open(&dir.path().join("unison.db")).await.unwrap();
            std::fs::create_dir(dir.path().join("music")).unwrap();
            Self { dir, pool, copy_to: None }
        }

        fn music(&self) -> PathBuf {
            self.dir.path().join("music")
        }

        async fn import(&self, paths: Vec<PathBuf>) -> (ImportSummary, Recorder) {
            let rec = Recorder::default();
            let config = ImportConfig {
                extensions: crate::model::LocalSettings::default().extensions,
                artwork_dir: self.dir.path().join("artwork"),
                copy_to: self.copy_to.clone(),
            };
            let summary = run(&self.pool, 1, paths, config, &rec).await.unwrap();
            (summary, rec)
        }

        async fn count(&self, sql: &'static str) -> i64 {
            sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn imports_a_thousand_files_in_batches_without_duplicates() {
        let env = Env::new().await;
        for i in 0..1_000u32 {
            let sub = env.music().join(format!("Artist {}", i % 20));
            std::fs::create_dir_all(&sub).unwrap();
            write_wav(&sub.join(format!("Artist {} - Song number {i}.wav", i % 20)), 0.05, i);
        }

        let (summary, rec) = env.import(vec![env.music()]).await;
        assert_eq!(summary, ImportSummary { imported: 1_000, ..ImportSummary::default() });
        let events = rec.progress.lock().unwrap().clone();
        // Walk event, five batch events, final event.
        assert_eq!(events.len(), 7);
        assert_eq!(events[0].total, Some(1_000));
        assert_eq!(events[1].scanned, 200);
        let last = events.last().unwrap();
        assert!(last.finished && last.error.is_none());
        assert_eq!((last.scanned, last.imported, last.skipped), (1_000, 1_000, 0));
        assert_eq!(*rec.changed.lock().unwrap(), vec![LocalChanged { added: 1_000, removed: 0, updated: 0 }]);

        assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 1_000);
        assert_eq!(env.count("SELECT COUNT(*) FROM local_files").await, 1_000);
        assert_eq!(env.count("SELECT COUNT(*) FROM track_sources WHERE provider = 'local'").await, 1_000);
        assert_eq!(env.count("SELECT COUNT(*) FROM library WHERE provider = 'local' AND kind = 'local'").await, 1_000);

        // Re-import: nothing read again, nothing duplicated, no change event.
        let (summary, rec) = env.import(vec![env.music()]).await;
        assert_eq!(summary, ImportSummary { skipped: 1_000, ..ImportSummary::default() });
        assert!(rec.changed.lock().unwrap().is_empty());
        assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 1_000);
        assert_eq!(env.count("SELECT COUNT(*) FROM local_files").await, 1_000);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_local_copy_of_a_liked_spotify_song_merges_into_one_track() {
        let env = Env::new().await;
        let mut conn = env.pool.acquire().await.unwrap();
        let liked = TrackInput {
            item_id: "sp1".into(),
            url: "https://open.spotify.com/track/sp1".into(),
            title: "Blinding Lights".into(),
            artists: vec!["The Weeknd".into()],
            album: Some("After Hours".into()),
            duration_ms: Some(2_000),
            isrc: None,
            artwork: Some("https://i.scdn.co/image/x".into()),
            available: true,
        };
        let track_id = upsert_track(&mut conn, ProviderId::Spotify, &liked, 0).await.unwrap();
        upsert_library(&mut conn, ProviderId::Spotify, LibraryKind::Liked, "sp1", Some(&track_id), Some(0))
            .await
            .unwrap();
        // A different song by the same artist that must stay separate.
        let other = TrackInput { item_id: "sp2".into(), title: "Save Your Tears".into(), ..liked.clone() };
        upsert_track(&mut conn, ProviderId::Spotify, &other, 0).await.unwrap();
        drop(conn);

        let file = env.music().join("01 Blinding Lights.wav");
        write_wav(&file, 2.0, 1);
        tag(&file, "Blinding Lights (Official Audio)", "The Weeknd", None, Some(png(800, 800)));
        let separate = env.music().join("Save Your Tears (Live).wav");
        write_wav(&separate, 2.0, 2);
        tag(&separate, "Save Your Tears (Live)", "The Weeknd", None, None);

        let (summary, _) = env.import(vec![env.music()]).await;
        assert_eq!(summary.imported, 2);
        assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 3, "only the live version is new");

        let (local_track, confidence, quality, gain): (String, f64, Option<String>, Option<f64>) = sqlx::query_as(
            "SELECT s.track_id, s.match_confidence, s.quality, s.replaygain_track FROM track_sources s
             JOIN local_files f ON f.fingerprint = s.item_id
             WHERE s.provider = 'local' AND f.path LIKE '%Blinding%'",
        )
        .fetch_one(&env.pool)
        .await
        .unwrap();
        assert_eq!(local_track, track_id);
        assert!(confidence > 0.95, "{confidence}");
        assert_eq!(quality.as_deref(), Some("wav"));
        assert_eq!(gain, Some(-6.5));
        // The Spotify metadata stays; the liked row still points at the same track.
        let (title, artwork): (String, Option<String>) =
            sqlx::query_as("SELECT title, artwork_url FROM tracks WHERE id = ?1").bind(&track_id).fetch_one(&env.pool).await.unwrap();
        assert_eq!(title, "Blinding Lights");
        assert_eq!(artwork.as_deref(), Some("https://i.scdn.co/image/x"));
        // Cover saved, resized.
        let art: Option<String> = sqlx::query_scalar("SELECT artwork_path FROM local_files WHERE path LIKE '%Blinding%'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
        let img = image::open(art.unwrap()).unwrap();
        assert_eq!(img.width(), 600);
        // The merged track keeps Spotify's artwork; a local-only track gets the saved cover's URL.
        let live_art: Option<String> = sqlx::query_scalar(
            "SELECT t.artwork_url FROM tracks t JOIN track_sources s ON s.track_id = t.id
             JOIN local_files f ON f.fingerprint = s.item_id WHERE f.path LIKE '%Live%'",
        )
        .fetch_one(&env.pool)
        .await
        .unwrap();
        assert_eq!(live_art, None, "no embedded cover");

        // The resolver now sees both sources for the liked song.
        let sources = crate::core::playback::resolver::stored_sources(&env.pool, &track_id).await.unwrap();
        let mut kinds: Vec<_> = sources.iter().map(|s| s.source.kind).collect();
        kinds.sort_by_key(|k| format!("{k:?}"));
        assert_eq!(kinds, vec![crate::model::SourceKind::Local, crate::model::SourceKind::Spotify]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn isrc_from_tags_merges_even_with_other_titles() {
        let env = Env::new().await;
        let mut conn = env.pool.acquire().await.unwrap();
        let sp = TrackInput {
            item_id: "sp1".into(),
            url: "u".into(),
            title: "Song".into(),
            artists: vec!["A".into()],
            album: None,
            duration_ms: Some(1_000),
            isrc: Some("USUM71400001".into()),
            artwork: None,
            available: true,
        };
        let id = upsert_track(&mut conn, ProviderId::Spotify, &sp, 0).await.unwrap();
        drop(conn);
        let file = env.music().join("x.wav");
        write_wav(&file, 0.5, 3);
        tag(&file, "Completely Different Name", "B", Some("US-UM7-14-00001"), None);
        env.import(vec![file]).await;
        let local: String = sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = 'local'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
        assert_eq!(local, id);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn moved_copied_and_changed_files() {
        let env = Env::new().await;
        let a = env.music().join("Artist - Song.wav");
        write_wav(&a, 0.2, 10);
        env.import(vec![a.clone()]).await;
        let track: String = sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = 'local'")
            .fetch_one(&env.pool)
            .await
            .unwrap();

        // Same file dropped again from elsewhere while the original still exists: no duplicate.
        let copy = env.dir.path().join("Downloads - copy.wav");
        std::fs::copy(&a, &copy).unwrap();
        let (s, _) = env.import(vec![copy.clone()]).await;
        assert_eq!(s.skipped, 1);
        assert_eq!(env.count("SELECT COUNT(*) FROM local_files").await, 1);

        // Original moved away: importing the new location follows the file.
        std::fs::remove_file(&a).unwrap();
        let (s, rec) = env.import(vec![copy.clone()]).await;
        assert_eq!(s.updated, 1);
        assert_eq!(rec.changed.lock().unwrap()[0].updated, 1);
        let url: String = sqlx::query_scalar("SELECT url FROM track_sources WHERE provider = 'local'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
        assert_eq!(url, scan::absolute(&copy).to_string_lossy());
        assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 1);

        // Contents change at the same path (e.g. re-tagged): old entry replaced, one track left.
        write_wav(&copy, 0.3, 11);
        tag(&copy, "Song", "Artist", None, None);
        let (s, _) = env.import(vec![copy.clone()]).await;
        assert_eq!(s.updated, 1);
        assert_eq!(env.count("SELECT COUNT(*) FROM local_files").await, 1);
        assert_eq!(env.count("SELECT COUNT(*) FROM track_sources").await, 1);
        assert_eq!(env.count("SELECT COUNT(*) FROM library").await, 1);
        assert_eq!(env.count("SELECT COUNT(*) FROM tracks").await, 1);
        let now_track: String = sqlx::query_scalar("SELECT track_id FROM track_sources")
            .fetch_one(&env.pool)
            .await
            .unwrap();
        assert_ne!(now_track, track, "orphaned track replaced by a fresh row");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn local_only_tracks_get_a_cover_url_and_copy_on_drop_imports_copies() {
        let mut env = Env::new().await;
        let downloads = env.dir.path().join("Downloads");
        std::fs::create_dir(&downloads).unwrap();
        let file = downloads.join("song.wav");
        write_wav(&file, 0.2, 30);
        tag(&file, "Fresh", "Someone", None, Some(png(64, 64)));
        let target = env.dir.path().join("Music").join("Unison");
        env.copy_to = Some(target.clone());

        let (summary, _) = env.import(vec![file.clone()]).await;
        assert_eq!(summary.imported, 1);
        let (url, art): (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT s.url, t.artwork_url FROM track_sources s JOIN tracks t ON t.id = s.track_id",
        )
        .fetch_one(&env.pool)
        .await
        .unwrap();
        assert_eq!(url.as_deref(), Some(scan::absolute(&target.join("song.wav")).to_string_lossy().as_ref()));
        assert!(target.join("song.wav").is_file());
        assert!(file.is_file(), "original kept");
        let art = art.unwrap();
        assert!(art.starts_with(crate::core::local::artwork::BASE_URL), "{art}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_files_get_per_file_reasons() {
        let env = Env::new().await;
        let good = env.music().join("Good - One.wav");
        write_wav(&good, 0.1, 20);
        std::fs::write(env.music().join("broken.mp3"), b"not audio at all").unwrap();
        std::fs::write(env.music().join("bought.m4p"), b"drm").unwrap();
        let text = env.dir.path().join("readme.txt");
        std::fs::write(&text, b"x").unwrap();

        let (summary, rec) = env.import(vec![env.music(), text, env.dir.path().join("gone")]).await;
        assert_eq!(summary.imported, 1);
        assert_eq!(summary.failed, 4);
        let last = rec.progress.lock().unwrap().last().cloned().unwrap();
        let mut reasons: Vec<IssueKind> = last.errors.iter().map(|e| e.reason).collect();
        reasons.sort_by_key(|r| format!("{r:?}"));
        assert_eq!(
            reasons,
            vec![IssueKind::DrmProtected, IssueKind::Missing, IssueKind::Unreadable, IssueKind::UnsupportedFormat]
        );
        assert!(last.errors.iter().any(|e| e.path.ends_with("broken.mp3") && e.reason == IssueKind::Unreadable));
    }
}
