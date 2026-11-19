# 0003 — What Spotify connect errors the app can detect

Date: 2026-11-19 · Status: accepted

## Context
Spec T-11 asks that a wrong Client ID and a redirect URI mismatch "show their specific
errors". Checked against Spotify's live authorize endpoint (2026-11-19): an unauthenticated
request with an invalid `client_id` returns `303` to the login page, the same as a valid one.
Spotify validates the client and redirect URI only after the user signs in, and then shows
`INVALID_CLIENT: …` in the browser without ever redirecting back to the loopback listener.
The app therefore cannot observe either failure.

## Decision
- Validate the Client ID format locally (32 hex chars) → `InvalidClientId` inline, before
  opening the browser.
- While waiting for the callback, after 20 s show a hint naming both causes (check the Client
  ID; add all three redirect URIs exactly), with a Cancel button. The listener still times out
  after 5 minutes (`AuthTimeout`).
- `invalid_client` from the token endpoint (if it ever gets that far) → `InvalidClientId`.
- New code `UserNotRegistered`: Development Mode apps only allow users listed under User
  Management; such users can sign in but `GET /me` returns 403. The UI explains the fix.

## Consequences
`RedirectUriMismatch` stays in `ErrorCode` for completeness but is not produced by the
connect flow today.
