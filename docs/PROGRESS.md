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

### EXT-01 — Extensions (owner-approved 2026-11-20)
Acceptance criteria:
- An extension is a declarative `extension.json` (schema 1): id, name, version, hosts allowlist,
  a search endpoint with JSON Pointer field mapping, and a stream URL (direct or resolved).
  No extension code runs inside Unison.
- Settings → Extensions: paste a GitHub repo / tree / blob link → preview (name, author,
  version, pinned commit, every host it will contact) → Install. Nothing is stored before
  confirming. Installed list with enable switch and remove.
- Rejected with a clear message: bad link, invalid manifest, non-https URL, a host not in the
  allowlist (also on redirects), oversized bodies.
- Search shows a section per enabled extension; a single click plays the row through
  NativeEngine (streamed, Range seeking, nothing written to disk). The stream URL is resolved
  at play time.
- Disabling or removing an extension marks its saved sources unavailable; never substituted.
- Tests: manifest validation + mapping, GitHub link parsing, mocked install pinned to a SHA,
  host allowlist incl. redirects, search with one failing extension, stream resolution,
  resolver ordering; Vitest for the Settings section and Search sections. All checks green.
- Status (2026-11-20): built; clippy -D warnings, 200 Rust + 123 Vitest tests green (Linux).
  Not committed yet. Waiting for the owner's live check with a real extension repo.

### EXT-02 — Script extensions (owner-approved 2026-11-20)
Acceptance criteria:
- Packaged extensions (`.spotiflac-ext` / `.sflx` / `.zip` with manifest.json + index.js) install
  from a pasted https link or a registry.json entry (`download_url`, pinned to its `sha256` when
  given). A GitHub folder with manifest.json + index.js installs the same way, pinned to a commit.
  A manifest.json with a `unison` section installs as a declarative extension instead.
- index.js runs in a sandboxed QuickJS context per call: 128 MB memory, 30 s deadline, no files,
  processes or modules. HTTP only to `permissions.network` hosts (`*.domain` wildcards), checked
  on every redirect, bodies capped at 10 MB, per-extension cookie jar.
- Documented API: registerExtension, http.*, fetch, storage (SQLite), credentials (OS keychain),
  log/console, utils (base64, md5, sha1/256, HMAC, random), URL/URLSearchParams, TextEncoder/
  Decoder, atob/btoa, gobackend.getLocalTime, matching.*. file.*, ffmpeg.* and OAuth browser
  sign-in answer "not available in Unison"; Unison never calls download().
- `searchTracks` / `customSearch` feed Search; `getDownloadUrl(id, quality)` gives the URL that
  NativeEngine streams (checked against the hosts, nothing written to disk). Extensions with
  neither still install, and the preview says they add nothing Unison can use.
- Preview shows hosts, what it adds (search / playback), "runs code", and the package SHA-256
  that install re-checks.
- Status (2026-11-20): built; migration 0005, clippy -D warnings, 220 Rust + 128 Vitest tests
  green (Linux). Not committed. Not covered: extension settings UI (initialize gets manifest
  defaults), lyrics, home feed, album/artist pages, OAuth sign-in.

### EXT-03 — Extensions replace Spotify and YouTube (owner-approved 2026-11-20)
Acceptance criteria:
- Spotify and YouTube are removed: providers, OAuth/PKCE, accounts, sync, the YouTube IFrame
  engine and relay, Spotify Connect, their commands, error codes, settings and UI. Migration 0006
  deletes their synced data (tracks kept when another source has them); their keychain entries
  are deleted at startup. Old settings load (unknown sources are dropped from the priority).
- Sidebar → Extensions page: add registry links (saved, removable) or one extension link
  (preview → install); Installed list and one list per registry, each row with icon, name,
  version and Install / Update / Installed.
- Extension details page: icon, name, author, version, Install / Update, on/off, Remove; About;
  Tags (registry category + tags); Capabilities (manifest types explained + what works in
  Unison); Information (version, author, website, runs code, hosts, source, pin, installed date,
  registry).
- Package icons become data: URLs; the CSP no longer allows Spotify/YouTube hosts or frames.
- Status (2026-11-20): built; clippy -D warnings, 149 Rust + 108 Vitest tests green (Linux).
  Not committed. Library pages (Liked, Albums, Artists, Playlists) stay but only fill from local
  files, Audius and extensions; nothing writes liked/saved rows yet.

### EXT-04 — Unified search, song page, likes (owner-approved 2026-11-20)
Acceptance criteria:
- Search shows one song list from every enabled extension, not a section per extension. The
  same song from several extensions (equal ISRC or the §8.2 matcher) shows once with all its
  sources; extensions that fail are listed under the results with Retry.
- A click on a result opens the song page (/track/:key); the artwork's play button plays it.
- Song page: artwork, title, artists, album, length; Play and Like. Sources tab lists every
  extension with the song and the stream qualities it offers (manifest qualityOptions); the
  picked source plays first (track_sources.preference) and the picked quality is remembered per
  extension. Download-only sources show "Can't play in Unison". Details tab: artists, album,
  length, ISRC, sources. No downloading.
- Like/unlike: the song (all its sources on one library track) goes to Liked songs; its album
  and primary artist appear under Albums and Artists and leave when no liked song has them.
- Runtime fixes from live testing: utils.randomUserAgent / sleep / isRequestCancelled, 20 s
  connect timeout, 45 s per call, gzip/brotli/deflate answers, newer searches cancel older ones,
  per-request deadlines (async HTTP, interruptible while waiting and downloading).
