# 0005 — Downloads only for open-licence sources

Date: 2026-11-21 · Status: accepted (owner)

## Context
The owner asked for a Download action. Most installed extensions front commercial streaming
services; saving their audio would copy content the services don't license for download, and
some (e.g. `signedSession@1` against `api.zarz.moe`) only work by calling a private API with a
signing secret. Those are refused (EXT-06) and stay refused.

## Decision
- A source can be downloaded only when its extension is on, needs no refused runtime feature,
  gives Unison an audio URL, and its search result declares an open licence Unison recognises:
  a Creative Commons licence (any of BY, BY-SA, BY-ND, BY-NC, BY-NC-SA, BY-NC-ND), CC0 or the
  public domain (`core/extensions/license.rs`). No licence, "all rights reserved", unknown
  licences or a bare `downloadable: true` flag are not enough.
- The gate is `ExtensionManager::download_license`; every download goes through it.
- The host fetches the same allowlisted stream URL it plays (hosts checked on every redirect),
  writes `<music folder>/Artist - Title.<ext>` via a `.part` file, never overwrites, caps at
  1 GB, and attaches the file to the same song as its local source. An extension's own
  `download()` is never called and scripts still get no file / ffmpeg APIs.
- Audius tracks whose artist allows downloads can join the same gate when UP-13 lands.

## Consequences
Download appears only on sources that report an open licence (CC catalogues). Extensions for
commercial services show no Download button. A licence an extension reports is trusted as
stated; an extension that lies about licences should be removed.
