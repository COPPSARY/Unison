-- EXT-01: declarative source extensions installed from GitHub. Their tracks use
-- track_sources rows with provider = 'extension' and item_id = '<extension id>:<item id>'.
CREATE TABLE extensions (
  id TEXT PRIMARY KEY,                 -- manifest id, ^[a-z0-9-]{3,40}$
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  repo_url TEXT NOT NULL,              -- the GitHub link the user pasted
  commit_sha TEXT NOT NULL,            -- manifest pinned to this commit
  manifest_json TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  installed_at INTEGER NOT NULL
);
