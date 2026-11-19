# Progress

## In progress

### T-02 — CI workflow
Acceptance criteria:
- `.github/workflows/ci.yml` runs on push and pull request.
- Matrix: windows-latest, macos-latest, ubuntu-latest.
- Each job: pnpm lint, typecheck, test; cargo clippy -D warnings; cargo test.
- Each job builds the app and uploads the bundle as an artifact (Windows NSIS, macOS .app, Linux .deb).
- Green on all three OSes.

## Done

- **T-01 — Scaffold** (2026-11-19): Tauri 2 + React 19 + TS strict + Vite + Tailwind v4 + pnpm,
  §4 layout, role tokens, placeholder icon mark. lint/typecheck/test, clippy -D warnings and
  cargo test green; `pnpm tauri dev` opens the "Unison" window.

## Blocked
