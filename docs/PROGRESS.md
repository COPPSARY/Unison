# Progress

## In progress

### T-02 — CI workflow
Acceptance criteria:
- `.github/workflows/ci.yml` runs on push and pull request.
- Matrix: windows-latest, macos-latest, ubuntu-latest.
- Each job: pnpm lint, typecheck, test; cargo clippy -D warnings; cargo test.
- Each job builds the app and uploads the bundle as an artifact (Windows NSIS, macOS .app, Linux .deb).
- Green on all three OSes.
- Status: pushed (f25acf7); waiting for the owner to confirm the first run.

### T-11 — Spotify BYOK onboarding + connect
Acceptance criteria:
- Settings → Accounts lists Spotify and YouTube with status and connected name.
- Connect Spotify opens a 3-step drawer: open Developer Dashboard · redirect URIs with copy
  buttons · paste Client ID → Connect opens the browser.
- `spotify_set_client_id` validates the format (32 hex) and stores it only in the keychain;
  the UI only learns whether one is saved.
- `account_connect(spotify)` runs PKCE loopback (ports 43821–43823), exchanges the code,
  reads `GET /me`, stores tokens in the keychain and the account row in SQLite.
- Errors: bad format → InvalidClientId inline; wrong ID / redirect mismatch are only visible in
  the browser (Spotify validates after login) → a hint appears while waiting + Cancel;
  account not in User Management (403 on /me) → UserNotRegistered with the fix.
- Fixture tests for /me and the 403 case; UI tests for drawer validation and states.

## Done

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
