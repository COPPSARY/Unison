-- 0003: Universal Player (addendum §6; "0002_universal_player" in the addendum, renumbered
-- because 0002 was already applied). Providers now: 'spotify' | 'youtube' | 'audius' | 'local'.
ALTER TABLE track_sources ADD COLUMN unavailable_reason TEXT; -- not_embeddable | file_missing | premium_required | gated | ...
ALTER TABLE track_sources ADD COLUMN quality TEXT;            -- e.g. 'mp3-320', 'flac', 'stream'
ALTER TABLE track_sources ADD COLUMN replaygain_track REAL;
ALTER TABLE track_sources ADD COLUMN replaygain_album REAL;

CREATE TABLE local_files (
  fingerprint TEXT PRIMARY KEY,           -- size + BLAKE3(first 1 MB + last 64 KB)
  path TEXT NOT NULL UNIQUE,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  codec TEXT,
  sample_rate INTEGER,
  bitrate INTEGER,
  artwork_path TEXT,
  imported_at INTEGER NOT NULL
);

CREATE TABLE watched_folders (path TEXT PRIMARY KEY, last_scan_at INTEGER);

CREATE TABLE playback_log (                -- feeds signals + debugging fallbacks
  id INTEGER PRIMARY KEY,
  track_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  played_ms INTEGER NOT NULL,
  ended_reason TEXT                        -- finished | skipped | error | fallback
);
CREATE INDEX idx_playback_log_track ON playback_log(track_id, started_at);
