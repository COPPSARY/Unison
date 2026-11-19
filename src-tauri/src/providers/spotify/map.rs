//! Spotify objects → provider-neutral inputs for the library repository.

use super::models::{AlbumObj, ArtistObj, Image, SimplePlaylist, TrackObj};
use crate::core::db::library::{AlbumInput, ArtistInput, PlaylistInput, TrackInput};

/// Preferred artwork edge in px: big enough for 180 px tiles at 1.5× scaling.
const ART_TARGET_PX: u32 = 300;

/// Smallest image at least `ART_TARGET_PX` wide, else the largest one.
pub fn pick_image(images: &[Image]) -> Option<String> {
    let width = |i: &Image| i.width.or(i.height).unwrap_or(0);
    images
        .iter()
        .filter(|i| width(i) >= ART_TARGET_PX)
        .min_by_key(|i| width(i))
        .or_else(|| images.iter().max_by_key(|i| width(i)))
        .map(|i| i.url.clone())
}

/// RFC 3339 → Unix seconds.
pub fn parse_time(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|d| d.timestamp())
}

pub fn track_url(id: &str) -> String {
    format!("https://open.spotify.com/track/{id}")
}

/// Maps a track; `None` when it has neither an id nor a uri (nothing to key it by).
pub fn track(t: &TrackObj) -> Option<TrackInput> {
    // Local files have no id; key them by uri and mark them unavailable (spec §6).
    let item_id = t.id.clone().or_else(|| t.uri.clone())?;
    let url = match (&t.external_urls.spotify, &t.id) {
        (Some(url), _) => url.clone(),
        (None, Some(id)) => track_url(id),
        (None, None) => String::new(),
    };
    Some(TrackInput {
        item_id,
        url,
        title: t.name.clone(),
        artists: t.artists.iter().map(|a| a.name.clone()).collect(),
        album: t.album.as_ref().map(|a| a.name.clone()),
        duration_ms: t.duration_ms,
        isrc: t
            .external_ids
            .as_ref()
            .and_then(|e| e.isrc.clone())
            .map(|i| i.to_ascii_uppercase()),
        artwork: t.album.as_ref().and_then(|a| pick_image(&a.images)),
        available: !t.is_local && t.is_playable.unwrap_or(true),
    })
}

pub fn album(a: &AlbumObj) -> AlbumInput {
    AlbumInput {
        item_id: a.id.clone(),
        name: a.name.clone(),
        artists: a.artists.iter().map(|x| x.name.clone()).collect(),
        artwork: pick_image(&a.images),
        release_date: a.release_date.clone(),
    }
}

pub fn artist(a: &ArtistObj) -> ArtistInput {
    ArtistInput {
        item_id: a.id.clone(),
        name: a.name.clone(),
        artwork: a.images.as_deref().and_then(pick_image),
    }
}

/// `me` is the connected user's Spotify id: owned or collaborative playlists are editable and
/// are the only ones whose items Spotify returns (Feb 2026).
pub fn playlist(p: &SimplePlaylist, me: &str) -> PlaylistInput {
    PlaylistInput {
        item_id: p.id.clone(),
        name: p.name.clone(),
        owner: p.owner.display_name.clone().unwrap_or_else(|| p.owner.id.clone()),
        owner_id: p.owner.id.clone(),
        editable: p.owner.id == me || p.collaborative,
        artwork: p.images.as_deref().and_then(pick_image),
        snapshot: p.snapshot_id.clone(),
        track_count: p.items.as_ref().map(|i| i.total).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32) -> Image {
        Image { url: format!("u{w}"), width: Some(w), height: Some(w) }
    }

    #[test]
    fn picks_smallest_image_over_target_else_largest() {
        assert_eq!(pick_image(&[img(640), img(300), img(64)]).as_deref(), Some("u300"));
        assert_eq!(pick_image(&[img(64), img(200)]).as_deref(), Some("u200"));
        assert_eq!(pick_image(&[]), None);
    }

    #[test]
    fn parses_spotify_timestamps() {
        assert_eq!(parse_time(Some("2024-01-02T03:04:05Z")), Some(1_704_164_645));
        assert_eq!(parse_time(Some("nope")), None);
    }

    #[test]
    fn local_files_are_unavailable_and_keyed_by_uri() {
        let t: TrackObj = serde_json::from_str(
            r#"{"id":null,"uri":"spotify:local:a:b:c:1","name":"Demo","is_local":true}"#,
        )
        .unwrap();
        let m = track(&t).unwrap();
        assert_eq!(m.item_id, "spotify:local:a:b:c:1");
        assert!(!m.available);
    }

    #[test]
    fn maps_full_track() {
        let t: TrackObj = serde_json::from_str(
            r#"{"id":"t1","name":"Song","artists":[{"id":"a","name":"A"},{"id":"b","name":"B"}],
                "album":{"id":"al","name":"Album","images":[{"url":"x","width":300,"height":300}]},
                "duration_ms":222000,"external_ids":{"isrc":"usabc1234567"}}"#,
        )
        .unwrap();
        let m = track(&t).unwrap();
        assert_eq!(m.url, "https://open.spotify.com/track/t1");
        assert_eq!(m.artists, vec!["A", "B"]);
        assert_eq!(m.isrc.as_deref(), Some("USABC1234567"));
        assert_eq!(m.artwork.as_deref(), Some("x"));
        assert!(m.available);
    }
}
