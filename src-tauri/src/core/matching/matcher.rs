//! Matching (spec §8.2). An in-memory index of existing tracks answers "is this new item the
//! same song as one we already have?". Build it once per job (`MatchIndex::load`) and add new
//! rows as they are created, so items inside one job also merge with each other.

use std::collections::{HashMap, HashSet};

use sqlx::SqliteConnection;
use strsim::jaro_winkler;

use super::normalize;
use crate::error::AppResult;

/// Merge thresholds (§8.2 step 3).
pub const MERGE_SCORE: f64 = 0.90;
pub const MERGE_SCORE_WITH_EXACT_DURATION: f64 = 0.80;
/// Artists this similar are candidates even when the normalised names differ.
pub const ARTIST_CANDIDATE_SIMILARITY: f64 = 0.9;

/// What the matcher compares for one song.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchKey {
    pub title: String,
    /// Normalised primary artist.
    pub artist: String,
    pub duration_ms: Option<u32>,
    /// Uppercase, no dashes.
    pub isrc: Option<String>,
    versions: Vec<&'static str>,
    /// Numbers in the title ("No. 5", "Part 2"); titles that disagree never merge.
    numbers: Vec<String>,
}

impl MatchKey {
    pub fn new(title: &str, artists: &[String], duration_ms: Option<u32>, isrc: Option<&str>) -> Self {
        let credit = artists.first().map(String::as_str).unwrap_or_default();
        let primary = normalize::split_artists(credit).into_iter().next().unwrap_or_default();
        let t = normalize::title(title, Some(credit));
        let versions = normalize::version_words(&t.norm);
        let numbers = t
            .norm
            .split_whitespace()
            .filter(|w| w.chars().all(char::is_numeric))
            .map(str::to_owned)
            .collect();
        Self {
            title: t.norm,
            artist: normalize::artist(&primary),
            duration_ms,
            isrc: isrc.and_then(clean_isrc),
            versions,
            numbers,
        }
    }
}

fn clean_isrc(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_uppercase();
    (s.len() == 12).then_some(s)
}

