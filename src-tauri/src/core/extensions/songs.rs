//! Songs across extensions (EXT-04): one search list where the same song from several
//! extensions shows once, and the track page's sources (with each extension's stream
//! qualities), playing from a chosen source, and liking.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::task::JoinSet;

use super::manifest::MappedItem;
use super::script::{effective_quality, QualityOption};
use super::{
    capabilities, search_one, split_key, Capability, ExtensionManager, ExtensionResults, SCRIPT_TIMEOUT, SEARCH_TIMEOUT,
};
use crate::core::db;
use crate::core::db::extensions::{Installed, Spec, PROVIDER};
use crate::core::db::library::TrackInput;
use crate::core::matching::matcher::{merges, score, MatchKey};
use crate::core::now_unix;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{StreamQuality, Track};

/// How long "no extension that streams has this song" is remembered.
pub const MISS_TTL: Duration = Duration::from_secs(10 * 60);

/// One place a song can play from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongSource {
    /// Source key `<extension>:<item>`.
    pub key: String,
    pub extension_id: String,
    pub extension_name: String,
    pub icon: Option<String>,
    /// The extension gives Unison a stream for it.
    pub can_play: bool,
    pub qualities: Vec<QualityOption>,
    /// The quality Unison streams (the user's pick, else the extension's first).
    pub quality: Option<String>,
    /// Web page of the song on that service ("" when unknown).
    pub url: String,
}

/// A song, merged across the extensions that have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// Key of the first source; the track page's address.
    pub key: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork: Option<String>,
    pub isrc: Option<String>,
    pub sources: Vec<SongSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchFailure {
    pub extension_id: String,
    pub extension_name: String,
    pub error: AppError,
}

/// Search across every extension: songs plus the extensions that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchAll {
    pub songs: Vec<Song>,
    pub failures: Vec<SearchFailure>,
}

/// The track page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongDetail {
    pub song: Song,
    pub liked: bool,
    /// Library track id once saved (played or liked).
    pub track_id: Option<String>,
}

/// What Unison knows about an installed extension when listing sources.
struct SourceInfo {
    name: String,
    icon: Option<String>,
    can_play: bool,
    qualities: Vec<QualityOption>,
    quality: Option<String>,
}

fn source_info(i: &Installed, tier: StreamQuality, download_only: bool) -> SourceInfo {
    match &i.spec {
        Spec::Declarative(m) => SourceInfo { name: m.name.clone(), icon: None, can_play: true, qualities: Vec::new(), quality: None },
        Spec::Script { manifest, .. } => SourceInfo {
            name: manifest.name.clone(),
            icon: manifest.icon.clone(),
            can_play: manifest.can_stream() && !download_only,
            qualities: manifest.qualities.clone(),
            // What Unison would stream now: a pick, else the Settings tier, else its default.
            quality: effective_quality(manifest, i.quality.as_deref(), tier),
        },
    }
}

fn song_source(key: &str, url: &str, infos: &HashMap<String, SourceInfo>) -> Option<SongSource> {
    let (ext, _) = split_key(key)?;
    let info = infos.get(ext)?;
    Some(SongSource {
        key: key.to_owned(),
        extension_id: ext.to_owned(),
        extension_name: info.name.clone(),
        icon: info.icon.clone(),
        can_play: info.can_play,
        qualities: info.qualities.clone(),
        quality: info.quality.clone(),
        url: url.to_owned(),
    })
}

/// Same song: equal ISRCs, or the matcher's merge rule (§8.2).
fn same_song(a: &MatchKey, b: &MatchKey) -> bool {
    match (&a.isrc, &b.isrc) {
        (Some(x), Some(y)) => x == y,
        _ => score(a, b).is_some_and(|(s, d)| merges(s, d)),
    }
}

