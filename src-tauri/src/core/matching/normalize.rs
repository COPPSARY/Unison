//! Normalisation (spec §8.1). Non-Latin scripts (Khmer, Korean, Japanese …) are kept as-is;
//! only case, compatibility forms, noise and punctuation are folded.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Bracketed noise removed from titles: `(Official Video)`, `[Lyrics]`, `(Remastered 2011)` …
static TITLE_NOISE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"[(\[]\s*(?:official\s+(?:music\s+)?video|official\s+audio|lyrics?(?:\s+video)?|audio|visuali[sz]er|mv|m/v|hd|4k|remaster(?:ed)?(?:\s+\d{4})?|\d{4}\s+remaster(?:ed)?)\s*[)\]]",
    )
    .ok()
});

/// Spotify-style remaster suffix: `Song - Remastered 2009`, `Song - 2011 Remaster`.
static REMASTER_SUFFIX: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\s+-\s+(?:\d{4}\s+)?remaster(?:ed)?(?:\s+\d{4})?(?:\s+version)?\s*$").ok());

/// `feat. X`, `ft. X`, `featuring X`, optionally in brackets; captures the names.
static FEATURING: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"[(\[]?\s*\b(?:feat\.?|ft\.?|featuring)\s+([^()\[\]]+?)\s*(?:[)\]]|$|\s-\s)").ok()
});

/// Artist list separators: `,` `&` ` x ` `feat.` `ft.` `featuring` `;` `/`.
static ARTIST_SPLIT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\s*(?:,|&|;|\s/\s|\sx\s|\b(?:feat\.?|ft\.?|featuring)\s)\s*").ok());

/// Channel/artist noise: `Artist - Topic`, `ArtistVEVO`, `Artist Official`.
static ARTIST_NOISE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?:\s+-\s+topic|vevo|\bofficial\b)\s*$").ok());

static PUNCTUATION: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"[\p{P}\p{S}]+").ok());

/// Words that mark a different recording; two titles that disagree on these never merge.
static VERSION_WORDS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\b(?:live|remix|acoustic|cover|sped up|slowed)\b").ok());

fn replace_all(re: &LazyLock<Option<Regex>>, text: &str, with: &str) -> String {
    match re.as_ref() {
        Some(re) => re.replace_all(text, with).into_owned(),
        None => text.to_owned(),
    }
}

/// Step 1: Unicode NFKC, lowercase, trim.
pub fn fold(s: &str) -> String {
    s.nfkc().collect::<String>().to_lowercase().trim().to_owned()
}

