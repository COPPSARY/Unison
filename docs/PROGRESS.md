# Progress

## Order (2026-11-19)
T-15 → UP-01 … UP-16 (Universal Player addendum) → T-16 → T-20+ (YouTube parked) → T-30 …
T-40 … T-44 are replaced by UP tasks. New human step H13: Audius API key (for UP-13).

## In progress

### T-02 — CI workflow
Acceptance criteria:
- `.github/workflows/ci.yml` runs on push and pull request.
- Matrix: windows-latest, macos-latest, ubuntu-latest.
- Each job: pnpm lint, typecheck, test; cargo clippy -D warnings; cargo test.
- Each job builds the app and uploads the bundle as an artifact (Windows NSIS, macOS .app, Linux .deb).
- Green on all three OSes.
- Status: pushed (f25acf7); waiting for the owner to confirm the first run.

### T-14 — Library pages
Acceptance criteria:
- Commands (local DB only): `library_liked` (source, sort, filter, offset, limit → Page<Track>),
  `library_liked_stats` (count + total duration), `library_albums`, `library_artists`,
  `playlists_list` (source + sort).
- Liked songs: header (2×2 cover mosaic, title, count + total duration, Play / Shuffle / menu),
  toolbar (source segmented, text filter, sort select), virtualised table (# · Title with 40 px
  cover + artist · Album · Source · Date added · Duration, 56 px rows, sticky header, hover play
  icon, unavailable rows disabled with reason tooltip), incremental loading.
- Albums / Artists / Playlists: 180 px cover grids (artists circular), same toolbar.
- Every view: loading skeleton, empty, no results (+ Clear filter), error (+ Retry).
- `pnpm seed` builds a demo DB (1,200 tracks incl. long/Khmer titles, unavailable items, and a
  10,000-song liked list) that the app opens with `UNISON_DB`.
- Rust query tests; Vitest tests for each state; all checks green.
- Status (2026-11-19): built and committed; Liked/Albums/Artists/Playlists + sidebar playlists
  read the real DB. Waiting for the owner to check 60 fps scrolling on the 10k demo DB.

### UP-04 — Spotify capabilities
- `spotify_capabilities(refresh)` → { premium: true | false | null, devices } (device list always
  fresh; definite Premium answers cached 1 h). Premium probed only with an active device by
  re-setting repeat to its current value (PUT /me/player/repeat is documented Premium-only):
  204 → Premium, 403 mentioning Premium → not Premium, anything else → unknown. Deviation from
  the addendum: a free account with no Spotify app open reports `null` (unknown) rather than
  `false`, because Spotify does not document what it returns then.
- Settings → Playback → Spotify row: Ready / Needs Premium / Open Spotify… / unknown + devices.
- Status: built and tested with fixtures; waiting for the owner's live check.

### UP-05 — NativeEngine (local files)
Acceptance criteria:
- `NativeEngine` implements `Engine` on a dedicated audio thread (rodio 0.23 + symphonia):
  load / play / pause / stop / seek / volume (perceptual curve) / state (position, duration,
  playing, ended); ReplayGain as a gain factor.
- Formats via symphonia: MP3, FLAC, WAV, OGG Vorbis, AAC/M4A; opus and unknown types →
  UnsupportedFormat; missing file → FileMissing; output problems → OutputDeviceUnavailable.
- `player_outputs` lists output devices; `player_set_output` switches mid-track (resumes at the
  same position) and is saved in `player.outputDevice`.
- Settings → Playback: output device select + Test sound (quiet generated tone).
- Tests: generated WAV decode/duration/seek and can_play rules (no device needed); an
  ignored-by-default device test for play/seek/switch, run on the owner's PC.
- Status (2026-11-19): built; the real-device test passed on the owner's PC (play, seek to 2.5 s,
  output switch keeping position, pause/resume, end detected). Waiting for the owner to hear
  Test sound and to try real MP3/FLAC/OGG/M4A files (no encoder on the PC for generated ones).

### UP-09 — Resolver, fallback, wiring
Acceptance criteria:
- Resolver orders a track's sources by `player.sourcePriority`, skips unavailable sources and
  Spotify when `spotifyMode` is never; engines' `can_play` decide the rest.
