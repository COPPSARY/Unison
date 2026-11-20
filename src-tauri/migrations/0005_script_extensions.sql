-- EXT-02: script extensions (manifest.json + index.js) run in a sandboxed JavaScript engine.
-- For kind = 'script': manifest_json holds Unison's ScriptManifest, script holds index.js,
-- repo_url is the package or GitHub link and commit_sha the package SHA-256 (64 hex) or the
-- GitHub commit (40 hex). Credentials an extension stores live in the OS keychain, not here.
ALTER TABLE extensions ADD COLUMN kind TEXT NOT NULL DEFAULT 'declarative';
ALTER TABLE extensions ADD COLUMN script TEXT;

-- Per-extension storage API values and cookie jar, kept between calls.
CREATE TABLE extension_state (
  extension_id TEXT PRIMARY KEY,
  storage_json TEXT NOT NULL DEFAULT '{}',
  cookies_json TEXT NOT NULL DEFAULT '{}'
);
