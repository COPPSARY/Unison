# 0001 — Bring-your-own Spotify Client ID

Date: 2026-11-19 · Status: accepted

## Context
Spotify Development Mode apps (since Feb 2026) need a Premium developer account and allow
only 5 authorized users. Extended Quota requires a registered business with 250k+ MAU.

## Decision
Each user creates their own Spotify developer app and pastes its Client ID into Unison.
Auth uses PKCE, so no client secret is needed. The Client ID is stored only in the OS
keychain (`app.unison` / `spotify.client_id`), never in SQLite, logs or frontend state.

## Consequences
Onboarding needs a 3-step guide (spec §3, Accounts & BYOK onboarding) with the exact
redirect URIs `http://127.0.0.1:43821/callback`, `…43822`, `…43823`.
