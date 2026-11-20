//! Universal Player settings (addendum §5). Stored in the settings table as JSON under the keys
//! `player`, `local` and `audius`; exported to the UI through tauri-specta.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Where a track can be played from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Local,
    Audius,
    /// An installed extension (EXT-01/02); streamed by NativeEngine.
    Extension,
}

impl SourceKind {
    /// Default "Play from" order: Local → Audius → extensions.
    pub const DEFAULT_PRIORITY: [SourceKind; 3] = [SourceKind::Local, SourceKind::Audius, SourceKind::Extension];

    /// The `track_sources.provider` string.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Local => "local",
            SourceKind::Audius => "audius",
            SourceKind::Extension => "extension",
        }
    }

    pub fn parse(value: &str) -> Option<SourceKind> {
        match value {
            "local" => Some(SourceKind::Local),
            "audius" => Some(SourceKind::Audius),
            "extension" => Some(SourceKind::Extension),
            _ => None,
        }
    }
}

/// Reads a stored priority list, dropping sources Unison no longer has (Spotify, YouTube).
fn known_sources<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<SourceKind>, D::Error> {
    let raw: Vec<serde_json::Value> = Deserialize::deserialize(d)?;
    Ok(raw.iter().filter_map(|v| v.as_str()).filter_map(SourceKind::parse).collect())
}

/// Volume leveling (NativeEngine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReplayGainMode {
    Off,
    #[default]
    Track,
    Album,
}

pub const MAX_CROSSFADE_MS: u32 = 12_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerSettings {
    #[serde(deserialize_with = "known_sources")]
    #[specta(type = Vec<SourceKind>)]
    pub source_priority: Vec<SourceKind>,
    pub auto_fallback: bool,
    pub fallback_on_error_midtrack: bool,
    /// NativeEngine output device; `None` = system default.
    pub output_device: Option<String>,
    pub gapless: bool,
    /// 0–12,000 ms (NativeEngine).
    pub crossfade_ms: u32,
    pub replaygain: ReplayGainMode,
    /// 0–100, remembered.
    pub volume: u8,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            source_priority: SourceKind::DEFAULT_PRIORITY.to_vec(),
            auto_fallback: true,
            fallback_on_error_midtrack: true,
            output_device: None,
            gapless: true,
            crossfade_ms: 0,
            replaygain: ReplayGainMode::Track,
            volume: 80,
        }
    }
}

impl PlayerSettings {
    pub fn sanitized(mut self) -> Self {
        // Keep the user's order, drop duplicates, append anything missing in default order.
        let mut priority = Vec::with_capacity(4);
        for kind in self.source_priority.iter().chain(SourceKind::DEFAULT_PRIORITY.iter()) {
            if !priority.contains(kind) {
                priority.push(*kind);
            }
        }
        self.source_priority = priority;
        self.crossfade_ms = self.crossfade_ms.min(MAX_CROSSFADE_MS);
        self.volume = self.volume.min(100);
        self.output_device = clean_optional(self.output_device);
        self
    }
}

/// Audio file types imported by default (addendum §4).
pub const DEFAULT_EXTENSIONS: [&str; 7] = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalSettings {
    pub watched_folders: Vec<String>,
    /// Copy dropped files into `copy_target` instead of referencing them in place.
    pub copy_on_drop: bool,
    /// `None` = %USERPROFILE%/Music/Unison.
    pub copy_target: Option<String>,
    /// Lowercase, without dots.
    pub extensions: Vec<String>,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            watched_folders: Vec::new(),
            copy_on_drop: false,
            copy_target: None,
            extensions: DEFAULT_EXTENSIONS.iter().map(|e| (*e).to_owned()).collect(),
        }
    }
}

impl LocalSettings {
    pub fn sanitized(mut self) -> Self {
        let mut exts: Vec<String> = self
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
            .collect();
        exts.dedup();
        let mut seen = std::collections::HashSet::new();
        exts.retain(|e| seen.insert(e.clone()));
        self.extensions = exts;
        let mut folders: Vec<String> = self
            .watched_folders
            .iter()
            .map(|f| f.trim().to_owned())
            .filter(|f| !f.is_empty())
            .collect();
        let mut seen = std::collections::HashSet::new();
        folders.retain(|f| seen.insert(f.to_lowercase()));
        self.watched_folders = folders;
        self.copy_target = clean_optional(self.copy_target);
        self
    }
}

/// A folder Unison watches for new, changed and deleted music (Settings → Local files).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WatchedFolder {
    pub path: String,
    /// Last full scan, Unix seconds.
    pub last_scan_at: Option<i64>,
    /// False while the folder can't be found (e.g. an unplugged drive).
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct AudiusSettings {
    /// Effective only once an API key is saved (keychain `audius.api_key`).
    pub enabled: bool,
    pub use_as_fallback: bool,
    pub show_in_search: bool,
    pub show_trending_row: bool,
}

impl Default for AudiusSettings {
    fn default() -> Self {
        Self { enabled: true, use_as_fallback: true, show_in_search: true, show_trending_row: false }
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_addendum() {
        let p = PlayerSettings::default();
        assert_eq!(p.source_priority, SourceKind::DEFAULT_PRIORITY.to_vec());
        assert!(p.auto_fallback && p.fallback_on_error_midtrack && p.gapless);
        assert_eq!((p.replaygain, p.crossfade_ms), (ReplayGainMode::Track, 0));
        let l = LocalSettings::default();
        assert!(!l.copy_on_drop);
        assert_eq!(l.extensions.len(), 7);
        let a = AudiusSettings::default();
        assert!(a.enabled && a.use_as_fallback && a.show_in_search && !a.show_trending_row);
    }

    #[test]
    fn priority_is_deduped_and_completed() {
        let p = PlayerSettings {
            source_priority: vec![SourceKind::Extension, SourceKind::Extension, SourceKind::Local],
            ..PlayerSettings::default()
        }
        .sanitized();
        assert_eq!(p.source_priority, vec![SourceKind::Extension, SourceKind::Local, SourceKind::Audius]);
    }

    #[test]
    fn stored_priorities_drop_removed_sources() {
        let p: PlayerSettings =
            serde_json::from_str(r#"{"sourcePriority":["youtube","extension","spotify","local"],"volume":40}"#).unwrap();
        assert_eq!(p.source_priority, vec![SourceKind::Extension, SourceKind::Local]);
        assert_eq!(p.volume, 40, "other player settings survive");
    }

    #[test]
    fn clamps_and_cleans() {
        let p = PlayerSettings {
            crossfade_ms: 99_999,
            volume: 250,
            output_device: Some("   ".into()),
            ..PlayerSettings::default()
        }
        .sanitized();
        assert_eq!((p.crossfade_ms, p.volume), (MAX_CROSSFADE_MS, 100));
        assert_eq!(p.output_device, None);

        let l = LocalSettings {
            extensions: vec![".MP3".into(), "flac".into(), "mp3".into(), "".into(), "m 4a".into()],
            watched_folders: vec![r"C:\Music".into(), r"c:\music".into(), "  ".into()],
            ..LocalSettings::default()
        }
        .sanitized();
        assert_eq!(l.extensions, vec!["mp3", "flac"]);
        assert_eq!(l.watched_folders, vec![r"C:\Music"]);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let p: PlayerSettings = serde_json::from_str(r#"{"volume":40}"#).unwrap();
        assert_eq!(p.volume, 40);
        assert!(p.gapless);
        let json = serde_json::to_value(PlayerSettings::default()).unwrap();
        assert_eq!(json["sourcePriority"], serde_json::json!(["local", "audius", "extension"]));
    }
}