/// Step 4: drop apostrophes, turn other punctuation/symbols into spaces, collapse whitespace.
pub fn collapse(s: &str) -> String {
    let no_apostrophes: String = s.chars().filter(|c| !matches!(c, '\'' | '’' | '`')).collect();
    let spaced = replace_all(&PUNCTUATION, &no_apostrophes, " ");
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Splits an artist credit into names (§8.1 step 3), keeping the original spelling.
pub fn split_artists(credit: &str) -> Vec<String> {
    let parts: Vec<String> = match ARTIST_SPLIT.as_ref() {
        Some(re) => re.split(credit).map(|p| p.trim().to_owned()).collect(),
        None => vec![credit.trim().to_owned()],
    };
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Normalised artist name (one artist, not a list).
pub fn artist(name: &str) -> String {
    let folded = fold(name);
    collapse(&replace_all(&ARTIST_NOISE, &folded, ""))
}

/// A normalised title plus the featured artists it named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title {
    pub norm: String,
    pub featured: Vec<String>,
}

/// Normalises a title (§8.1 step 2). `artist` is the credited artist (or channel); a leading
/// `"Artist - "` equal to it is removed.
pub fn title(raw: &str, artist_name: Option<&str>) -> Title {
    let mut s = fold(raw);
    s = replace_all(&TITLE_NOISE, &s, " ");
    s = replace_all(&REMASTER_SUFFIX, &s, "");

    let mut featured = Vec::new();
    if let Some(re) = FEATURING.as_ref() {
        for caps in re.captures_iter(&s) {
            if let Some(names) = caps.get(1) {
                featured.extend(split_artists(names.as_str()).iter().map(|n| artist(n)));
            }
        }
        s = re.replace_all(&s, " ").into_owned();
    }

    if let Some(a) = artist_name.map(fold).filter(|a| !a.is_empty()) {
        if let Some(rest) = s.trim_start().strip_prefix(a.as_str()) {
            if let Some(rest) = rest.trim_start().strip_prefix('-') {
                s = rest.to_owned();
            }
        }
    }

    Title { norm: collapse(&s), featured }
}

/// The version words (live, remix …) present in a normalised title, sorted.
pub fn version_words(title_norm: &str) -> Vec<&'static str> {
    let Some(re) = VERSION_WORDS.as_ref() else { return Vec::new() };
    let mut words: Vec<&'static str> = re
        .find_iter(title_norm)
        .filter_map(|m| match m.as_str() {
            "live" => Some("live"),
            "remix" => Some("remix"),
            "acoustic" => Some("acoustic"),
            "cover" => Some("cover"),
            "sped up" => Some("sped up"),
            "slowed" => Some("slowed"),
            _ => None,
        })
        .collect();
    words.sort_unstable();
    words.dedup();
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_width_and_whitespace() {
        assert_eq!(fold("  ＨＥＬＬＯ World "), "hello world");
        assert_eq!(collapse("don't  stop -- (me) now!"), "dont stop me now");
    }

    #[test]
    fn removes_bracketed_noise_and_remaster_suffixes() {
        assert_eq!(title("Song (Official Music Video)", None).norm, "song");
        assert_eq!(title("Song [Lyrics]", None).norm, "song");
        assert_eq!(title("Song (Remastered 2011)", None).norm, "song");
        assert_eq!(title("Song - Remastered 2009", None).norm, "song");
        assert_eq!(title("Song - 2011 Remaster", None).norm, "song");
        assert_eq!(title("Song (Visualizer) [4K]", None).norm, "song");
        // Not noise: real parts of a title stay.
        assert_eq!(title("Song (Live at Wembley)", None).norm, "song live at wembley");
        assert_eq!(title("Audio Love", None).norm, "audio love");
    }

    #[test]
    fn moves_featured_artists_out_of_the_title() {
        let t = title("Song (feat. Alice & Bob)", None);
        assert_eq!(t.norm, "song");
        assert_eq!(t.featured, vec!["alice", "bob"]);
        let t = title("Song ft. Carol", None);
        assert_eq!(t.norm, "song");
        assert_eq!(t.featured, vec!["carol"]);
        let t = title("Song featuring Dan - Radio Edit", None);
        assert_eq!(t.norm, "song radio edit");
        assert_eq!(t.featured, vec!["dan"]);
        // "ft" inside a word is not a credit.
        assert_eq!(title("Left Behind", None).norm, "left behind");
    }

    #[test]
    fn strips_artist_prefix_only_when_it_matches() {
        assert_eq!(title("Adele - Hello", Some("Adele")).norm, "hello");
        assert_eq!(title("Someone - Hello", Some("Adele")).norm, "someone hello");
    }

    #[test]
    fn keeps_non_latin_scripts() {
        assert_eq!(title("ស្រលាញ់អូនណាស់ (Official Video)", None).norm, "ស្រលាញ់អូនណាស់");
        assert_eq!(title("봄날 (Spring Day)", None).norm, "봄날 spring day");
        assert_eq!(artist("방탄소년단"), "방탄소년단");
    }

    #[test]
    fn cleans_and_splits_artists() {
        assert_eq!(artist("Adele - Topic"), "adele");
        assert_eq!(artist("AdeleVEVO"), "adele");
        assert_eq!(artist("Ed Sheeran Official"), "ed sheeran");
        assert_eq!(split_artists("A, B & C x D feat. E"), vec!["A", "B", "C", "D", "E"]);
        assert_eq!(split_artists("Lil Nas X"), vec!["Lil Nas X"]);
        assert_eq!(split_artists("AC/DC"), vec!["AC/DC"]);
        assert_eq!(split_artists("A; B"), vec!["A", "B"]);
    }

    #[test]
    fn finds_version_words() {
        assert_eq!(version_words("song live"), vec!["live"]);
        assert_eq!(version_words("song sped up remix"), vec!["remix", "sped up"]);
        assert!(version_words("alive").is_empty());
        assert!(version_words("discover").is_empty());
    }
}