/// Merges per-extension results into one list, interleaving by rank so every extension's best
/// matches come first. A song never takes two sources from the same extension.
fn merge(results: &[ExtensionResults], infos: &HashMap<String, SourceInfo>) -> Vec<Song> {
    let mut songs: Vec<(MatchKey, Song)> = Vec::new();
    let longest = results.iter().map(|r| r.tracks.len()).max().unwrap_or(0);
    for rank in 0..longest {
        for r in results {
            let Some(t) = r.tracks.get(rank) else { continue };
            let url = t.sources.first().map(|s| s.url.as_str()).unwrap_or_default();
            let Some(source) = song_source(&t.id, url, infos) else { continue };
            let key = MatchKey::new(&t.title, &t.artists, t.duration_ms, t.isrc.as_deref());
            let found = songs.iter_mut().find(|(k, s)| {
                same_song(k, &key) && !s.sources.iter().any(|x| x.extension_id == source.extension_id)
            });
            match found {
                Some((_, song)) => {
                    song.artwork = song.artwork.take().or_else(|| t.artwork.clone());
                    song.album = song.album.take().or_else(|| t.album.clone());
                    song.isrc = song.isrc.take().or_else(|| t.isrc.clone());
                    song.sources.push(source);
                }
                None => songs.push((key, song_from(t, source))),
            }
        }
    }
    songs.into_iter().map(|(_, s)| s).collect()
}

fn song_from(t: &Track, source: SongSource) -> Song {
    Song {
        key: t.id.clone(),
        title: t.title.clone(),
        artists: t.artists.clone(),
        album: t.album.clone(),
        duration_ms: t.duration_ms,
        artwork: t.artwork.clone(),
        isrc: t.isrc.clone(),
        sources: vec![source],
    }
}

/// `tracks` row of a stored song.
type StoredRow = (String, String, String, Option<String>, Option<i64>, Option<String>, Option<String>);

impl ExtensionManager {
    async fn source_infos(&self) -> AppResult<HashMap<String, SourceInfo>> {
        let tier = db::settings::load(&self.db).await?.player.stream_quality;
        Ok(db::extensions::list(&self.db)
            .await?
            .iter()
            .filter(|i| i.enabled)
            .map(|i| (i.spec.id().to_owned(), source_info(i, tier, self.is_download_only(i.spec.id()))))
            .collect())
    }

    /// Searches every extension and merges the results into one song list.
    pub async fn search_all(&self, query: &str) -> AppResult<SearchAll> {
        let results = self.search(query).await?;
        let infos = self.source_infos().await?;
        let songs = merge(&results, &infos);
        if let Ok(mut map) = self.songs.lock() {
            if map.len() > 2000 {
                map.clear();
            }
            for song in &songs {
                for s in &song.sources {
                    map.insert(s.key.clone(), song.clone());
                }
            }
        }
        let failures = results
            .into_iter()
            .filter_map(|r| {
                r.error.map(|error| SearchFailure { extension_id: r.extension_id, extension_name: r.extension_name, error })
            })
            .collect();
        Ok(SearchAll { songs, failures })
    }

    /// Library track that holds source `key`, if saved.
    async fn track_of(&self, key: &str) -> AppResult<Option<String>> {
        Ok(sqlx::query_scalar("SELECT track_id FROM track_sources WHERE provider = ?1 AND item_id = ?2")
            .bind(PROVIDER)
            .bind(key)
            .fetch_optional(&self.db)
            .await?)
    }

    /// A saved song read back from the library (after a restart, or from Liked songs).
    async fn stored_song(&self, track_id: &str, infos: &HashMap<String, SourceInfo>) -> AppResult<Option<Song>> {
        let row: Option<StoredRow> = sqlx::query_as(
            "SELECT id, title, artists_json, album, duration_ms, artwork_url, isrc FROM tracks WHERE id = ?1",
        )
        .bind(track_id)
        .fetch_optional(&self.db)
        .await?;
        let Some((_, title, artists_json, album, duration, artwork, isrc)) = row else { return Ok(None) };
        let sources: Vec<(String, String)> = sqlx::query_as(
            "SELECT item_id, url FROM track_sources WHERE track_id = ?1 AND provider = ?2 ORDER BY preference DESC, rowid",
        )
        .bind(track_id)
        .bind(PROVIDER)
        .fetch_all(&self.db)
        .await?;
        let sources: Vec<SongSource> = sources.iter().filter_map(|(k, u)| song_source(k, u, infos)).collect();
        let Some(first) = sources.first() else { return Ok(None) };
        Ok(Some(Song {
            key: first.key.clone(),
            title,
            artists: serde_json::from_str(&artists_json).unwrap_or_default(),
            album,
            duration_ms: duration.and_then(|d| u32::try_from(d).ok()),
            artwork,
            isrc,
            sources,
        }))
    }

