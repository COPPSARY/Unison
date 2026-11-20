//! Types shared with the UI (spec §5), exported to src/lib/bindings.ts by tauri-specta.

use serde::{Deserialize, Serialize};
use specta::Type;

pub use crate::model_player::{
    AudiusSettings, LocalSettings, PlayerSettings, ReplayGainMode, SourceKind, WatchedFolder,
};

/// Build and schema information shown in Settings → About.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub schema_version: i64,
}

/// One stored source of a track as shown in lists (a local file, Audius, an extension).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackSource {
    pub provider: SourceKind,
    /// Local fingerprint, Audius id or `<extension>:<item>`.
    pub item_id: String,
    /// Web URL, or the absolute file path for local files.
    pub url: String,
    /// False when this source can't play (e.g. a local file that was moved or deleted).
    pub available: bool,
}

/// One song, possibly available on several platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Unified id (uuid v7) — or a provider-scoped id before matching.
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork: Option<String>,
    pub isrc: Option<String>,
    pub sources: Vec<TrackSource>,
    pub available: bool,
    /// When it was liked/added (library and playlist views), Unix seconds.
    pub added_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    /// Where it came from (`None` = Unison local playlist).
    pub provider: Option<String>,
    pub name: String,
    pub owner: String,
    pub track_count: u32,
    pub artwork: Option<String>,
    pub editable: bool,
}

/// Offset-paged result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub next_offset: Option<u32>,
}

/// Concrete page of tracks for commands (tauri-specta rc.29 exports generic return types with an
/// unbound `T`, so commands return this instead of `Page<Track>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackPage {
    pub items: Vec<Track>,
    pub total: u32,
    pub next_offset: Option<u32>,
}

impl From<Page<Track>> for TrackPage {
    fn from(p: Page<Track>) -> Self {
        Self { items: p.items, total: p.total, next_offset: p.next_offset }
    }
}

// ---------------------------------------------------------------- Library views

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub provider: String,
    pub item_id: String,
    pub name: String,
    pub artists: Vec<String>,
    pub artwork: Option<String>,
    pub release_date: Option<String>,
    pub added_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub provider: String,
    pub item_id: String,
    pub name: String,
    pub artwork: Option<String>,
}

/// Sort orders for list and grid views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySort {
    DateAdded,
    Title,
    Artist,
}

/// Playlist page header data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub playlist: Playlist,
    /// Total length of the stored items, once loaded.
    pub duration_ms: Option<f64>,
}

/// Count and total duration for a list header.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListStats {
    pub count: u32,
    pub duration_ms: f64,
}

// ---------------------------------------------------------------- Settings

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// Window backdrop (ADR 0002). Applied at launch; live switching arrives in T-71.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum WindowMaterial {
    #[default]
    Mica,
    Acrylic,
    Off,
}

/// User preferences. Missing keys fall back to defaults, so older databases keep working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Glass frost 0..=1 (more blur + tint).
    pub glass_frost: f32,
    pub window_material: WindowMaterial,
    /// Sidebar sections the user collapsed (by section key).
    pub sidebar_collapsed: Vec<String>,
    /// Universal Player (addendum §5): stored under the key `player`.
    pub player: PlayerSettings,
    /// Local files: stored under the key `local`.
    pub local: LocalSettings,
    /// Audius: stored under the key `audius`.
    pub audius: AudiusSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            glass_frost: 0.5,
            window_material: WindowMaterial::Mica,
            sidebar_collapsed: Vec::new(),
            player: PlayerSettings::default(),
            local: LocalSettings::default(),
            audius: AudiusSettings::default(),
        }
    }
}

impl Settings {
    /// Clamps or resets out-of-range values instead of rejecting the whole update.
    pub fn sanitized(mut self) -> Self {
        if !self.glass_frost.is_finite() {
            self.glass_frost = 0.5;
        }
        self.glass_frost = self.glass_frost.clamp(0.0, 1.0);
        self.sidebar_collapsed.sort();
        self.sidebar_collapsed.dedup();
        self.sidebar_collapsed.truncate(16);
        self.player = self.player.sanitized();
        self.local = self.local.sanitized();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_sanitize() {
        let s = Settings {
            glass_frost: 3.0,
            sidebar_collapsed: vec!["b".into(), "a".into(), "b".into()],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.glass_frost, 1.0);
        assert_eq!(s.sidebar_collapsed, vec!["a", "b"]);
        let nan = Settings { glass_frost: f32::NAN, ..Settings::default() }.sanitized();
        assert_eq!(nan.glass_frost, 0.5);
    }

    #[test]
    fn settings_tolerate_missing_fields() {
        let s: Settings = serde_json::from_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(s.theme, Theme::Light);
    }
}
