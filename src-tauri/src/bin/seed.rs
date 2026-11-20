//! `pnpm seed`: builds a demo database for UI and performance checks (spec §10): 10,000 liked
//! songs from two demo extensions (the first 1,200 hand-varied: long names, Khmer and K-pop
//! titles, unavailable items, duplicates across extensions), albums, artists and playlists. Open it with
//! `UNISON_DB=<path> pnpm tauri dev`. Never touches the real database.

use std::path::PathBuf;

use unison_lib::core::db::{self, library as lib};
use unison_lib::core::db::library::{AlbumInput, ArtistInput, LibraryKind, PlaylistInput, TrackInput};

/// `track_sources` provider of extension tracks; item ids are `<extension>:<item>`.
const EXT: &str = "extension";

/// Small deterministic PRNG so every seed run produces the same data.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let len = items.len().max(1) as u64;
        &items[usize::try_from(self.next() % len).unwrap_or(0)]
    }
}

const WORDS: &[&str] = &[
    "Midnight", "Signal", "Golden", "Hour", "Echo", "River", "Neon", "Paper", "Moon", "Ocean", "Static",
    "Velvet", "Summer", "Rain", "Glass", "Heart", "City", "Lights", "Wild", "Fire", "Slow", "Dance",
];
const KHMER: &[&str] = &[
    "ស្រលាញ់អូនជានិច្ច", "ភ្លៀងធ្លាក់ពេលល្ងាច", "ចម្រៀងនៃក្តីស្រលាញ់", "ផ្កាយលើមេឃ", "ទឹកភ្នែកអ្នកនឹករលឹក",
];
const KPOP: &[&str] = &["Butterfly", "Next Level", "Ditto", "Love Dive", "Hype Boy", "Spicy"];
const ARTISTS: &[&str] = &[
    "Aurora Lane", "The Paper Kites", "Neon Harbor", "Sokha Chan", "Preap Sovath", "NewJeans",
    "IVE", "aespa", "Mia Rivers", "Low Tide", "Glass Animals", "Bon Iver", "Kalimba Trio",
    "Midnight Static", "Velvet Moon",
];

fn title(rng: &mut Lcg, i: usize) -> String {
    match i % 25 {
        0 => (*rng.pick(KHMER)).to_owned(),
        1 => (*rng.pick(KPOP)).to_owned(),
        2 => format!(
            "{} {} (feat. {}) [Extended Live Version Recorded at the Royal Albert Hall, London 2019]",
            rng.pick(WORDS),
            rng.pick(WORDS),
            rng.pick(ARTISTS)
        ),
        _ => format!("{} {}", rng.pick(WORDS), rng.pick(WORDS)),
    }
}

fn data_path() -> PathBuf {
    std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("demo.db")
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = data_path();
    for suffix in ["", "-wal", "-shm"] {
        let mut p = path.clone().into_os_string();
        p.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
    let pool = db::open(&path).await?;
    let mut rng = Lcg(42);
    let now: i64 = 1_763_500_000;
    let mut tx = pool.begin().await?;

    let mut album_ids = Vec::new();
    for a in 0..60 {
        let artist = (*rng.pick(ARTISTS)).to_owned();
        let input = AlbumInput {
            item_id: format!("demo-album-{a}"),
            name: format!("{} {}", rng.pick(WORDS), rng.pick(WORDS)),
            artists: vec![artist],
            artwork: None,
            release_date: Some(format!("20{:02}-0{}-1{}", 10 + a % 15, 1 + a % 9, a % 9)),
        };
        let id = lib::upsert_album(&mut tx, EXT, &input).await?;
        lib::upsert_library(&mut tx, EXT, LibraryKind::SavedAlbum, &input.item_id, Some(&id), Some(now - (a as i64) * 86_400)).await?;
        album_ids.push(input.name);
    }

    for (n, name) in ARTISTS.iter().enumerate() {
        let input = ArtistInput { item_id: format!("demo-artist-{n}"), name: (*name).to_owned(), artwork: None };
        lib::upsert_artist(&mut tx, EXT, &input).await?;
        lib::upsert_library(&mut tx, EXT, LibraryKind::FollowedArtist, &input.item_id, None, None).await?;
    }

    for i in 0..10_000usize {
        let ext = if i % 4 == 3 { "demo-radio" } else { "demo-store" };
        let item_id = format!("{ext}:{i:05}");
        let url = format!("https://{ext}.example/track/{i:05}");
        let track = TrackInput {
            item_id: item_id.clone(),
            url,
            title: title(&mut rng, i),
            artists: if i % 11 == 0 {
                vec![(*rng.pick(ARTISTS)).to_owned(), (*rng.pick(ARTISTS)).to_owned()]
            } else {
                vec![(*rng.pick(ARTISTS)).to_owned()]
            },
            album: Some(rng.pick(&album_ids).clone()),
            duration_ms: Some(u32::try_from(120_000 + rng.next() % 240_000).unwrap_or(200_000)),
            isrc: None,
            artwork: None,
            // Unavailable items: removed by the extension's service.
            available: i % 97 != 0,
        };
        let tid = lib::upsert_track(&mut tx, EXT, &track, now).await?;
        let added = now - i64::try_from(i).unwrap_or(0) * 9_000;
        lib::upsert_library(&mut tx, EXT, LibraryKind::Liked, &item_id, Some(&tid), Some(added)).await?;

        // Duplicates across extensions (first 1,200): the same song from the radio extension,
        // unmatched until T-30 merges them.
        if i < 1_200 && i % 6 == 0 && ext == "demo-store" {
            let dup_id = format!("demo-radio:dup-{i:05}");
            let dup = TrackInput {
                item_id: dup_id.clone(),
                url: format!("https://demo-radio.example/track/dup-{i:05}"),
                title: format!("{} (Live)", track.title),
                ..track.clone()
            };
            let did = lib::upsert_track(&mut tx, EXT, &dup, now).await?;
            lib::upsert_library(&mut tx, EXT, LibraryKind::Liked, &dup_id, Some(&did), Some(added - 60)).await?;
        }
    }

    for p in 0..12 {
        let input = PlaylistInput {
            item_id: format!("demo-playlist-{p}"),
            name: if p == 4 { "ស្តាប់ពេលយប់".to_owned() } else { format!("{} mix", rng.pick(WORDS)) },
            owner: if p % 2 == 0 { "Demo Listener".to_owned() } else { "Unison Radio".to_owned() },
            owner_id: if p % 2 == 0 { "demo".to_owned() } else { "other".to_owned() },
            editable: p % 2 == 0,
            artwork: None,
            snapshot: Some(format!("s{p}")),
            track_count: u32::try_from(10 + p * 17).unwrap_or(10),
        };
        lib::upsert_playlist(&mut tx, EXT, &input, now).await?;
    }

    tx.commit().await?;
    pool.close().await;
    let abs = std::fs::canonicalize(&path)?;
    let shown = abs.to_string_lossy().trim_start_matches(r"\\?\").to_owned();
    println!("Demo database written to {shown}");
    println!("Open it with:  $env:UNISON_DB=\"{shown}\"; pnpm tauri dev   (PowerShell)");
    Ok(())
}