    /// The track page for source `key` (any of the song's sources).
    pub async fn song(&self, key: &str) -> AppResult<SongDetail> {
        let infos = self.source_infos().await?;
        let remembered = self.songs.lock().ok().and_then(|m| m.get(key).cloned());
        let track_id = self.track_of(key).await?;
        let song = match (remembered, &track_id) {
            (Some(mut song), _) => {
                // Sources reflect the extensions as they are now (turned off, new quality).
                song.sources = song.sources.iter().filter_map(|s| song_source(&s.key, &s.url, &infos)).collect();
                song
            }
            (None, Some(id)) => self.stored_song(id, &infos).await?.ok_or_else(|| AppError::not_found("song not found"))?,
            (None, None) => return Err(AppError::not_found("song not found; search again")),
        };
        let liked = match &track_id {
            Some(id) => db::library::is_liked(&mut *self.db.acquire().await?, id).await?,
            None => false,
        };
        Ok(SongDetail { song, liked, track_id })
    }

    /// Saves a song (all its sources on one library track) and returns the track id.
    async fn save_song(&self, key: &str) -> AppResult<String> {
        if let Some(id) = self.track_of(key).await? {
            // Already saved; add sources found since (e.g. a new search).
            let song = self.songs.lock().ok().and_then(|m| m.get(key).cloned());
            if let Some(song) = song {
                let inputs = self.inputs(&song)?;
                let mut tx = self.db.begin().await?;
                for input in &inputs {
                    db::library::attach_source(&mut tx, &id, PROVIDER, input, now_unix()).await?;
                }
                tx.commit().await?;
            }
            return Ok(id);
        }
        let song = self
            .songs
            .lock()
            .ok()
            .and_then(|m| m.get(key).cloned())
            .ok_or_else(|| AppError::not_found("search result expired; search again"))?;
        let inputs = self.inputs(&song)?;
        let now = now_unix();
        let mut tx = self.db.begin().await?;
        let mut iter = inputs.iter();
        let first = iter.next().ok_or_else(|| AppError::not_found("song has no source"))?;
        let id = db::library::upsert_track(&mut tx, PROVIDER, first, now).await?;
        for input in iter {
            db::library::attach_source(&mut tx, &id, PROVIDER, input, now).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// Search metadata of each source of a song (Rust's own copy, never the UI's).
    fn inputs(&self, song: &Song) -> AppResult<Vec<TrackInput>> {
        let map = self.remembered.lock().map_err(|_| AppError::internal("extension results lock poisoned"))?;
        Ok(song.sources.iter().filter_map(|s| map.get(&s.key).cloned()).collect())
    }

    /// Saves the song, makes `source_key` play first (with `quality` for its extension when
    /// given) and returns the library track id to play.
    pub async fn prepare_play(&self, key: &str, source_key: &str, quality: Option<&str>) -> AppResult<String> {
        tracing::info!(song = %key, source = %source_key, quality = ?quality, "play requested");
        let id = self.save_song(key).await?;
        let (ext, _) = split_key(source_key).ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "bad source key"))?;
        // A pick applies to this play only; without one the Settings quality (or the
        // extension's default) is used. A pick the extension doesn't offer is ignored.
        let installed = db::extensions::get(&self.db, ext).await?.ok_or_else(|| AppError::not_found("extension not installed"))?;
        let pick = quality.filter(|q| matches!(&installed.spec, Spec::Script { manifest, .. } if manifest.qualities.iter().any(|o| o.id == *q)));
        if quality.is_some() && pick.is_none() {
            tracing::info!(extension = %ext, quality = ?quality, "quality not offered; using the default");
        }
        db::extensions::set_quality(&self.db, ext, pick).await?;
        let mut conn = self.db.acquire().await?;
        db::library::prefer_source(&mut conn, PROVIDER, source_key, now_unix()).await?;
        Ok(id)
    }