- Status (2026-11-20): built; migration 0007, clippy -D warnings, 155 Rust + 113 Vitest tests
  green (Linux). Not committed. Not covered yet: making playlists, extension settings UI
  (logins / server URLs), "*" network permission (Jellyfin).

### EXT-05 — Play metadata-only songs through a streaming extension (owner-approved 2026-11-21)
Acceptance criteria:
- A song whose sources can't stream (metadata-only extensions, no getDownloadUrl) plays when
  another enabled extension can: at play time Rust searches the extensions that can both search
  and stream for "<artist> <title>", accepts a hit only on equal ISRC or the strict §8.2 merge,
  attaches it to the same tracks row and plays it. Never a different song; a miss is a clear
  error, and the miss is remembered for 10 minutes so repeated presses don't search again.
- Metadata-only sources are skipped at once (not loaded and failed).
- getDownloadUrl may answer `{ url, headers?, mimeType? }`: allowlisted headers (User-Agent,
  Referer, Origin, Authorization, Cookie) go with the stream requests, dropped on a redirect to
  another host, never logged; mimeType is the decoder hint.
- Stream requests send Accept-Encoding: identity; a 200 to a Range request skips ahead to the
  wanted offset; a 401/403/404/410 on open re-resolves the URL once. The extension's probe
  passes the length on, so the player doesn't fetch a signed URL a second time.
- Errors say what failed: stream-open failures are ExtensionFailed (not "Audius isn't
  reachable"); when every source fails, the last real error is returned.
- The automatic Stream quality tier never picks Opus or Dolby (E-AC-3/AC-4/Atmos) options,
  which this build can't decode; a user's own pick is unchanged.
- Idea (not code) from the owner's Android app: catalogs give metadata, audio-source extensions
  are asked at play time. That app is GPL-3.0; nothing is copied from it.
- Live fix (2026-11-21): getDownloadUrl stubs that answer null (Amazon, Deezer, SoundCloud,
  YouTube Music in the Mornye store) are learnt as download-only and skipped; every strict match
  is attached so a stub can't hide a real streamer; "only downloads" never ends up as the final
  error (NoPlayableSource instead, reworded).
- Live check (ignored test `live_play_through_installed_extensions`, on a copy of the owner's DB):
  "Arijit Singh Tum Hi Ho" resolves through JioSaavn and decodes (AAC, 262 s). GMENGZ songs:
  JioSaavn doesn't carry them; every service that does is download-only or encrypted here.
- Status (2026-11-21): built; clippy -D warnings, 164 Rust + 115 Vitest tests green (Windows).
  Not committed.

### EXT-06 — Refuse extensions that need unsupported runtime features (CLAUDE.md, 2026-11-21)
Acceptance criteria:
- manifest.json `requiredRuntimeFeatures` (list of names) is read and stored with the extension.
- The supported set is declared in code (`core/extensions/features.rs`, `SUPPORTED`, empty for
  now). `signedSession@1`, `sessionGrant@1`, `preparedContext@1` and anything that names DRM /
  decryption / encrypted AC-4 or E-AC-3 are refused by name; unknown features are unsupported too.
- Install and turning on fail with `ExtensionUnsupportedFeature { extension, feature }`
  ("Unison won't turn on amazon: it needs signedSession@1 …"). A stored extension that needs a
  refused feature shows as off and is never searched or streamed (`Installed::usable`).
- Extensions page: preview, registry rows and details say which feature is refused; Install and
  the on/off switch are disabled.
- No `download()` call exists, so nothing new is wired there; when one is added it must go
  through `Installed::usable()` + the `download_provider` type check in the resolver.
- Tests: feature rules, manifest parsing, install / enable / search / stream refusal for an
  `amazon` package with `signedSession@1`, Vitest for the disabled registry row and the message.
- Status (2026-11-21): built; clippy -D warnings, 170 Rust + 116 Vitest tests green (Windows).
  Note: extensions installed before this change were stored without `requiredRuntimeFeatures`;
  reinstall or update them so the check sees their manifest.

### EXT-07 — Download open-licence songs (owner-approved 2026-11-21, ADR 0005)
Acceptance criteria:
- Extension results may declare a licence (script: `license` / `license_url` / `licenseUrl`;
  declarative: `fields.license` pointer). Creative Commons, CC0 and public domain are
  recognised (`core/extensions/license.rs`); each search saves or clears it per source key
  (migration 0008 `source_licenses`).
- Gate (`download_license`): extension on + no refused runtime feature + gives audio + open
  licence; otherwise `DownloadNotAllowed` / `ExtensionDisabled` / `ExtensionUnsupportedFeature`.
- `extension_download(key, sourceKey)` → job id; background job fetches the allowlisted stream
  URL (hosts checked on redirects, 1 GB cap, 30 min limit), writes `Artist - Title.<ext>` into
  the music folder (Settings → Local files → where copies go) via a `.part` file, never
  overwrites, and attaches it to the same song as its local file. `download://progress`
  reports progress and the outcome. An extension's `download()` is never called.
- Song page: sources with a licence show "Licence: …" and Download (with progress); a toast
  says where the file went or why it failed.
- Tests: licence parsing, file naming / type detection, end-to-end gate + save + attach +
  licence withdrawn + extension off; Vitest for the Download button and the refusal message.
- Status (2026-11-21): built; clippy -D warnings, 177 Rust + 118 Vitest tests green (Windows).
  Waiting for the owner's live check with an extension that reports CC licences (e.g. a
  Jamendo or Internet Archive extension). None of the installed ones do yet.

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
