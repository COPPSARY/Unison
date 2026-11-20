-- Track page (EXT-04): the stream quality chosen per extension, and which source of a song the
-- user picked (higher preference plays first among sources of the same kind).
ALTER TABLE extensions ADD COLUMN quality TEXT;
ALTER TABLE track_sources ADD COLUMN preference INTEGER NOT NULL DEFAULT 0;