    /// When none of a track's sources can stream (metadata-only extensions), searches the
    /// extensions that can both search and stream for the same song and attaches the first
    /// strict match (equal ISRC, else the §8.2 merge rule) to the same track. Never attaches
    /// a different song. Returns whether a source was added; a miss is remembered for
    /// [`MISS_TTL`] so repeated presses don't search again.
    pub async fn find_stream_source(&self, track_id: &str) -> AppResult<bool> {
        if self.misses.lock().ok().and_then(|m| m.get(track_id).copied()).is_some_and(|at| at.elapsed() < MISS_TTL) {
            return Ok(false);
        }
        let row: Option<(String, String, Option<i64>, Option<String>)> =
            sqlx::query_as("SELECT title, artists_json, duration_ms, isrc FROM tracks WHERE id = ?1")
                .bind(track_id)
                .fetch_optional(&self.db)
                .await?;
        let Some((title, artists_json, duration, isrc)) = row else { return Ok(false) };
        let artists: Vec<String> = serde_json::from_str(&artists_json).unwrap_or_default();
        let duration = duration.and_then(|d| u32::try_from(d).ok());
        let have: Vec<String> =
            sqlx::query_scalar("SELECT item_id FROM track_sources WHERE track_id = ?1 AND provider = ?2")
                .bind(track_id)
                .bind(PROVIDER)
                .fetch_all(&self.db)
                .await?;
        let have_ext: Vec<&str> = have.iter().filter_map(|k| split_key(k).map(|(e, _)| e)).collect();
        let streamers: Vec<Spec> = db::extensions::list(&self.db)
            .await?
            .into_iter()
            .filter(|i| i.enabled && !have_ext.contains(&i.spec.id()))
            .filter(|i| capabilities(&i.spec).contains(&Capability::Search) && self.streams(&i.spec))
            .map(|i| i.spec)
            .collect();
        if streamers.is_empty() {
            tracing::info!(track = %track_id, "no extension that streams is enabled");
            self.remember_miss(track_id);
            return Ok(false);
        }

        let query = match artists.first() {
            Some(artist) => format!("{artist} {title}"),
            None => title.clone(),
        };
        let wanted = MatchKey::new(&title, &artists, duration, isrc.as_deref());
        let mut set = JoinSet::new();
        for (order, spec) in streamers.into_iter().enumerate() {
            let (http, art, q) = (self.http.clone(), self.art.clone(), query.clone());
            let (db, secrets) = (self.db.clone(), self.secrets.clone());
            set.spawn(async move {
                let limit = match &spec {
                    Spec::Declarative(_) => SEARCH_TIMEOUT,
                    Spec::Script { .. } => SCRIPT_TIMEOUT + Duration::from_secs(5),
                };
                let cancel = Arc::new(AtomicBool::new(false));
                let found = tokio::time::timeout(limit, search_one(&http, &art, &db, &secrets, &spec, &q, cancel)).await;
                (order, spec.id().to_owned(), found)
            });
        }
        let mut hits: Vec<(usize, MappedItem, Option<String>)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            let Ok((order, ext, found)) = joined else { continue };
            let items = match found {
                Ok(Ok(items)) => items,
                Ok(Err(e)) => {
                    tracing::warn!(extension = %ext, error = %e, "stream source search failed");
                    continue;
                }
                Err(_) => {
                    tracing::warn!(extension = %ext, "stream source search timed out");
                    continue;
                }
            };
            let hit = items.into_iter().find(|(item, _)| {
                let t = &item.track;
                same_song(&wanted, &MatchKey::new(&t.title, &t.artists, t.duration_ms, t.isrc.as_deref()))
            });
            if let Some((item, art)) = hit {
                hits.push((order, item, art));
            }
        }
        if hits.is_empty() {
            tracing::info!(track = %track_id, query = %query, "no extension that streams has this song");
            self.remember_miss(track_id);
            return Ok(false);
        }
        // Every match is attached (in the user's extension order), so one that turns out to be
        // download-only can't hide another that streams; the player tries them in turn.
        hits.sort_by_key(|(order, _, _)| *order);
        let now = now_unix();
        let mut added: Vec<TrackInput> = Vec::new();
        let mut tx = self.db.begin().await?;
        for (_, item, art) in &hits {
            let mut input = item.track.clone();
            input.artwork = art.clone();
            db::library::attach_source(&mut tx, track_id, PROVIDER, &input, now).await?;
            added.push(input);
        }
        tx.commit().await?;
        self.remember(&hits.into_iter().map(|(_, item, art)| (item, art)).collect::<Vec<_>>());
        // A song already open in Search or on its page shows the new sources too.
        let infos = self.source_infos().await?;
        let new_sources: Vec<SongSource> = added.iter().filter_map(|i| song_source(&i.item_id, &i.url, &infos)).collect();
        if let Ok(mut songs) = self.songs.lock() {
            // The map holds each song once per source key; update each song once.
            let updated: HashMap<String, Song> = songs
                .values()
                .filter(|s| s.sources.iter().any(|x| have.contains(&x.key)))
                .map(|s| (s.key.clone(), s.clone()))
                .collect();
            for mut song in updated.into_values() {
                for source in &new_sources {
                    if !song.sources.iter().any(|x| x.key == source.key) {
                        song.sources.push(source.clone());
                    }
                }
                for s in &song.sources {
                    songs.insert(s.key.clone(), song.clone());
                }
            }
        }
        if let Ok(mut misses) = self.misses.lock() {
            misses.remove(track_id);
        }
        let keys: Vec<&str> = added.iter().map(|i| i.item_id.as_str()).collect();
        tracing::info!(track = %track_id, sources = ?keys, "found stream sources for the song");
        Ok(true)
    }

    /// Learnt at play time: the extension's getDownloadUrl is a stub (answers null).
    pub(super) fn is_download_only(&self, ext_id: &str) -> bool {
        self.download_only.lock().is_ok_and(|s| s.contains(ext_id))
    }

    /// Gives streams: declares getDownloadUrl (or is declarative) and hasn't answered null.
    pub(super) fn streams(&self, spec: &Spec) -> bool {
        capabilities(spec).contains(&Capability::Play) && !self.is_download_only(spec.id())
    }

    fn remember_miss(&self, track_id: &str) {
        if let Ok(mut misses) = self.misses.lock() {
            if misses.len() > 2000 {
                misses.retain(|_, at| at.elapsed() < MISS_TTL);
            }
            misses.insert(track_id.to_owned(), std::time::Instant::now());
        }
    }

    /// Likes or unlikes a song; Liked songs, Albums and Artists follow.
    pub async fn like_song(&self, key: &str, liked: bool) -> AppResult<bool> {
        let id = self.save_song(key).await?;
        let mut tx = self.db.begin().await?;
        if liked {
            db::library::like_track(&mut tx, &id, PROVIDER, key, now_unix()).await?;
        } else {
            db::library::unlike_track(&mut tx, &id, PROVIDER).await?;
        }
        tx.commit().await?;
        Ok(liked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SourceKind, TrackSource};

    fn track(ext: &str, id: &str, title: &str, artist: &str, ms: u32, isrc: Option<&str>) -> Track {
        Track {
            id: format!("{ext}:{id}"),
            title: title.into(),
            artists: vec![artist.into()],
            album: None,
            duration_ms: Some(ms),
            artwork: None,
            isrc: isrc.map(Into::into),
            sources: vec![TrackSource { provider: SourceKind::Extension, item_id: format!("{ext}:{id}"), url: String::new(), available: true }],
            available: true,
            added_at: None,
        }
    }

    fn info(name: &str) -> SourceInfo {
        SourceInfo { name: name.into(), icon: None, can_play: true, qualities: Vec::new(), quality: None }
    }

    #[test]
    fn merges_the_same_song_across_extensions() {
        let results = vec![
            ExtensionResults {
                extension_id: "a".into(),
                extension_name: "A".into(),
                tracks: vec![
                    track("a", "1", "Song One", "Artist", 200_000, None),
                    track("a", "2", "Other", "Someone", 100_000, Some("USABC2400001")),
                ],
                error: None,
            },
            ExtensionResults {
                extension_id: "b".into(),
                extension_name: "B".into(),
                tracks: vec![
                    track("b", "9", "Song One (Remastered 2011)", "Artist", 201_000, None),
                    track("b", "8", "Totally different", "Nobody", 300_000, Some("USABC2400001")),
                ],
                error: None,
            },
        ];
        let infos: HashMap<String, SourceInfo> = [("a".to_owned(), info("A")), ("b".to_owned(), info("B"))].into();
        let songs = merge(&results, &infos);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].key, "a:1");
        assert_eq!(songs[0].sources.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["a:1", "b:9"]);
        assert_eq!(songs[1].sources.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["a:2", "b:8"], "same ISRC");
        assert_eq!(songs[0].sources[1].extension_name, "B");

        // A turned-off extension's results are dropped.
        let only_a: HashMap<String, SourceInfo> = [("a".to_owned(), info("A"))].into();
        assert!(merge(&results, &only_a).iter().all(|s| s.sources.iter().all(|x| x.extension_id == "a")));
    }
}