- Player falls back to the next candidate when one can't play or fails to load (auto-fallback),
  and mid-track when the engine reports failure (`fallbackOnErrorMidtrack`). Every fallback
  writes playback_log (ended_reason = fallback) and emits `player://fallback`. Finished /
  skipped / error rows are logged too.
- `player_play(track_ids, start_index, force_source?)`, `player_control(action)`,
  `player_state`, `player_sources(track_id)`; `player://state` (track, source, status,
  position, duration, volume, shuffle, repeat) pushed on change and every 500 ms while playing.
- UI: Play / double-click / Enter in Liked songs and playlists; player bar shows the track,
  source and working controls (play/pause, next, previous, seek, volume, shuffle, repeat);
  Space toggles; NoPlayableSource toast offers Open in Spotify.
- Test matrix (fake engines): free Spotify falls to YouTube/local; blocked YouTube falls to
  local/Audius; only-Spotify track without Premium → NoPlayableSource.
- Status (2026-11-19): built and committed; all checks green incl. the fallback matrix. Waiting
  for the owner's live test (double-click a liked song with Spotify open).

### UP-11 — Drag-and-drop, Add files / Add folder, copy-on-drop
Acceptance criteria:
- Window drag-and-drop (Tauri `DragDropEvent`, handled in Rust): Enter/Leave → `local://drag`
  for a drop overlay; Drop → import job (same pipeline as UP-10). No HTML5 drag handling.
- `local_add_files` / `local_add_folder`: native dialogs (tauri-plugin-dialog, Rust-side only,
  no JS permissions; titles from en.json), audio-type filter from Settings → Local files;
  cancel → null. Folders are always referenced in place.
- Copy-on-drop (`local.copyOnDrop`, target = `local.copyTarget` or `<Music>/Unison`): drops and
  Add files copy first (folder layout kept, originals only read, same-size file at the target
  reused, name clashes get " (2)"), then import the copies; copy failures → `copyFailed` reason.
  Settings → Local files: switch + where copies go.
- Library → Local files (sidebar entry): `library_local` / `library_local_stats` (local DB),
  header with count/duration, live import progress, Play / Shuffle / Add files / Add folder,
  filter + sort, virtualised table with the "This PC" mark; loading, empty (with add buttons),
  no-results and error states.
- Import summary toast: added count, or per-file problems with Details → dialog listing each
  file and its reason (not found, format not supported, DRM, unreadable, couldn't be copied);
  "Already in your library" / "No music files found" / "Adding files stopped".
- Local covers: saved covers are served by an `artwork` URI scheme that only answers
  `<32 hex>.jpg` names from `<app data>/artwork` (CSP img-src allows it); new local-only tracks
  store that URL, merged tracks keep their Spotify art (gap-filled otherwise).
- Track lists carry every source kind (`TrackSource.provider: SourceKind`), so local / Audius
  marks show next to Spotify / YouTube.
- Owner's request (2026-11-20): a single click anywhere on a track row plays it (spec §3 said
  double-click); the hover play button still works and a click never starts the song twice.
- Status (2026-11-20): built; clippy -D warnings, 182 Rust + 109 Vitest tests green (Linux).
  Waiting for the owner's live check: drop a few MP3s / a folder (plus a .m4p or .txt) on the
  window, try Add files / Add folder, and turn on Copy dropped files once.

### UP-12 — Watched folders, rescan, missing files, Locate file…
Acceptance criteria:
- `watched_folders` table is the list (Settings `local.watchedFolders` mirrors it; settings_set
  can't change it). Commands: `local_folders_list`, `local_folder_add(path)` (imports the folder
  in the background), `local_folder_remove(path)` (songs already added stay),
  `local_rescan` → job id, `local_locate(track_id, new_path)`, plus native pickers
  `local_pick_folder` / `local_pick_file` (dialogs opened from Rust).
- A notify watcher (debounced 1.5 s) on every watched folder: a file added, changed, moved or
  deleted there updates the library within 5 s, quietly (no toast; lists refresh through
  `local://changed`). Restarted when folders change; started at launch with a quiet scan.
