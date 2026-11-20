-- EXT-07: open licences extensions report for their results (keyed by source key
-- `<extension>:<item>`). A source can be downloaded only while it has a row here.
CREATE TABLE source_licenses (
  item_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  url TEXT,
  fetched_at INTEGER NOT NULL
);
