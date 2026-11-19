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

## Done

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