- Missing files: a file deleted from a watched folder leaves the library; any other missing
  file (or a whole watched folder that's gone, e.g. an unplugged drive) is kept greyed out with
  reason `file_missing` and a "Locate file…" action; a file that comes back plays again.
- Locate: same file → path updated; a different file → replaces the old one on the same song
  (keeps date added). DRM / unsupported / missing picks give a clear error.
- Rescan library: imports new files from every watched folder, runs the missing-file check,
  updates last-scan time, and reports the result (added / removed / up to date).
- UI: Settings → Local files lists watched folders (path, last scanned or "Folder not found",
  Remove), Add folder…, Rescan library with last-scan time. Track lists carry per-source
  `available`; on Local files a missing file's row says "File not found" and clicking it opens
  Locate.
- Tests: plan rules (remove / missing / restored / unplugged), relink, rescan job, a real
  watcher picking up an added and a deleted file within 5 s; Vitest for the settings list and
  Locate flow. All checks green.
- Notes: a missing file that isn't in a watched folder stays greyed out until it's located or
  comes back (there is no "Remove from library" yet; ideas list). The settings page gets file-type
  checkboxes in UP-14.
- Status (2026-11-20): built; clippy -D warnings, 187 Rust + 115 Vitest tests green (Linux).
  Waiting for the owner's live check: Settings → Local files → Add folder…, copy an MP3 into
  that folder (appears within ~5 s), delete it (disappears), move a dropped file and use
  Locate file… on its greyed row, then Rescan library.

## Done

- **UP-10 — Local import pipeline** (2026-11-20): `core/matching` with the §8.1 normaliser and
  §8.2 matcher (in-memory index per job; two precision additions for T-30 to review: titles
  with different numbers never merge, and " - Remastered 2009" suffixes count as noise);
  `core/local`: walk (configured extensions, no symlinked dirs), lofty tags + `Artist - Title`
  file-name fallback, fingerprint = size + BLAKE3(first 1 MB + last 64 KB), covers ≤ 600 px
  JPEG in `<app data>/artwork`, per-file reasons (missing / unsupportedFormat incl. opus /
  drmProtected .m4p / unreadable). Files merge into existing songs through the matcher, then
  `local_files`, `track_sources(local)` and `library(local, local)`; moved files are followed,
  second copies skipped, re-tagged files replaced. `local_import(paths)` → job id;
  background batches of 200 with `local://import-progress`, `local://changed` at the end,
  one job at a time. Tests: 1,000-file import + no-op re-import, liked-Spotify merge, ISRC
  merge, moved/copied/re-tagged files, bad-file reasons. Spotify sync still creates its own
  rows until T-31; local covers and marks show in the UI with UP-11.

- **UP-08 — SpotifyConnectEngine** (2026-11-19): remote control via PUT /me/player/play {uris,
  position_ms} on the chosen / active / first controllable device, pause / resume / seek /
  volume, 1 s polling of GET /me/player with the §8.6 ended rules; Premium 403 → not-Premium +
  SpotifyPremiumRequired, 404 → NoSpotifyDevice; `spotify_set_device` / `spotify_chosen_device`.
  6 wiremock tests. Live check happens with UP-09.

- **UP-07 — YouTubeEngine bridge** (2026-11-19): Rust `YouTubeEngine` (Engine, kind youtube) sends
  load/play/pause/seek/volume/stop as `player://youtube-command`; `youtube_player_report` feeds
  playing/paused/ended/error + position/duration back; `youtube_host_ready` gates can_play so
  YouTube never plays without the visible player. `YouTubeDock` (always mounted) shows the
  official player in a glass card (≥ 200×200), runs the commands on the IFrame API, reports
  state changes and the position every second. Tests: Rust engine with a recording sink,
  Vitest dock (commands → IFrame calls, events → reports, Settings test ignored). Live use
  comes with the resolver (UP-09) — the library has no YouTube items while YouTube sync is parked.

- **UP-06 — NativeEngine HTTP streaming** (2026-11-19): `HttpSource` (Read + Seek) with a download
  thread using Range requests into memory (never disk), restart on far seeks, 15 s stall timeout,
  error flag on dropped connections; NativeEngine plays http(s) URLs (Audius kind) and reports
  `failed`; `Engine::kinds` replaces `kind`. Tests against a local Range server (byte-exact,
  seek, drop → error, streamed WAV decode + seek); real-device stream/seek/drop tests passed on
  the owner's PC. A real Audius URL is verified with UP-13 (needs the H13 API key).

- **UP-03 — YouTube embed fix** (2026-11-19): https app scheme (https://tauri.localhost), nocookie
  embed with `origin` + strict-origin-when-cross-origin referrer, official IFrame API attached to
  our iframe (≥ 200×200), `player://youtube-command`, `youtube_test_playback` bridge with
  YoutubeEmbedBlocked{code} mapping, Settings → Playback test card, relay page template.
  Verified by the owner: the test video plays in the Windows release build — no relay needed.

- **UP-02 — Engines, queue, player state machine** (2026-11-19): `core/playback`: `Engine` and
  `Candidates` traits, `PlaySource`, `SourceStatus` + `UnavailableReason` and `PlayerSnapshot` in
  bindings; `Queue` of unified ids (manual vs auto next, shuffle keeping current and one pass per
  cycle, repeat off/all/one, play-next/append); `Player` picks the first engine that can play,
  pauses the previous engine on switch, one volume for all, restart-on-prev after 3 s,
  auto-advance, NoPlayableSource error state. New error codes from addendum §6 with messages.
  13 fake-engine/queue tests.

- **UP-01 — Universal Player migration + settings** (2026-11-19): migration
  `0003_universal_player.sql` (track_sources reason/quality/ReplayGain, local_files,
  watched_folders, playback_log); `SourceKind`, `SpotifyMode`, `ReplayGainMode`,
  `PlayerSettings` / `LocalSettings` / `AudiusSettings` with addendum defaults under settings keys
  `player` / `local` / `audius`; sanitising (priority deduped + completed, crossfade/volume
  clamps, https-only relay URL, extensions normalised); UI carries the sections through when it
  saves. Applied to the owner's DB by the running dev app.

- **T-15 — Playlist detail** (2026-11-19): `playlist_get` / `playlist_tracks` /
  `playlist_open_external`; owned/collaborative Spotify playlists load items on first open
  (Feb 2026 `item` shape, legacy `track` accepted; episodes and null items skipped; local files
  unavailable) and refetch after a snapshot change; followed playlists show metadata + Open in
  Spotify with the reason and never request items; owned playlists prefetch after each sync.
  Page with header, virtualised table and loading / empty / not-found / error states.
  Rust + Vitest tests (84 frontend tests total).

- **T-13 — Spotify sync** (2026-11-19): full sync profile → liked → playlists (metadata, Feb 2026
  `items` shape) → saved albums → followed artists → top/recent signals; per-page transactions;
  removals only after a complete page set (tracks kept); `sync://progress` / `sync://finished`
  events; one sync per provider, cancellable; Syncing status; runs after connect, on Sync now,
  and when older than the interval (failed attempts also wait an interval). Migration 0002 adds
  `playlists.track_count` / `owner_id`. Spotify pages may contain `null` entries (seen live in
  /me/albums) — dropped. Tests: 2,050-song sync + re-sync with removals. Live sync with the
  owner's account succeeded (24 items, then 0/0 on re-sync).

- **T-12 — Spotify client** (2026-11-19): `SpotifyClient` with single-flight token refresh
  (60 s margin + once on 401, rotated tokens persisted, invalid_grant → needs_reconnect),
  429 Retry-After (≤3 retries, >60 s → RateLimited), 5xx backoff 0.5/1/2/4 s, 4-request
  semaphore, `collect_pages` for offset and cursor paging. 9 wiremock tests.

- **T-11 — Spotify BYOK onboarding + connect** (2026-11-19): Settings → Accounts, 3-step drawer
  (dashboard, redirect URIs with copy, Client ID), keychain-only Client ID, PKCE loopback
  connect + `/me`, tokens in Credential Manager, account row in SQLite, sidebar shows the
  connected account, `UserNotRegistered` error (ADR 0003). Verified by the owner with a real
  Spotify account (manual check for T-10 too).

- **T-10 — PKCE, loopback, keychain** (2026-11-19): `core/auth`: PKCE S256 (64-byte verifier,
  32-byte state, RFC 7636 vector test, constant-time state check); one-shot 127.0.0.1 listener
  on preferred ports (Spotify) or any port (Google), ignores stray/forged requests, close-tab
  page, 5 min timeout, cancellable; generic OAuth client (authorize URL, code exchange,
  refresh keeping un-rotated refresh tokens, invalid_grant → NeedsReconnect, transport errors
  → Offline); keychain `SecretStore` (Credential Manager via keyring, memory store for tests),
  verified against the real Credential Manager. Manual browser check runs with T-11.

- **T-06 — Provider trait, models, settings, single instance** (2026-11-19): §5 models
  (ProviderId, SourceRef, Track, Playlist, Page, Account, Signal) in bindings; object-safe
  async `MusicProvider` trait; `Settings` stored per key as JSON with per-field fallback and
  sanitising; `settings_get/set`; UI hydrates before first paint and saves theme / frost /
  sidebar sections (debounced); window material read from settings at launch
  (`UNISON_MATERIAL` still overrides); single-instance focuses the running window (verified).
  27 Rust + 56 Vitest tests.

- **T-05 — Rust core** (2026-11-19): `AppError` + `ErrorCode` (§9) serialised as a tagged union;
  `tracing` to a daily file (+ stdout in debug) through a redacting writer (Authorization,
  token/code/key params, token JSON); SQLite pool (WAL, FKs, busy timeout) with embedded
  migration 0001 at `%APPDATA%/app.unison/unison.db`; tauri-specta bindings regenerated on every
  debug run and by `cargo test` (CI fails on drift); first command `app_info` shown in
  Settings → About via TanStack Query; UI error mapping in `src/lib/errors.ts`. Windows test
  binaries now embed the Common Controls manifest (build.rs). 20 Rust + 54 Vitest tests.
  Note for T-32: `tracks_fts` declares column `artists` but `tracks` has `artists_json`;
  fix with a new migration before using external-content reads/rebuild.

- **T-04 — Shell** (2026-11-19): undecorated window with custom glass title bar (drag,
  back/forward, Windows caption buttons), floating glass sidebar collapsing to icons below
  1100 px, floating glass player bar placeholder, all routes with empty states, shortcuts
  (Ctrl+K/L/, · Alt+arrows · Space), window size/position persisted via
  tauri-plugin-window-state. 47 Vitest tests; all checks green.

- **T-03a — Apple-inspired redesign** (2026-11-19, ADR 0002): main window built in Rust with
  Mica on Windows 11 (opaque fallback; `UNISON_MATERIAL=acrylic|off` override until T-71);
  label/fill/glass tokens with reduced-transparency fallback; artwork-adaptive accent with
  contrast correction; spring easings via CSS linear(); Inter-only type scale; kit restyled
  (capsules, glass overlays, sliding segmented thumb, floating sheet); /dev/ui materials stage,
  accent presets and frost slider. 30 Vitest + 6 Rust tests; all checks green.

- **T-03 — Design tokens + UI kit** (2026-11-19): role tokens (dark + light), bundled Inter /
  Space Grotesk / JetBrains Mono, Radix-based Button, IconButton, Input, Select, Segmented, Menu,
  Dialog, Drawer, Tooltip, Toast, Skeleton, Banner; `/dev/ui` gallery (dev builds only) with
  theme toggle. 16 Vitest tests; lint/typecheck green.

- **T-01 — Scaffold** (2026-11-19): Tauri 2 + React 19 + TS strict + Vite + Tailwind v4 + pnpm,
  §4 layout, role tokens, placeholder icon mark. lint/typecheck/test, clippy -D warnings and
  cargo test green; `pnpm tauri dev` opens the "Unison" window.

## Blocked

- **Phase 2 (T-20 – T-24) / H4–H6:** Google Cloud asked the owner for a $50 prepayment before
  the project could be used (2026-11-19). Not paying for now. YouTube work is parked; Spotify
  and provider-independent tasks continue. Options later: retry with another Google account,
  or revisit when budget allows. T-30 (matching) and T-41 (YouTube player) can still be built
  and tested with fixtures; live YouTube verification waits on this.
