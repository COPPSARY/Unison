//! Spotify Web API response shapes Unison reads (Feb 2026 Development Mode). Every optional
//! or recently changed field is `Option`/defaulted so a removed field never breaks a sync.

use serde::Deserialize;

use super::client::Paging;

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExternalUrls {
    pub spotify: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExternalIds {
    pub isrc: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistRef {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumRef {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
}

/// Full track object (saved tracks, top tracks, recently played).
#[derive(Debug, Clone, Deserialize)]
pub struct TrackObj {
    /// Null for local files.
    pub id: Option<String>,
    pub uri: Option<String>,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    pub album: Option<AlbumRef>,
    pub duration_ms: Option<u32>,
    pub external_ids: Option<ExternalIds>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
    #[serde(default)]
    pub is_local: bool,
    pub is_playable: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavedTrack {
    pub added_at: Option<String>,
    /// Null when the track was removed from Spotify (skipped, spec §6).
    pub track: Option<TrackObj>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumObj {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artists: Vec<ArtistRef>,
    #[serde(default)]
    pub images: Vec<Image>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SavedAlbum {
    pub added_at: Option<String>,
    pub album: AlbumObj,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Owner {
    pub id: String,
    pub display_name: Option<String>,
}

/// `{ "total": n }` — playlist item count. Feb 2026 renamed `tracks` → `items`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ItemCount {
    #[serde(default)]
    pub total: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimplePlaylist {
    pub id: String,
    pub name: String,
    pub owner: Owner,
    #[serde(default)]
    pub collaborative: bool,
    /// Can be `null` for playlists without artwork.
    #[serde(default)]
    pub images: Option<Vec<Image>>,
    pub snapshot_id: Option<String>,
    #[serde(default, alias = "tracks")]
    pub items: Option<ItemCount>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtistObj {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub images: Option<Vec<Image>>,
}

/// `GET /me/following?type=artist` wraps a cursor page in `artists`.
#[derive(Debug, Clone, Deserialize)]
pub struct FollowedArtists {
    pub artists: Paging<ArtistObj>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayHistory {
    pub track: Option<TrackObj>,
    pub played_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_accepts_items_or_legacy_tracks_and_null_images() {
        let new: SimplePlaylist = serde_json::from_str(
            r#"{"id":"p","name":"P","owner":{"id":"me"},"images":null,"snapshot_id":"s","items":{"href":"x","total":12}}"#,
        )
        .unwrap();
        assert_eq!(new.items.unwrap().total, 12);
        assert!(new.images.is_none());
        let old: SimplePlaylist = serde_json::from_str(
            r#"{"id":"p","name":"P","owner":{"id":"me"},"tracks":{"total":3}}"#,
        )
        .unwrap();
        assert_eq!(old.items.unwrap().total, 3);
    }

    #[test]
    fn saved_track_tolerates_null_track_and_missing_fields() {
        let s: SavedTrack = serde_json::from_str(r#"{"added_at":"2024-01-01T00:00:00Z","track":null}"#).unwrap();
        assert!(s.track.is_none());
        let t: TrackObj = serde_json::from_str(r#"{"id":null,"name":"Demo","is_local":true}"#).unwrap();
        assert!(t.is_local && t.artists.is_empty());
    }
}
