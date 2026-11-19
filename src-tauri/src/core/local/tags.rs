//! Reads what Unison needs from an audio file (addendum §4 steps 2–3): tags via lofty, falling
//! back to the file name. Files are opened read-only; tags are never written.

use std::path::Path;
use std::sync::LazyLock;

use lofty::file::FileType;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use regex::Regex;

use super::IssueKind;
use crate::core::matching::normalize::split_artists;

/// Everything stored for one file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileMeta {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub isrc: Option<String>,
    pub replaygain_track: Option<f64>,
    pub replaygain_album: Option<f64>,
    /// `mp3`, `flac`, `wav`, `vorbis`, `mp4`, `aac`.
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    /// kbps.
    pub bitrate: Option<u32>,
    /// Embedded cover (front cover preferred), raw bytes.
    pub cover: Option<Vec<u8>>,
}

impl FileMeta {
    /// Short quality label for `track_sources.quality`, e.g. `mp3-320`, `flac`.
    pub fn quality(&self) -> Option<String> {
        let codec = self.codec.as_deref()?;
        Some(match (codec, self.bitrate) {
            ("flac" | "wav", _) => codec.to_owned(),
            (_, Some(kbps)) if kbps > 0 => format!("{codec}-{kbps}"),
            _ => codec.to_owned(),
        })
    }
}

/// Leading track numbers in file names: `01 - `, `07. `, `3_`.
static TRACK_NO_PREFIX: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^\d{1,3}\s*[.\-_)]\s+").ok());

/// Title and artists from a file name: `Artist - Title.mp3`, else the whole name is the title.
pub fn from_file_name(path: &Path) -> (String, Vec<String>) {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = stem.replace('_', " ");
    let stem = match TRACK_NO_PREFIX.as_ref() {
        Some(re) => re.replace(stem.trim(), "").into_owned(),
        None => stem,
    };
    let stem = stem.trim();
    match stem.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (title.trim().to_owned(), split_artists(artist))
        }
        _ => (stem.to_owned(), Vec::new()),
    }
}

/// `-6.54 dB` → -6.54.
pub fn parse_gain(raw: &str) -> Option<f64> {
    let number: String = raw
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'))
        .collect();
    number.parse::<f64>().ok().filter(|g| g.is_finite() && g.abs() < 60.0)
}

fn codec_of(file_type: FileType) -> Result<&'static str, IssueKind> {
    match file_type {
        FileType::Mpeg => Ok("mp3"),
        FileType::Flac => Ok("flac"),
        FileType::Wav => Ok("wav"),
        FileType::Vorbis => Ok("vorbis"),
        FileType::Mp4 => Ok("mp4"),
        FileType::Aac => Ok("aac"),
        // No decoder in this build (opus, speex, aiff, ape …): skipped with a reason.
        _ => Err(IssueKind::UnsupportedFormat),
    }
}

fn non_empty(s: Option<impl AsRef<str>>) -> Option<String> {
    s.map(|s| s.as_ref().trim().to_owned()).filter(|s| !s.is_empty())
}