/// §8.2 duration score: 1 if |Δ| ≤ 3 s, 0.5 if ≤ 8 s, else 0 (also 0 when unknown).
pub fn duration_score(a: Option<u32>, b: Option<u32>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => match a.abs_diff(b) {
            0..=3_000 => 1.0,
            3_001..=8_000 => 0.5,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// Score of a candidate pair, or `None` when rule 4 forbids merging. Beyond §8.2, titles with
/// different numbers ("Symphony No. 5" / "No. 7") never merge either: their similarity is
/// near 1 although they are different songs.
pub fn score(a: &MatchKey, b: &MatchKey) -> Option<(f64, f64)> {
    if a.versions != b.versions || a.numbers != b.numbers || a.title.is_empty() || b.title.is_empty() {
        return None;
    }
    let dur = duration_score(a.duration_ms, b.duration_ms);
    let s = 0.6 * jaro_winkler(&a.title, &b.title) + 0.25 * jaro_winkler(&a.artist, &b.artist) + 0.15 * dur;
    Some((s, dur))
}

/// Whether a score merges (§8.2 step 3).
pub fn merges(score: f64, duration_score: f64) -> bool {
    score >= MERGE_SCORE || (score >= MERGE_SCORE_WITH_EXACT_DURATION && duration_score >= 1.0)
}

/// `id, title, artists_json, duration_ms, isrc` of a stored track.
type TrackRow = (String, String, String, Option<i64>, Option<String>);

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub track_id: String,
    pub confidence: f64,
}

#[derive(Debug, Default)]
pub struct MatchIndex {
    entries: Vec<(String, MatchKey)>,
    by_isrc: HashMap<String, usize>,
    by_artist: HashMap<String, Vec<usize>>,
    /// Tracks deleted during the job; never returned again.
    removed: HashSet<String>,
}

impl MatchIndex {
    /// Loads every track in the database.
    pub async fn load(conn: &mut SqliteConnection) -> AppResult<Self> {
        let rows: Vec<TrackRow> =
            sqlx::query_as("SELECT id, title, artists_json, duration_ms, isrc FROM tracks")
                .fetch_all(&mut *conn)
                .await?;
        let mut index = Self::default();
        for (id, title, artists_json, duration, isrc) in rows {
            let artists: Vec<String> = serde_json::from_str(&artists_json).unwrap_or_default();
            let duration = duration.and_then(|d| u32::try_from(d).ok());
            index.insert(id, MatchKey::new(&title, &artists, duration, isrc.as_deref()));
        }
        Ok(index)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn insert(&mut self, track_id: String, key: MatchKey) {
        let i = self.entries.len();
        if let Some(isrc) = &key.isrc {
            // A newer row wins, so a removed track's ISRC can point at its replacement.
            self.by_isrc.insert(isrc.clone(), i);
        }
        self.by_artist.entry(key.artist.clone()).or_default().push(i);
        self.entries.push((track_id, key));
    }

    /// Forgets a track that was deleted.
    pub fn remove(&mut self, track_id: &str) {
        self.removed.insert(track_id.to_owned());
    }

    fn live(&self, i: usize) -> Option<&(String, MatchKey)> {
        self.entries.get(i).filter(|(id, _)| !self.removed.contains(id))
    }

    /// The best existing track for `key`, if one merges.
    pub fn find(&self, key: &MatchKey) -> Option<Match> {
        let by_isrc = key.isrc.as_ref().and_then(|isrc| self.by_isrc.get(isrc)).and_then(|i| self.live(*i));
        if let Some((id, _)) = by_isrc {
            return Some(Match { track_id: id.clone(), confidence: 1.0 });
        }
        // Songs without an artist never merge by name (too many false positives).
        if key.artist.is_empty() {
            return None;
        }
        let candidates = self.by_artist.iter().filter(|(artist, _)| {
            **artist == key.artist || jaro_winkler(artist, &key.artist) >= ARTIST_CANDIDATE_SIMILARITY
        });
        let mut best: Option<(f64, usize)> = None;
        for i in candidates.flat_map(|(_, ids)| ids.iter().copied()) {
            let Some((_, other)) = self.live(i) else { continue };
            let Some((s, dur)) = score(key, other) else { continue };
            if merges(s, dur) && best.is_none_or(|(b, _)| s > b) {
                best = Some((s, i));
            }
        }
        best.and_then(|(s, i)| {
            self.entries.get(i).map(|(id, _)| Match { track_id: id.clone(), confidence: (s * 1000.0).round() / 1000.0 })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(title: &str, artist: &str, secs: u32) -> MatchKey {
        MatchKey::new(title, &[artist.to_owned()], Some(secs * 1000), None)
    }

    fn index(items: &[(&str, MatchKey)]) -> MatchIndex {
        let mut idx = MatchIndex::default();
        for (id, k) in items {
            idx.insert((*id).to_owned(), k.clone());
        }
        idx
    }

    #[test]
    fn duration_score_steps() {
        assert_eq!(duration_score(Some(200_000), Some(203_000)), 1.0);
        assert_eq!(duration_score(Some(200_000), Some(207_500)), 0.5);
        assert_eq!(duration_score(Some(200_000), Some(209_000)), 0.0);
        assert_eq!(duration_score(None, Some(1)), 0.0);
    }

    #[test]
    fn isrc_matches_with_full_confidence() {
        let a = MatchKey::new("Totally Different", &["X".into()], None, Some("us-um7-14-00001"));
        let idx = index(&[("t1", MatchKey::new("Song", &["Y".into()], None, Some("USUM71400001")))]);
        assert_eq!(idx.find(&a), Some(Match { track_id: "t1".into(), confidence: 1.0 }));
    }

    #[test]
    fn same_song_from_another_source_merges() {
        let idx = index(&[("sp", key("Blinding Lights", "The Weeknd", 200))]);
        let m = idx.find(&key("Blinding Lights (Official Audio)", "The Weeknd - Topic", 201)).unwrap();
        assert_eq!(m.track_id, "sp");
        assert!(m.confidence > 0.99);
        // Featured artist written in the title vs the credit.
        let idx = index(&[("sp", MatchKey::new("Song", &["Alice".into(), "Bob".into()], Some(180_000), None))]);
        assert!(idx.find(&key("Song (feat. Bob)", "Alice", 181)).is_some());
        // Similar artist spelling is still a candidate.
        let idx = index(&[("sp", key("Hello", "Beyoncé", 200))]);
        assert!(idx.find(&key("Hello", "Beyonce", 200)).is_some());
    }

    #[test]
    fn different_songs_or_versions_do_not_merge() {
        let idx = index(&[("sp", key("Hello", "Adele", 295))]);
        assert!(idx.find(&key("Hello (Live)", "Adele", 296)).is_none(), "live vs studio");
        assert!(idx.find(&key("Hello - Remix", "Adele", 295)).is_none(), "remix vs original");
        assert!(idx.find(&key("Someone Like You", "Adele", 285)).is_none(), "other title");
        assert!(idx.find(&key("Hello", "Lionel Richie", 295)).is_none(), "other artist");
        assert!(idx.find(&key("Hello", "", 295)).is_none(), "no artist");
        // Same words but far apart in length: duration score 0 and title similarity < 1.
        assert!(idx.find(&key("Hello Again", "Adele", 400)).is_none());
        // Numbered parts of a work are different songs.
        let idx = index(&[("s5", key("Symphony No. 5", "Beethoven", 1_800))]);
        assert!(idx.find(&key("Symphony No. 7", "Beethoven", 1_800)).is_none());
        assert!(idx.find(&key("Symphony No. 5", "Beethoven", 1_801)).is_some());
    }

    #[test]
    fn mid_scores_need_exact_duration() {
        // A dropped leading "The": title similarity ≈ 0.78, so the score lands in 0.80–0.90.
        let base = key("The Night We Met", "Lord Huron", 208);
        let idx = index(&[("sp", base.clone())]);
        let near = key("Night We Met", "Lord Huron", 209);
        let (s, d) = score(&near, &base).unwrap();
        assert!((0.80..0.90).contains(&s), "score {s}");
        assert_eq!(d, 1.0);
        assert!(idx.find(&near).is_some());
        let far = key("Night We Met", "Lord Huron", 214);
        let (s, d) = score(&far, &base).unwrap();
        assert!(s < 0.90 && d < 1.0, "score {s}");
        assert!(idx.find(&far).is_none());
    }

    #[test]
    fn khmer_and_korean_titles_match_themselves_only() {
        let idx = index(&[("km", key("ស្រលាញ់អូនណាស់", "ព្រាប សុវត្ថិ", 240)), ("kr", key("봄날", "BTS", 274))]);
        assert_eq!(idx.find(&key("ស្រលាញ់អូនណាស់ (Official MV)", "ព្រាប សុវត្ថិ", 241)).unwrap().track_id, "km");
        // A translated subtitle in brackets still merges when the duration agrees.
        assert_eq!(idx.find(&key("봄날 (Spring Day)", "BTS", 274)).unwrap().track_id, "kr");
        assert_eq!(idx.find(&key("봄날", "BTS", 275)).unwrap().track_id, "kr");
        assert!(idx.find(&key("피 땀 눈물", "BTS", 217)).is_none());
    }

    #[test]
    fn removed_tracks_are_not_found() {
        let mut idx = index(&[("old", MatchKey::new("Song", &["A".into()], Some(100_000), Some("USUM71400001")))]);
        idx.remove("old");
        assert!(idx.find(&MatchKey::new("Song", &["A".into()], Some(100_000), Some("USUM71400001"))).is_none());
        idx.insert("new".into(), MatchKey::new("Song", &["A".into()], Some(100_000), Some("USUM71400001")));
        assert_eq!(idx.find(&key("Song", "A", 100)).unwrap().track_id, "new");
    }

    #[test]
    fn picks_the_best_candidate() {
        let idx = index(&[("a", key("Yellow Submarine", "The Beatles", 160)), ("b", key("Yellow", "Coldplay", 269))]);
        assert_eq!(idx.find(&key("Yellow", "Coldplay", 268)).unwrap().track_id, "b");
    }
}
