-- 0001_init.sql — initial schema (spec §7). Never edit once applied; add a new file instead.
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
