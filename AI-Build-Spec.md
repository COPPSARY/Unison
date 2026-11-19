# Unison — AI Build Spec (v1.0, Windows)

This tab is the build contract for an AI coding agent. Together with the Plan tab, it defines everything needed to go from an empty folder to a released Unison 1.0 for Windows. Where this tab and the Plan tab disagree, this tab wins.

## 0. How the AI agent uses this spec

Work through the task backlog (section 11) strictly in order, one task per session or commit. Never start a task whose dependencies are not done.

**Read order at the start of every session:** `CLAUDE.md` → `docs/PROGRESS.md` → the current task in section 11 → the sections it references.

**Per-task loop**

1. Restate the task's acceptance criteria in `docs/PROGRESS.md` under "In progress".
2. Write or update tests first where the task names them.
3. Implement the smallest change that meets the criteria. Don't refactor unrelated code.
4. Run `pnpm lint`, `pnpm typecheck`, `cargo clippy -- -D warnings`, `cargo test`, `pnpm test`. All must pass.
5. Run the app (`pnpm tauri dev`) and check the criteria by hand when they involve UI.
6. Move the task to "Done" in `docs/PROGRESS.md` with a one-line note, then commit: `feat(T-xx): <summary>`.

**Definition of done (every task):** criteria met · all checks green · no `unwrap()`/`expect()` in non-test Rust code · no `any` in TypeScript · user-facing strings in `src/i18n/en.json` · no secrets or tokens in code, logs or the database.

**When blocked:** if a task needs something only the owner can provide (section 1), or the spec is ambiguous, stop and write the question under "Blocked" in `docs/PROGRESS.md`. Don't invent API behavior: check the official docs linked in section 6.

**Scope rule:** build only what section 2 marks "In". Ideas go in `docs/IDEAS.md`.

## 1. Human-only setup (owner does these)

An AI agent cannot create accounts, accept terms or pay. Do these before the task that needs them; put the resulting values in `.env.local` (never committed) or GitHub Actions secrets.

| # | What | Steps | Produces | Needed by |
| --- | --- | --- | --- | --- |
| H1 | GitHub repo | Create private repo `unison`; enable Actions | Repo URL | T-01 |
| H2 | Dev tools on Windows | Install Rust (rustup, MSVC), Node 22 LTS + pnpm, VS Build Tools "Desktop development with C++", WebView2 (preinstalled on Win 11), Git | Working `pnpm tauri dev` | T-01 |
| H3 | Your own Spotify dev app (for testing BYOK) | developer.spotify.com/dashboard → Create app → Web API; add redirect URIs `http://127.0.0.1:43821/callback`, `…43822`, `…43823` | A Client ID to test with | T-10 |
| H4 | Google Cloud project | console.cloud.google.com → new project "Unison" → enable **YouTube Data API v3** | Project | T-20 |
| H5 | OAuth consent screen | External, app name Unison, support email, scope `youtube.readonly`; status Testing; add tester emails (max 100) | Consent screen | T-20 |
| H6 | Google OAuth client | Credentials → OAuth client ID → type **Desktop app** | `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` (not truly secret for desktop) | T-20 |
| H7 | Last.fm API key | last.fm/api/account/create | `LASTFM_API_KEY` | T-51 |
| H8 | Domain + website | e.g. unison app domain; host a landing page, privacy policy and terms (Section 9 lists what they must say) | Public URLs | T-75, Google verification |
| H9 | Updater signing key | Run `pnpm tauri signer generate` locally; keep private key + password safe | `TAURI_SIGNING_PRIVATE_KEY`, password secrets; public key goes in config | T-61 |
| H10 | Windows code signing | Get a code-signing option (e.g. a cloud signing service or an OV certificate) so SmartScreen doesn't block the installer | Signing credentials as CI secrets | T-62 |
| H11 | Google OAuth verification | After beta: submit app for verification with homepage, privacy policy, demo video of each scope's use | Public Google login (no 100-user cap) | Public launch |
| H12 | Testers | Recruit beta testers with Spotify Premium (needed for their own dev app) and a Google account | Tester list | T-63 |

## 2. Product scope (v1.0, Windows)

v1.0 is done when every "In" row works for a tester who connects their own Spotify key and a Google account. Milestone column shows where each feature lands: **M** = MVP (Phases 0–5), **B** = beta (Phase 6), **R** = 1.0 release.

| Area | Feature | Status | Milestone |
| --- | --- | --- | --- |
| Onboarding | First-run screen: connect Spotify (BYOK guide) and/or YouTube; skip allowed | In | M |
| Accounts | Connect, reconnect, disconnect Spotify and YouTube; show sync status | In | M |
| Library | Liked songs (both), Spotify saved albums, playlists (both), followed artists / subscriptions | In | M |
| Library | Merged "All liked" list with source badges and dedupe | In | M |
| Library | Filter by source, sort by date added / title / artist, text filter | In | M |
| Playlists | View any playlist's tracks; play from position | In | M |
| Playlists | Create Unison playlist mixing sources (stored locally) | In | B |
| Playlists | Add/remove tracks on Spotify playlists the user owns | In | R |
| Playlists | Copy a playlist Spotify → YouTube or back | Out (v1.1) | — |
| Search | Ctrl+K search across local library; then Spotify + YouTube catalog | In | M |
| Home | Quick picks, Jump back in, Because you like {artist}, Your top (4 weeks / 6 months / all time), Rediscover, Trending music | In | M |
| Playback | Spotify via Connect (device picker, play/pause/seek/next/volume) | In | M |
| Playback | YouTube via official IFrame player (mini-player + expanded video view) | In | M |
| Playback | Unified queue mixing Spotify and YouTube items, auto-advance | In | B |
| Playback | Media keys, Windows media overlay (SMTC) | In | B |
| Playback | Time-synced lyrics in Now Playing (LRCLIB), plain-text fallback | In | B |
| Matching | Same song on both platforms shown once; manual "not the same song" fix | In | M |
| Settings | Theme (dark/light), BYOK Client ID, sync interval, clear cache, export my data | In | R |
| System | Tray icon, start minimized option, auto-update, crash reports (opt-in) | In | B |
| i18n | English UI; Khmer-ready (font fallback, string files) | In | R |
| Unofficial YT Music features (home mixes) | — | Out | — |
| SoundCloud, Apple Music, Deezer | — | Out (later providers) | — |
| macOS / Linux release | — | Out (CI builds only) | — |
| Downloads / offline audio | — | Out (against platform terms) | — |

