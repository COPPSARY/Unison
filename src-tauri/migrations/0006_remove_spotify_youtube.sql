-- Spotify and YouTube were removed (owner decision 2026-11-20): extensions are the music
-- sources. Drop their synced data; tracks that only came from them go too. Local files,
-- Audius and extension tracks (and Unison playlists) stay.
DELETE FROM library WHERE provider IN ('spotify', 'youtube');
DELETE FROM playlists WHERE provider IN ('spotify', 'youtube');
DELETE FROM playlist_items WHERE source_provider IN ('spotify', 'youtube');
DELETE FROM albums WHERE provider IN ('spotify', 'youtube');
DELETE FROM artists WHERE provider IN ('spotify', 'youtube');
DELETE FROM track_sources WHERE provider IN ('spotify', 'youtube');
DELETE FROM playback_log WHERE provider IN ('spotify', 'youtube');
DELETE FROM playlist_items WHERE track_id NOT IN (SELECT track_id FROM track_sources);
DELETE FROM tracks WHERE id NOT IN (SELECT track_id FROM track_sources);
DELETE FROM accounts;
DELETE FROM sync_state;
DELETE FROM quota_usage;
DELETE FROM signals;
DELETE FROM settings WHERE key IN ('syncIntervalMin', 'preferredSource');

-- Registries the user added on the Extensions page.
CREATE TABLE extension_registries (
  url TEXT PRIMARY KEY,
  added_at INTEGER NOT NULL
);