/// Reads tags and properties. Missing title → file name (`Artist - Title`).
pub fn read(path: &Path) -> Result<FileMeta, IssueKind> {
    let probe = Probe::open(path)
        .map_err(|_| IssueKind::Unreadable)?
        .guess_file_type()
        .map_err(|_| IssueKind::Unreadable)?;
    let file_type = probe.file_type().ok_or(IssueKind::UnsupportedFormat)?;
    let codec = codec_of(file_type)?;
    let tagged = probe.read().map_err(|_| IssueKind::Unreadable)?;

    let props = tagged.properties();
    let duration_ms = u32::try_from(props.duration().as_millis()).ok().filter(|d| *d > 0);
    let mut meta = FileMeta {
        duration_ms,
        codec: Some(codec.to_owned()),
        sample_rate: props.sample_rate(),
        bitrate: props.audio_bitrate().or(props.overall_bitrate()),
        ..FileMeta::default()
    };

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    if let Some(tag) = tag {
        meta.title = non_empty(tag.title()).unwrap_or_default();
        let mut artists: Vec<String> = tag
            .get_strings(ItemKey::TrackArtists)
            .flat_map(split_artists)
            .collect();
        if artists.is_empty() {
            artists = tag.get_strings(ItemKey::TrackArtist).flat_map(split_artists).collect();
        }
        if artists.is_empty() {
            artists = non_empty(tag.get_string(ItemKey::AlbumArtist)).map(|a| split_artists(&a)).unwrap_or_default();
        }
        let mut seen = std::collections::HashSet::new();
        artists.retain(|a| seen.insert(a.to_lowercase()));
        meta.artists = artists;
        meta.album = non_empty(tag.album());
        meta.isrc = non_empty(tag.get_string(ItemKey::Isrc));
        meta.replaygain_track = tag.get_string(ItemKey::ReplayGainTrackGain).and_then(parse_gain);
        meta.replaygain_album = tag.get_string(ItemKey::ReplayGainAlbumGain).and_then(parse_gain);
        let pictures = tag.pictures();
        meta.cover = pictures
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pictures.first())
            .map(|p| p.data().to_vec());
    }

    if meta.title.is_empty() {
        let (title, artists) = from_file_name(path);
        meta.title = title;
        if meta.artists.is_empty() {
            meta.artists = artists;
        }
    } else if meta.artists.is_empty() {
        meta.artists = from_file_name(path).1;
    }
    if meta.title.is_empty() {
        return Err(IssueKind::Unreadable);
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::local::testing::write_wav;

    #[test]
    fn file_name_fallbacks() {
        let p = |s: &str| from_file_name(Path::new(s));
        assert_eq!(p("Adele - Hello.mp3"), ("Hello".into(), vec!["Adele".into()]));
        assert_eq!(p("01 - Adele - Hello.mp3"), ("Hello".into(), vec!["Adele".into()]));
        assert_eq!(p("07. Hello.flac"), ("Hello".into(), vec![]));
        assert_eq!(p("Alice & Bob - Duet.mp3"), ("Duet".into(), vec!["Alice".into(), "Bob".into()]));
        assert_eq!(p("1979.mp3"), ("1979".into(), vec![]));
        assert_eq!(p("99 Problems.mp3"), ("99 Problems".into(), vec![]));
        assert_eq!(p("my_song_name.ogg"), ("my song name".into(), vec![]));
        assert_eq!(p("ស្រលាញ់ - បទ.mp3"), ("បទ".into(), vec!["ស្រលាញ់".into()]));
    }

    #[test]
    fn replaygain_values() {
        assert_eq!(parse_gain("-6.54 dB"), Some(-6.54));
        assert_eq!(parse_gain("+1.2 dB"), Some(1.2));
        assert_eq!(parse_gain("loud"), None);
        assert_eq!(parse_gain("-900 dB"), None);
    }

    #[test]
    fn untagged_wav_uses_the_file_name_and_reads_properties() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Some Artist - Some Song.wav");
        write_wav(&path, 1.5, 1);
        let meta = read(&path).unwrap();
        assert_eq!(meta.title, "Some Song");
        assert_eq!(meta.artists, vec!["Some Artist"]);
        assert_eq!(meta.codec.as_deref(), Some("wav"));
        assert_eq!(meta.sample_rate, Some(44_100));
        assert!((1_450..=1_550).contains(&meta.duration_ms.unwrap()), "{:?}", meta.duration_ms);
        assert_eq!(meta.quality().as_deref(), Some("wav"));
    }

    #[test]
    fn unreadable_and_unsupported_files() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.mp3");
        std::fs::write(&junk, b"definitely not audio").unwrap();
        assert!(read(&junk).is_err());
        let opus = dir.path().join("x.opus");
        // A minimal Ogg page carrying an Opus header: lofty identifies it as Opus.
        let mut ogg = b"OggS".to_vec();
        ogg.extend_from_slice(&[0; 24]);
        ogg.extend_from_slice(b"OpusHead");
        std::fs::write(&opus, &ogg).unwrap();
        assert!(read(&opus).is_err());
    }

    #[test]
    fn quality_labels() {
        let m = |codec: &str, br: Option<u32>| FileMeta { codec: Some(codec.into()), bitrate: br, ..FileMeta::default() };
        assert_eq!(m("mp3", Some(320)).quality().as_deref(), Some("mp3-320"));
        assert_eq!(m("flac", Some(900)).quality().as_deref(), Some("flac"));
        assert_eq!(m("vorbis", None).quality().as_deref(), Some("vorbis"));
    }
}
