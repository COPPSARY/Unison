//! Types shared with the UI (spec §5), exported to src/lib/bindings.ts by tauri-specta.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Build and schema information shown in Settings → About.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub schema_version: i64,
}

/// A music platform. Stored in SQLite as `'spotify'` / `'youtube'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ProviderId {
    Spotify,
    Youtube,
}

impl ProviderId {
    pub const ALL: [ProviderId; 2] = [ProviderId::Spotify, ProviderId::Youtube];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Spotify => "spotify",
            ProviderId::Youtube => "youtube",
        }
    }

    pub fn parse(value: &str) -> Option<ProviderId> {
        match value {
            "spotify" => Some(ProviderId::Spotify),
            "youtube" => Some(ProviderId::Youtube),
            _ => None,
        }
    }
}

/// Where a unified track can be played.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub provider: ProviderId,
    /// Spotify track id or YouTube video id.
    pub item_id: String,
    pub url: String,
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
    pub sources: Vec<SourceRef>,
    pub available: bool,
    /// When it was liked/added (library and playlist views), Unix seconds.
    pub added_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    /// `None` = Unison local playlist.
    pub provider: Option<ProviderId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    NotConnected,
    Connected,
    NeedsReconnect,
    Syncing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub provider: ProviderId,
    pub status: AccountStatus,
    pub display_name: Option<String>,
    /// Unix seconds.
    pub last_synced_at: Option<i64>,
}

/// Taste signal kinds (spec §8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SignalKind {
    TopShort,
    TopMedium,
    TopLong,
    Recent,
    Liked,
    Play,
    Skip,
    Replay,
    Followed,
}

/// A taste signal about a track or an artist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SignalSubject {
    /// Provider-side track reference (resolved to a unified id on insert).
    Track(SourceRef),
    /// Artist name.
    Artist(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub subject: SignalSubject,
    pub kind: SignalKind,
    /// Unix seconds.
    pub at: i64,
}

// ---------------------------------------------------------------- Library views

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub provider: ProviderId,
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
    pub provider: ProviderId,
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
    /// Background sync interval in minutes: 15, 30 or 60.
    pub sync_interval_min: u32,
    /// Source to play when a song exists on both platforms; `None` = Spotify if connected.
    pub preferred_source: Option<ProviderId>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            glass_frost: 0.5,
            window_material: WindowMaterial::Mica,
            sidebar_collapsed: Vec::new(),
            sync_interval_min: 30,
            preferred_source: None,
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
        if ![15, 30, 60].contains(&self.sync_interval_min) {
            self.sync_interval_min = 30;
        }
        self.sidebar_collapsed.sort();
        self.sidebar_collapsed.dedup();
        self.sidebar_collapsed.truncate(16);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_round_trip() {
        for p in ProviderId::ALL {
            assert_eq!(ProviderId::parse(p.as_str()), Some(p));
        }
        assert_eq!(ProviderId::parse("soundcloud"), None);
    }

    #[test]
    fn settings_sanitize() {
        let s = Settings {
            glass_frost: 3.0,
            sync_interval_min: 7,
            sidebar_collapsed: vec!["b".into(), "a".into(), "b".into()],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.glass_frost, 1.0);
        assert_eq!(s.sync_interval_min, 30);
        assert_eq!(s.sidebar_collapsed, vec!["a", "b"]);
        let nan = Settings { glass_frost: f32::NAN, ..Settings::default() }.sanitized();
        assert_eq!(nan.glass_frost, 0.5);
    }

    #[test]
    fn settings_tolerate_missing_fields() {
        let s: Settings = serde_json::from_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.sync_interval_min, 30);
    }
}