## 3. Screen specs

Direction: Apple-inspired glass, dark-first (see `docs/decisions/0002-apple-inspired-glass-design.md`, which supersedes the Plan tab's colour/type/shape/motion sections). Opaque content layer; glass control layer (title bar, sidebar, player bar, menus, sheets, toasts) over a Windows 11 Mica backdrop. One accent that adapts to the current artwork (violet fallback), label/fill tiers, Inter with optical sizing, capsule controls, concentric radii, spring motion, monochrome line icons, hover by tone only. Components must use role tokens, never raw hex. Pixel values below (sidebar width, row height, radii) may be adjusted to the glass layout as long as the structure stays.

### Shell (every screen)

- **Window:** custom title bar (Tauri `decorations: false`) 40 px high, drag region, Windows min/max/close on the right. Minimum window 960 × 600.
- **Sidebar:** 240 px, collapses to 64 px below 1100 px width. Groups: *Listen* (Home, Search) · *Library* (Liked songs, Albums, Playlists, Artists) · *Your playlists* (scrollable list, both sources mixed, small monochrome source mark). Settings and account status pinned at the bottom.
- **Style:** Finder-like (ADR 0002 addendum). Bold muted section headers that collapse from a hover chevron; 28 px rows; icons tinted with the accent; labels in `text`. **Active item:** grey rounded fill (`fill-secondary`), no bar.
- **Player bar:** 72 px, fixed bottom, full width. Left: 48 px cover, title, artist, source mark, like toggle. Centre: previous, play/pause (the only accent-filled control), next, shuffle, repeat; seek bar with tabular time on both sides. Right: queue, device picker (Spotify), volume, expand.
- **Content area:** page padding 32 px; list pages use full width; Settings capped at 720 px.

### Home

- Rows in this order: Quick picks (4-column grid of compact track rows, 20 items), Jump back in (cover grid, 6 across), Because you like {artist} (2–3 rows), Your top (artists as circular tiles + tracks, with a segmented 4 weeks / 6 months / all time switch; hidden until a Spotify sync has top items), Rediscover, Trending music.
- Row title only, no subtitles. "Show all" text link on the right.
- Hovering a cover reveals a play button on the art (meaningful reveal); the card itself doesn't move.
- **States:** first run with nothing connected → a single panel: "Connect a music account to fill Home" with two buttons (Connect Spotify, Connect YouTube). Syncing → skeleton rows shaped like the content. Too little data (< 20 signals) → hide taste rows, show Trending + a line "Picks improve as you listen".

### Library lists (Liked songs, Playlist detail, Album detail)

- **Header:** cover (or 2×2 mosaic), title, owner, track count and total duration in `text-muted`, source mark; actions: Play (accent), Shuffle, overflow menu.
- **Toolbar:** source filter (segmented: All / Spotify / YouTube), text filter, sort select.
- **Table:** columns # · Title (cover 40 px + title + artist under it) · Album · Source · Date added · Duration (right-aligned, tabular). Row 56 px. Sticky header. Hover = `surface-raised`; double-click or Enter plays; the row number turns into a play icon on hover.
- Virtualised (TanStack Virtual) to handle 10,000+ rows.
- Unavailable items (deleted video, region-blocked) shown in `text-disabled` with a reason tooltip.
- **States:** loading skeleton rows · empty ("No liked songs on this account yet") · no results ("No tracks match 'xyz'" + Clear filter) · error ("Couldn't load from YouTube" + Retry).

### Albums / Artists / Playlists index

Cover grid (image is the identifier): 180 px tiles, 1:1 covers, artists circular. Title + one line of metadata. Same source filter and sort as lists.

### Search (Ctrl+K)

- Command-palette overlay, 640 px wide, opens with a 150 ms fade + 0.98→1 scale from centre.
- Sections: In your library (instant, local) · Spotify · YouTube (fetched after 300 ms debounce, Enter forces). Arrow keys move, Enter plays, Ctrl+Enter opens.
- YouTube catalog search costs 100 quota units, so it only runs on Enter or when the user selects the YouTube section, never on every keystroke.

### Now playing (expanded)

For YouTube: the video at 16:9 in the content area with title and channel below; the IFrame player is the visual. For Spotify: large cover (max 480 px) with title, artist, album. Queue in a 320 px right panel.

Background: a slow-moving ambient blur built from the artwork's colours (static under reduced motion). Lyrics panel (T-45) toggles in place of the queue: the current line in `text`, others in `text-tertiary`, auto-scrolls with a smooth spring; clicking a line seeks; "Lyrics not available" empty state; small "Lyrics: LRCLIB" credit.

### Accounts & BYOK onboarding

- Settings → Accounts: one row per provider with name, connected account, status label (Connected / Reconnect needed / Not connected), last sync time, actions.
- Connect Spotify opens a 3-step drawer: (1) open Spotify Developer Dashboard button, (2) redirect URIs with copy buttons, (3) paste Client ID → "Connect" opens the browser. Each step has one short line of instruction; this is where explanation is warranted.
- Disconnect asks for confirmation: "Disconnect Spotify? Your Spotify library will be removed from Unison. Nothing changes on Spotify."

### Settings

Sections at 720 px: Accounts · Playback (default source preference when a song exists on both) · Appearance (theme) · Sync (interval 15/30/60 min) · Data (clear cache, export JSON, delete all local data — danger, at the bottom) · About (version, check for updates, licenses).

### Global feedback

Toasts bottom-right above the player bar (sync finished, added to playlist, with Undo where possible). Page banner for app-wide problems: offline, YouTube quota exhausted until reset time, Spotify token expired.

## 4. Repository structure

One repo, pnpm for the frontend, a Cargo workspace for Rust. Pin versions in lockfiles; Tauri 2.x, React 19, TypeScript strict.

```text
unison/
├─ CLAUDE.md                 # agent rules (section 13)
├─ docs/
│  ├─ PROGRESS.md            # In progress / Done / Blocked
│  ├─ IDEAS.md
│  └─ decisions/             # short ADRs: 0001-byok-spotify.md …
├─ .github/workflows/
│  ├─ ci.yml                 # lint, typecheck, test (win, mac, linux)
│  └─ release.yml            # signed Windows installer + updater json
├─ src/                      # React app
│  ├─ app/                   # router, providers, shell layout
│  ├─ routes/                # home, search, library/*, playlist/$id, album/$id, artist/$id, settings/*
│  ├─ components/
│  │  ├─ ui/                 # buttons, inputs, menus (Radix-based), table, skeleton, toast
│  │  ├─ track/              # TrackRow, TrackTable, CoverTile, SourceMark
│  │  ├─ player/             # PlayerBar, Seek, Volume, DevicePicker, QueuePanel, YouTubeHost
│  │  └─ shell/              # TitleBar, Sidebar, CommandPalette, Banner
│  ├─ lib/
│  │  ├─ bindings.ts         # GENERATED by tauri-specta — never edit
│  │  ├─ queries.ts          # TanStack Query hooks wrapping commands
│  │  └─ events.ts           # typed event listeners
│  ├─ stores/                # zustand: player, ui
│  ├─ styles/tokens.css      # role tokens, dark + light
│  └─ i18n/en.json
├─ src-tauri/
│  ├─ tauri.conf.json
│  ├─ capabilities/default.json
│  ├─ migrations/            # 0001_init.sql …
│  └─ src/
│     ├─ main.rs / lib.rs     # builder, plugins, command registration
│     ├─ commands/            # thin #[tauri::command] fns, one file per area
│     ├─ core/
│     │  ├─ auth/             # pkce.rs, loopback.rs, keychain.rs
│     │  ├─ db/               # pool, repos per table
│     │  ├─ sync/             # scheduler, jobs, quota.rs
│     │  ├─ matching/         # normalize.rs, matcher.rs
│     │  ├─ recs/             # signals.rs, candidates.rs, rank.rs, lastfm.rs
│     │  └─ playback/         # router.rs, queue.rs, smtc.rs
│     ├─ providers/
│     │  ├─ mod.rs            # MusicProvider trait, ProviderId
│     │  ├─ spotify/          # client.rs, models.rs, map.rs
│     │  └─ youtube/          # client.rs, models.rs, map.rs, music_filter.rs
│     ├─ model.rs             # Track, Playlist, Account… (serde + specta)
│     └─ error.rs             # AppError (thiserror), serialisable to UI
└─ tests/fixtures/           # recorded API JSON for offline tests
```

**Key crates:** `tauri` 2, `tauri-plugin-opener`, `tauri-plugin-updater`, `tauri-plugin-single-instance`, `tauri-plugin-window-state`, `tokio`, `reqwest` (rustls), `serde`, `sqlx` (sqlite, migrate), `keyring`, `oauth2` or hand-rolled PKCE, `thiserror`, `tracing`, `specta` + `tauri-specta`, `strsim`, `unicode-normalization`, `windows` (SMTC).

**Key npm packages:** `react-router` or `@tanstack/react-router`, `@tanstack/react-query`, `@tanstack/react-virtual`, `zustand`, `@radix-ui/*`, `tailwindcss` v4, `lucide-react`, `vitest`, `@testing-library/react`, `eslint`, `prettier`.

## 5. React ↔ Rust contract

All types live in Rust (`model.rs`) and are exported to `src/lib/bindings.ts` by tauri-specta on every debug build. The UI never calls HTTP APIs directly and never sees tokens.

**Shared types (Rust, abbreviated)**

```rust
pub enum ProviderId { Spotify, Youtube }
pub struct SourceRef { pub provider: ProviderId, pub item_id: String, pub url: String }
pub struct Track { pub id: String /* unified uuid */, pub title: String, pub artists: Vec<String>,
  pub album: Option<String>, pub duration_ms: Option<u32>, pub artwork: Option<String>,
  pub sources: Vec<SourceRef>, pub available: bool }
pub struct Playlist { pub id: String, pub provider: Option<ProviderId> /* None = Unison */, pub name: String,
  pub owner: String, pub track_count: u32, pub artwork: Option<String>, pub editable: bool }
pub struct Page<T> { pub items: Vec<T>, pub total: u32, pub next_offset: Option<u32> }
pub enum AccountStatus { NotConnected, Connected, NeedsReconnect, Syncing }
pub struct Account { pub provider: ProviderId, pub status: AccountStatus, pub display_name: Option<String>, pub last_synced_at: Option<i64> }
pub struct AppError { pub code: ErrorCode, pub message: String, pub retryable: bool }
```

**Commands**

| Command | Args | Returns | Notes |
| --- | --- | --- | --- |
| `accounts_list` | — | `Vec<Account>` |  |
| `spotify_set_client_id` | `client_id` | `()` | Validates format, stores in keychain |
| `account_connect` | `provider` | `Account` | Runs full PKCE loopback flow; 5 min timeout |
| `account_disconnect` | `provider` | `()` | Deletes tokens + that provider's rows |
| `sync_now` | `provider?` | `()` | Emits `sync://progress` |
| `library_liked` | `source?, sort, filter?, offset, limit` | `Page<Track>` | Local DB only |
| `library_albums` / `library_artists` | `source?, sort` | `Vec<…>` | Local |
| `playlists_list` | `source?` | `Vec<Playlist>` | Local |
| `playlist_tracks` | `playlist_id, offset, limit` | `Page<Track>` | Local; fetches if never synced |
| `playlist_create` | `name` | `Playlist` | Unison playlist (local) |
| `playlist_add` / `playlist_remove` | `playlist_id, track_ids` | `()` | Spotify owned or Unison only |
| `search_local` | `query` | `Vec<Track>` | FTS5, < 30 ms |
| `search_remote` | `query, provider` | `Vec<Track>` | YouTube costs 100 units; checks quota first |
| `home_feed` | — | `HomeFeed` | Cached 1 h; rows from section 8 |
| `track_mark_not_same` | `track_id, source` | `()` | Splits a wrong match |
| `track_like` | `track_id, liked` | `()` | Writes to Spotify library / YouTube rating |
| `player_play` | `track_ids, start_index` | `()` | Builds queue, routes by source |
| `player_control` | `PlayerAction` | `()` | Pause, Resume, Next, Prev, Seek(ms), Volume(0–100), Shuffle, Repeat |
| `spotify_devices` / `spotify_set_device` | `device_id?` | `Vec<Device>` / `()` | Connect |
| `youtube_player_report` | `state, position_ms` | `()` | Called by the IFrame host so Rust owns queue state |
| `settings_get` / `settings_set` | `Settings` | `Settings` |  |
| `data_export` / `data_wipe` | — | path / `()` |  |

**Events (Rust → UI)**

| Event | Payload | UI reaction |
| --- | --- | --- |
| `sync://progress` | provider, stage, done, total | Status line in sidebar footer |
| `sync://finished` | provider, added, removed | Invalidate queries, toast |
| `account://status` | `Account` | Banner when `NeedsReconnect` |
| `player://state` | now playing track, position, playing, queue length, device | Player bar |
| `player://youtube-command` | load(video\_id) / play / pause / seek | YouTubeHost component drives the IFrame |
| `quota://youtube` | used, limit, resets\_at | Banner at 90% and 100% |

**YouTube playback detail:** the IFrame player lives in a React component (`YouTubeHost`) that is always mounted (hidden 1×1 when the mini-player is closed is **not** allowed by YouTube's terms, so keep it visible at min 200×200 px in the mini-player or the expanded view). Rust sends commands via `player://youtube-command`; the component reports `onStateChange` back via `youtube_player_report`. Set `origin` correctly; Tauri serves the app from `http://tauri.localhost` on Windows, so test embeds early (T-41).

## 6. Provider API mapping

Base URLs: Spotify `https://api.spotify.com/v1`, YouTube `https://www.googleapis.com/youtube/v3`. Before implementing any call, the agent checks the current official reference: [Spotify Web API](https://developer.spotify.com/documentation/web-api) and its [Feb 2026 changes](https://developer.spotify.com/documentation/web-api/references/changes/february-2026); [YouTube Data API](https://developers.google.com/youtube/v3/docs) and [quota costs](https://developers.google.com/youtube/v3/determine_quota_cost). If a call below has changed, follow the docs and note it in `docs/decisions/`.

### Spotify

| Purpose | Call | Paging | Maps to |
| --- | --- | --- | --- |
| Profile | `GET /me` | — | `accounts.display_name` (email/country no longer returned) |
| Liked songs | `GET /me/tracks?limit=50` | offset/next until null | `tracks`, `library(kind=liked)` |
| Saved albums | `GET /me/albums?limit=50` | offset | `albums`, `library(kind=saved_album)` |
| Playlists | `GET /me/playlists?limit=50` | offset | `playlists`; store `snapshot_id` |
| Playlist items | `GET /playlists/{id}/items?limit=100` | offset | `playlist_items`; only the user's own playlists return items; followed playlists show metadata + "Open in Spotify" |
| Followed artists | `GET /me/following?type=artist&limit=50` | cursor `after` | `artists` |
| Top items | `GET /me/top/{artists,tracks}?time_range={short,medium,long}_term&limit=50` | — | `signals(kind=top_*)` |
| Recently played | `GET /me/player/recently-played?limit=50` | cursor `before` | `signals(kind=recent)` |
| Artist albums | `GET /artists/{id}/albums` | offset | candidates |
| Search | `GET /search?type=track&limit=10` | max 10 | search results, matching |
| Like / unlike | `PUT` / `DELETE /me/library` with URIs | — | `library` |
| Playback | `GET /me/player/devices`, `PUT /me/player/play`, `/pause`, `POST /next`, `/previous`, `PUT /seek`, `/volume`, `GET /me/player` (poll 1 s while playing) | — | `player://state` |

**Rules:** honour `429` + `Retry-After`; max 4 concurrent requests; exponential backoff on 5xx (0.5, 1, 2, 4 s, then fail). On `401` refresh once; on refresh `invalid_grant` set `NeedsReconnect`. Playback calls need Premium and an active device: if none, return `ErrorCode::NoSpotifyDevice` and the UI shows "Open Spotify on this PC or your phone, then pick it here." Local files (`is_local: true`) are shown but not playable. Tracks with `null` (removed) are skipped.

### YouTube

| Purpose | Call | Cost (units) | Maps to |
| --- | --- | --- | --- |
| Channel | `channels.list?mine=true&part=snippet` | 1 | `accounts` |
| Liked videos | `videos.list?myRating=like&part=snippet,contentDetails&maxResults=50` | 1 per page | `tracks` (music only), `library(kind=liked)` |
| Playlists | `playlists.list?mine=true&part=snippet,contentDetails&maxResults=50` | 1 per page | `playlists`; store ETag |
| Playlist items | `playlistItems.list?playlistId=…&part=snippet,contentDetails&maxResults=50` | 1 per page | `playlist_items` |
| Video details batch | `videos.list?id=…(50)&part=contentDetails,snippet,status` | 1 | duration, category, embeddable |
| Subscriptions | `subscriptions.list?mine=true&part=snippet&maxResults=50` | 1 per page | `artists` (music channels only) |
| Trending music | `videos.list?chart=mostPopular&videoCategoryId=10&regionCode=XX&maxResults=50` | 1 | Home trending row (cache 6 h, shared by all) |
| Search | `search.list?type=video&videoCategoryId=10&q=…` | **100** | On explicit request only |
| Rate (like) | `videos.rate` | 50 | `library` |

**Music filter** (`music_filter.rs`): keep a video if any of: `categoryId == 10`; channel title ends with " - Topic"; channel name contains "VEVO"; or it sits in a playlist the user has marked "Music playlist" in Unison (the API can't tell YouTube Music playlists apart). Everything else stays out of the music library but is still visible inside the user's own playlists.

**Rules:** duration parsed from ISO-8601 (`PT3M42S`); skip items where `status.embeddable == false` (mark unavailable); "Deleted video" / "Private video" titles → unavailable. Use ETags with `If-None-Match`. Track quota locally per Pacific-time day (`quota.rs`); refuse non-essential calls above 90% of the daily budget. Refresh any stored YouTube data older than 30 days or delete it.

### LRCLIB (lyrics)

`GET https://lrclib.net/api/get?track_name=…&artist_name=…&album_name=…&duration=…` (seconds) returns `syncedLyrics` (LRC `[mm:ss.xx] line`) and/or `plainLyrics`; 404 when unknown. No key; send a `User-Agent` naming the app and version (no user data). Cache per track 30 days in `cache_kv`; fetch only when Now Playing lyrics are shown or prefetch the next queue item. Verify against the live docs at lrclib.net/docs before implementing.

### Last.fm (recommendations)

`artist.getSimilar` (limit 20), `track.getSimilar` (limit 20), `artist.getTopTracks` (limit 10). Cache 7 days. Max 5 req/s. Results are names only: resolve to playable items via section 8.

## 7. Database schema

SQLite in `%APPDATA%/app.unison/unison.db`, WAL mode, migrations run at startup via `sqlx::migrate!`. Timestamps are Unix seconds (UTC).

```sql
-- 0001_init.sql
CREATE TABLE accounts (
  provider TEXT PRIMARY KEY,              -- 'spotify' | 'youtube'
  account_id TEXT NOT NULL,
  display_name TEXT,
  status TEXT NOT NULL,                   -- connected | needs_reconnect
  connected_at INTEGER NOT NULL,
  last_synced_at INTEGER
);
CREATE TABLE tracks (
  id TEXT PRIMARY KEY,                    -- uuid v7
  title TEXT NOT NULL,
  title_norm TEXT NOT NULL,
  artists_json TEXT NOT NULL,             -- ["Artist A","Artist B"]
  artist_norm TEXT NOT NULL,              -- normalised primary artist
  album TEXT,
  duration_ms INTEGER,
  isrc TEXT,
  artwork_url TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_tracks_isrc ON tracks(isrc);
CREATE INDEX idx_tracks_norm ON tracks(artist_norm, title_norm);
CREATE TABLE track_sources (
  track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  item_id TEXT NOT NULL,                  -- spotify track id | youtube video id
  url TEXT NOT NULL,
  available INTEGER NOT NULL DEFAULT 1,
  match_confidence REAL NOT NULL DEFAULT 1.0,
  user_split INTEGER NOT NULL DEFAULT 0,  -- user said "not the same song"
  fetched_at INTEGER NOT NULL,
  PRIMARY KEY (provider, item_id)
);
CREATE INDEX idx_sources_track ON track_sources(track_id);
CREATE TABLE albums (id TEXT PRIMARY KEY, provider TEXT NOT NULL, item_id TEXT NOT NULL, name TEXT NOT NULL,
  artists_json TEXT NOT NULL, artwork_url TEXT, release_date TEXT, UNIQUE(provider, item_id));
CREATE TABLE artists (id TEXT PRIMARY KEY, provider TEXT NOT NULL, item_id TEXT NOT NULL, name TEXT NOT NULL,
  name_norm TEXT NOT NULL, artwork_url TEXT, UNIQUE(provider, item_id));
CREATE TABLE playlists (
  id TEXT PRIMARY KEY,
  provider TEXT,                          -- NULL = Unison local playlist
  item_id TEXT,
  name TEXT NOT NULL,
  owner TEXT,
  editable INTEGER NOT NULL,
  artwork_url TEXT,
  snapshot TEXT,                          -- spotify snapshot_id | youtube etag
  items_synced INTEGER NOT NULL DEFAULT 0,
  synced_at INTEGER,
  UNIQUE(provider, item_id)
);
CREATE TABLE playlist_items (
  playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  track_id TEXT NOT NULL REFERENCES tracks(id),
  source_provider TEXT NOT NULL,
  added_at INTEGER,
  PRIMARY KEY (playlist_id, position)
);
CREATE TABLE library (
  track_id TEXT REFERENCES tracks(id) ON DELETE CASCADE,
  album_id TEXT REFERENCES albums(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  kind TEXT NOT NULL,                     -- liked | saved_album | followed_artist
  ref_id TEXT NOT NULL,                   -- track/album/artist id
  added_at INTEGER,
  PRIMARY KEY (provider, kind, ref_id)
);
CREATE TABLE signals (
  id INTEGER PRIMARY KEY,
  subject_type TEXT NOT NULL,             -- track | artist
  subject_id TEXT NOT NULL,               -- tracks.id or artists.name_norm
  kind TEXT NOT NULL,                     -- top_short|top_medium|top_long|recent|liked|play|skip|replay
  weight REAL NOT NULL,
  at INTEGER NOT NULL
);
CREATE INDEX idx_signals_subject ON signals(subject_type, subject_id);
CREATE TABLE sync_state (provider TEXT, resource TEXT, cursor TEXT, etag TEXT, last_synced_at INTEGER,
  PRIMARY KEY (provider, resource));
CREATE TABLE quota_usage (day TEXT PRIMARY KEY, units INTEGER NOT NULL); -- youtube, Pacific date
CREATE TABLE cache_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL, expires_at INTEGER NOT NULL);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE VIRTUAL TABLE tracks_fts USING fts5(title, artists, album, content='tracks', content_rowid='rowid');
```

Tokens and the BYOK Client ID never go in this database; they live in Windows Credential Manager under service `app.unison`, keys `spotify.client_id`, `spotify.tokens`, `youtube.tokens`.

## 8. Algorithms

### 8.1 Normalisation (`normalize.rs`)

1. Unicode NFKC, lowercase, trim; keep non-Latin scripts (Khmer, Korean, Japanese) as-is.
2. Title: remove bracketed noise matching `(official (music )?video|official audio|lyrics?( video)?|audio|visuali[sz]er|mv|m/v|hd|4k|remaster(ed)?( \d{4})?)` inside `()` or `[]`; remove `feat./ft./featuring …` (move names to artists); strip `"Artist - "` prefix when it equals the channel/artist.
3. Artist: strip `  - topic `, `vevo`, `official`; split on `,`, `&`, `x`, `feat.`.
4. Collapse punctuation and whitespace.

### 8.2 Matching (`matcher.rs`)

Run when a source item is inserted. Find an existing `tracks` row:

1. Same ISRC → match, confidence 1.0.
2. Else candidates with same `artist_norm` (or artist similarity ≥ 0.9 Jaro-Winkler), then score = 0.6 × title similarity (Jaro-Winkler on `title_norm`) + 0.25 × artist similarity + 0.15 × duration score, where duration score = 1 if |Δ| ≤ 3 s, 0.5 if ≤ 8 s, else 0.
3. Score ≥ 0.90 → merge. 0.80–0.90 → merge only if duration score = 1. Else new track.
4. Never merge if a `user_split` flag exists for the pair, or if one title contains "live", "remix", "acoustic", "cover", "sped up", "slowed" and the other doesn't.

**Test set:** `tests/fixtures/matching.csv` with at least 60 labelled pairs (same / different), including Khmer and K-pop titles. Target: ≥ 95% precision, ≥ 85% recall.

### 8.3 Taste profile (`signals.rs`)

Artist score = Σ (weight × decay). Weights: top\_short 5, top\_medium 3, top\_long 2, recent 2, liked 1.5, play (in Unison, ≥ 30 s) 1, replay 2, skip (< 30 s) −1.5, subscription/followed 2. Decay = 0.5^(age\_days / 30) for recent/play/skip; none for top and liked. Track score uses the same weights on track-level signals plus 0.3 × its artists' scores.

### 8.4 Home rows (`recs/`)

| Row | Rule | Size |
| --- | --- | --- |
| Quick picks | 12 from top tracks by score (exclude played in last 24 h) + 5 from Last.fm `track.getSimilar` of top-5 tracks + 3 rediscoveries; interleave so no artist appears twice in a row; reshuffle each app launch | 20 |
| Jump back in | Distinct albums/playlists from last 50 plays + Spotify recently played | 12 |
| Because you like {artist} | For each of top 3 artists: Last.fm similar artists (exclude ones already in top 50) → their top tracks; resolve playable | 3 rows × 10 |
| Your top | Spotify top artists and tracks for the selected range (signals `top_short` / `top_medium` / `top_long`, order as returned) | 10 artists + 10 tracks |
| Rediscover | Liked > 90 days ago, not played in 90 days, score-weighted random | 15 |
| Trending music | YouTube chart for region (from Windows locale, fallback US) | 20 |

**Resolving a name to a playable item:** (1) local DB by normalised artist+title, (2) Spotify search `track:"t" artist:"a"` limit 5 with matcher, (3) YouTube only from already-fetched data (no `search.list` for recommendations). Unresolvable items are dropped. Cache the feed 1 h; rebuild after sync.

### 8.5 Sync (`sync/`)

- First connect: full sync in order profile → liked → playlists (metadata) → albums/followed → top/recent; playlist items load lazily when a playlist is opened, then in background (owned playlists first).
- Incremental every N minutes (default 30) while running: Spotify playlists only when `snapshot_id` changed; liked songs stop paging at the first known item; YouTube with ETags, max 50 units per user per day for background sync.
- Removals: items missing after a full page set are removed from `library` / `playlist_items`, not from `tracks`.
- One sync per provider at a time; cancellable; progress events every page.

### 8.6 Playback routing (`playback/router.rs`)

For a track with both sources, use Settings → "Preferred source" (default Spotify if connected). Queue holds `SourceRef`s. On end of a Spotify item (poll shows progress ≥ duration − 1 s or paused at 0 after track change) or YouTube `ENDED`, Rust advances and dispatches to the right player, pausing the other. If Spotify has no active device, skip to the YouTube source when available, else show the device prompt.

## 9. Errors, security & privacy

**Error codes** (`ErrorCode`, shown to users through `i18n` messages): `Offline`, `AuthCancelled`, `AuthTimeout`, `NeedsReconnect`, `InvalidClientId`, `RedirectUriMismatch` (show the exact URIs to add), `SpotifyPremiumRequired`, `NoSpotifyDevice`, `RateLimited { retry_after }`, `YoutubeQuotaExceeded { resets_at }`, `NotEmbeddable`, `NotFound`, `Upstream { status }`, `Internal`. Every error the UI shows says what happened and the one next step.

**Security**

- Tokens, refresh tokens and Client ID only in Windows Credential Manager via `keyring`. Never in SQLite, logs, crash reports, events or the UI.
- PKCE S256 with a 64-byte verifier; `state` 32 random bytes, checked; loopback listener binds `127.0.0.1` only, accepts one request, 5-minute timeout.
- Tauri capabilities: least privilege. No `shell` plugin; `opener` limited to `https://` URLs; CSP restricts `frame-src` to `https://www.youtube-nocookie.com https://www.youtube.com`, `img-src` to Spotify/YouTube/Last.fm image hosts, `connect-src` to `ipc:` and the Tauri origin only (all HTTP happens in Rust).
- `tracing` redacts `Authorization` headers and query params named `access_token`, `code`, `key`.
- Single instance lock; updater verifies signature.

**Privacy (must also appear in the privacy policy, H8)**

- All data stays on the user's PC. Unison has no server and collects no listening data.
- Data used: Spotify library, playlists, top items, recently played; YouTube likes, playlists, subscriptions; used only to display the library and generate recommendations locally.
- Last.fm receives artist and track names (no user identity) to fetch similar music.
- Crash reports are opt-in and contain no library data or tokens.
- Users can export (JSON) and wipe all local data in Settings; disconnecting revokes local tokens and deletes that provider's data. Link to Google's [third-party access page](https://myaccount.google.com/permissions) and Spotify's apps page to revoke access fully.
- YouTube API data refreshed or deleted within 30 days; the app links to the [YouTube Terms of Service](https://www.youtube.com/t/terms) and [Google Privacy Policy](https://policies.google.com/privacy), as YouTube API Services require.

## 10. Testing strategy

| Layer | What | Tool | Rule |
| --- | --- | --- | --- |
| Rust unit | normalise, matcher, scoring, PKCE, quota math, ISO-8601 parsing, mappers | `cargo test` | Every function in `core/` and `providers/*/map.rs` has tests |
| Rust integration | Provider clients against recorded JSON (`tests/fixtures/spotify/*.json`, `youtube/*.json`) via `wiremock` | `cargo test` | No live API calls in CI |
| DB | Migrations apply on empty DB; repos round-trip | `sqlx` + temp file | Run in CI |
| Frontend unit | Components render all states (loading, empty, no results, error, full) | Vitest + Testing Library | One test per state per data view |
| E2E smoke | Launch app, navigate every route with a seeded DB | WebdriverIO + `tauri-driver` (Windows) | Runs on Windows CI |
| Manual live | Real Spotify + Google accounts, checklist in `docs/MANUAL_TESTS.md` | Owner | Before every beta/release build |

**Seed data:** `pnpm seed` creates a demo DB with 1,200 tracks including long names, Khmer titles, unavailable items, duplicates across sources and a 10,000-track liked list for performance checks.

**Performance budgets:** cold start < 2 s to first paint; library table scroll 60 fps with 10k rows; `search_local` < 30 ms; idle RAM < 250 MB; installer < 15 MB.

## 11. Task backlog

Do tasks in order. "Needs" lists tasks that must be done first and human steps (H#) from section 1.

### Phase 0 — Foundations

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-01 | Scaffold Tauri 2 + React 19 + TS strict + Vite + Tailwind v4 + pnpm; repo layout from section 4; `CLAUDE.md`, `docs/PROGRESS.md` | H1, H2 | `pnpm tauri dev` opens a window; lint/typecheck/test scripts exist and pass |
| T-02 | CI workflow: lint, typecheck, clippy, tests on windows-latest, macos-latest, ubuntu-latest; build artifacts | T-01 | Green on all three OSes |
| T-03 | Design tokens (`tokens.css`, dark + light), fonts bundled, base UI kit: Button, IconButton, Input, Select, Segmented, Menu, Dialog, Drawer, Tooltip, Toast, Skeleton, Banner | T-01 | Storybook-style `/dev/ui` route shows every component in every state; hover is tone-only; no bright icon on tinted background anywhere |
| T-03a | Apple-inspired redesign (ADR 0002): glass materials + Mica window backdrop with Windows 10 fallback, label/fill tokens, adaptive accent helper with contrast correction, Inter type scale, spring easings, kit restyled (capsules, concentric radii, glass overlays) | T-03 | `/dev/ui` shows glass over artwork, accent swatches adapting, every component in both themes; reduced transparency/motion fallbacks work; tests for spring + accent maths |
| T-04 | Shell: custom title bar, sidebar (collapsing), routes with empty pages, player bar placeholder, keyboard shortcuts | T-03 | All routes reachable by mouse and keyboard; window state persists |
| T-05 | Rust core: `AppError`, `tracing` with redaction, SQLite pool + migration 0001, tauri-specta bindings generation | T-01 | Bindings regenerate on build; DB file created; migration test passes |
| T-06 | `MusicProvider` trait, models, settings commands, single-instance plugin | T-05 | `settings_get/set` round-trip from UI |

### Phase 1 — Spotify

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-10 | PKCE + loopback listener + keychain store (generic, reused for Google) | T-06, H3 | Unit tests for PKCE/state; manual: browser opens, callback page shows, tokens saved in Credential Manager |
| T-11 | BYOK onboarding drawer + `spotify_set_client_id` + `account_connect(spotify)` | T-10, T-04 | New user can paste Client ID and connect; wrong ID and redirect mismatch show their specific errors |
| T-12 | Spotify client: auth refresh, 429/backoff, paging helper | T-10 | Fixture tests for 401 refresh, 429 retry, paging |
| T-13 | Sync liked songs, albums, playlists (metadata), followed artists, top, recent → DB | T-12 | 2,000+ liked songs sync with progress events; re-sync adds/removes correctly |
| T-14 | Liked songs, Albums, Artists, Playlists pages from local DB with virtualised table, filters, sort, all states | T-13, T-03 | 10k-row seed scrolls at 60 fps; every state renders |
| T-15 | Playlist detail with lazy item sync; followed (non-owned) playlists show "Open in Spotify" | T-14 | Owned playlist shows tracks; followed one shows metadata + link |
| T-16 | Accounts settings page: status, last sync, sync now, disconnect with confirmation; reconnect banner on `invalid_grant` | T-13 | Disconnect wipes Spotify rows; forced invalid token shows banner |

### Phase 2 — YouTube

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-20 | Google OAuth (desktop client, loopback, PKCE), channel select | T-10, H4–H6 | Test user can connect; tokens in keychain |
| T-21 | YouTube client with ETags, quota tracker (`quota_usage`, Pacific day), 90% guard, `quota://youtube` events | T-20 | Unit tests on quota day rollover; banner shows at 90% |
| T-22 | Sync likes (music filter), playlists, items, subscriptions; batch `videos.list` for durations/embeddable | T-21 | YouTube likes and playlists appear in library with YouTube mark; non-music likes excluded |
| T-23 | "Mark as music playlist" toggle on YouTube playlists | T-22 | Toggling re-evaluates filter |
| T-24 | 30-day refresh/delete job for YouTube data | T-22 | Test with clock override deletes/refreshes stale rows |

### Phase 3 — Unified library

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-30 | Normaliser + matcher + `matching.csv` test set | T-13, T-22 | ≥ 95% precision, ≥ 85% recall on the set |
| T-31 | Merge on insert, "All liked" merged view, source filter, "Not the same song" action | T-30 | A song liked on both shows once with two marks; split persists across re-sync |
| T-32 | FTS5 index + Ctrl+K palette (local results) | T-31 | Results < 30 ms on 10k seed; full keyboard control |
| T-33 | Remote search sections (Spotify on debounce, YouTube on Enter only) | T-32 | Quota cost logged; YouTube never searched per keystroke |

### Phase 4 — Playback

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-40 | Spotify Connect: devices, play/pause/seek/next/prev/volume, state polling → `player://state` | T-13 | Plays on desktop Spotify and phone; NoSpotifyDevice prompt works |
| T-41 | YouTubeHost IFrame component (min 200×200), command/report bridge; verify embed origin in Tauri on Windows | T-22 | Video plays in mini-player and expanded view; non-embeddable shows error |
| T-42 | Player bar wired to both; like toggle; expanded Now Playing view | T-40, T-41 | Controls work for both sources |
| T-43 | Unified queue + auto-advance + routing + preferred source setting | T-42 | Mixed queue of 10 items plays through without manual action |
| T-44 | Media keys + Windows SMTC overlay | T-43 | Keyboard media keys and Win overlay control playback |
| T-45 | Lyrics: LRCLIB client + cache, LRC parser, Now Playing lyrics panel with live line highlight and click-to-seek | T-43 | Parser unit tests; fixture tests for 200/404; lyrics follow playback for both sources |

### Phase 5 — Recommendations

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-50 | Signals ingestion (top, recent, likes, in-app play/skip/replay) | T-43 | Signals table fills; decay test passes |
| T-51 | Last.fm client + cache | H7 | Fixture tests; 7-day cache |
| T-52 | Candidate generation + resolver + ranking per section 8.4 | T-50, T-51 | Deterministic test with seeded data produces expected rows |
| T-53 | Home page with all rows (incl. Your top with range switch) and states, 1 h feed cache, rebuild after sync | T-52 | First-run, low-data and full states correct; Your top switches ranges |

### Phase 6 — Beta

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-60 | Unison local playlists (create, add, remove, reorder) | T-31 | Mixed-source playlist plays in order |
| T-61 | Auto-updater with signed `latest.json` on GitHub Releases | H9 | Old build updates itself to a new one |
| T-62 | Release workflow: NSIS installer, code signing, version from tag | H10, T-61 | Signed installer passes SmartScreen without "unknown publisher" |
| T-63 | Tray icon, start minimized, opt-in crash reports, `docs/MANUAL_TESTS.md`, beta build | T-62, H12 | Testers install and use daily for a week |

### Phase 7 — Release 1.0

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-70 | Add/remove tracks on owned Spotify playlists; like/unlike sync to source | T-42 | Changes appear in Spotify and YouTube within one sync |
| T-71 | Settings complete: theme, window material (Mica / Acrylic / Off) + glass intensity, sync interval, preferred source, export, wipe | T-16 | Export JSON opens; wipe returns app to first run; material change applies live |
| T-72 | Light theme pass + contrast audit (4.5:1 text, 3:1 controls) + reduced motion | T-03 | Automated contrast check passes; greyscale review done |
| T-73 | Khmer-ready: font fallback (Noto Sans Khmer), line height, all strings in `en.json`, `km.json` stub | T-03 | Khmer titles render without clipping |
| T-74 | Performance + memory pass against section 10 budgets | all | All budgets met on a mid-range Windows laptop |
| T-75 | Beta bug backlog to zero P1/P2; final manual test run; tag `v1.0.0` | all, H8, H11 | Signed 1.0 installer published with release notes |

## 12. Release

- **Versioning:** SemVer. `0.x` during MVP, `0.9.x` for beta, `1.0.0` at release. Version lives in `package.json`, `Cargo.toml` and `tauri.conf.json`; a script keeps them in sync.
- **Installer:** NSIS (`bundle.targets: ["nsis"]`), per-user install (no admin), WebView2 bootstrapper embedded, Start menu shortcut, uninstaller removes app files and asks before deleting user data.
- **Identifier:** `app.unison` (pick final before first release; changing it later breaks updates and data paths).
- **Signing:** sign the installer and the exe in `release.yml` with the credentials from H10. Unsigned builds are for internal testing only.
- **Updates:** `tauri-plugin-updater` reading `latest.json` from GitHub Releases; signed with the H9 key; the app checks on launch and every 6 h, downloads in background, asks to restart.
- **Release flow:** push tag `vX.Y.Z` → CI builds, signs, uploads installer + `latest.json` to a draft GitHub Release → owner runs `docs/MANUAL_TESTS.md` on the draft → publish.
- **Release notes:** generated from conventional commits, edited by the owner; written for users ("YouTube playlists now load twice as fast"), not commit messages.
- **Channels:** `beta` and `stable` update feeds; testers on beta.
- **Before 1.0:** Google OAuth verification approved (H11), privacy policy live (H8), Spotify and YouTube branding checked against their current guidelines, lyrics licensing reviewed (LRCLIB is community-sourced), no Apple trade dress or assets in the UI.

## 13. CLAUDE.md (put in repo root)

```markdown
# Unison — agent rules

Unison is a Windows desktop app (Tauri 2 + React 19 + TypeScript) that unifies a user's
Spotify and YouTube / YouTube Music libraries, with local recommendations.
The full spec is the "AI Build Spec" doc; sections are referenced as §N.

## Workflow
- Read docs/PROGRESS.md first. Work on exactly one task (T-xx) from §11, in order.
- Before coding, write the task's acceptance criteria under "In progress".
- Finish with: pnpm lint && pnpm typecheck && pnpm test && cargo clippy -- -D warnings && cargo test
- Commit as feat(T-xx): summary. Update PROGRESS.md (Done / Blocked).
- Blocked or ambiguous? Write the question under "Blocked" and stop. Never guess API behaviour.

## Architecture rules
- All network calls happen in Rust. React only calls typed commands from src/lib/bindings.ts.
- Never edit src/lib/bindings.ts by hand; it is generated by tauri-specta.
- Tokens and the Spotify Client ID live only in the OS keychain (keyring). Never in SQLite,
  logs, events, frontend state or crash reports.
- New platform = new module in src-tauri/src/providers implementing MusicProvider. No provider
  logic in commands/ or the UI.
- YouTube: never call search.list during sync or recommendations. Check quota.rs before any call.
  Playback only through the official IFrame player, visible at >= 200x200 px.
- Spotify: Development Mode endpoints only (see §6). Handle 429 Retry-After and invalid_grant.
- No unwrap()/expect() outside tests. Errors are AppError with an ErrorCode (§9).
- SQL changes only through new files in src-tauri/migrations; never edit an applied migration.

## UI rules (minimal, dark-first)
- Use role tokens from src/styles/tokens.css. No raw hex in components.
- One accent (violet) for primary action, selection, progress and focus only.
- Icons: lucide-react, 1.5px stroke, colour text or text-muted. Never bright-coloured icons,
  never icons on tinted circles/squares.
- Hover = background or text tone change, 120ms. No translate/scale/shadow on hover.
- No gradients except album-art scrim and skeleton shimmer.
- No subtitles that repeat a title, no greeting banners, no emoji.
- Numbers: tabular-nums, right-aligned. Spacing from the 4px scale only.
- Every data view has loading (skeleton), empty, no-results and error states.
- Motion 120-200ms, cubic-bezier(0.16,1,0.3,1), no bounce, nothing loops; respect reduced motion.
- All user-facing strings in src/i18n/en.json.

## Commands
- Dev: pnpm tauri dev
- Seed demo data: pnpm seed
- Tests: pnpm test / cargo test (no live API calls; use tests/fixtures)
- Build installer: pnpm tauri build

## Never
- Commit .env.local, keys, tokens or fixture files containing real user data.
- Add unofficial YouTube Music APIs, audio download or ad-blocking.
- Add features not marked "In" in §2; put ideas in docs/IDEAS.md.
```
