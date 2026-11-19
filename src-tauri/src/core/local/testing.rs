//! Test helpers: tiny distinct WAV files and ID3 tags written with lofty.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};

/// Writes a mono 16-bit 44.1 kHz WAV of `secs` seconds. `seed` makes the samples (and so the
/// fingerprint) unique per file.
pub fn write_wav(path: &Path, secs: f32, seed: u32) {
    let rate = 44_100u32;
    let samples = (secs * rate as f32) as u32;
    let data_len = samples * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..samples {
        let v = ((i.wrapping_mul(31).wrapping_add(seed.wrapping_mul(7919))) % 2000) as i16 - 1000;
        out.extend_from_slice(&v.to_le_bytes());
    }
    // The seed also goes into the first samples so even short files differ in the head.
    out[44..48].copy_from_slice(&seed.to_le_bytes());
    std::fs::write(path, out).unwrap();
}

/// Adds an ID3v2 tag to a file (test files only — Unison never writes tags).
pub fn tag(path: &Path, title: &str, artist: &str, isrc: Option<&str>, cover: Option<Vec<u8>>) {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.to_owned());
    tag.set_artist(artist.to_owned());
    tag.set_album("Test Album".to_owned());
    if let Some(isrc) = isrc {
        tag.insert_text(ItemKey::Isrc, isrc.to_owned());
    }
    tag.insert_text(ItemKey::ReplayGainTrackGain, "-6.50 dB".to_owned());
    if let Some(bytes) = cover {
        let mut pic = Picture::unchecked(bytes).mime_type(MimeType::Png).build();
        pic.set_pic_type(PictureType::CoverFront);
        tag.push_picture(pic);
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

/// A small PNG.
pub fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}
