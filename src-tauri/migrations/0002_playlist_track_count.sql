-- 0002: playlist track count and owner id (needed for Playlist.trackCount / editable).
ALTER TABLE playlists ADD COLUMN track_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE playlists ADD COLUMN owner_id TEXT;
CREATE INDEX idx_library_kind ON library(provider, kind, added_at);
