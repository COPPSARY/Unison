//! Installed extensions (EXT-01 declarative, EXT-02 script), their saved state and the
//! availability of their saved sources.

use serde_json::{Map, Value};
use sqlx::SqlitePool;

use crate::core::extensions::manifest::Manifest;
use crate::core::extensions::script::ScriptManifest;
use crate::error::{AppError, AppResult};

/// Provider string of extension rows in `track_sources`.
pub const PROVIDER: &str = "extension";

const KIND_DECLARATIVE: &str = "declarative";
const KIND_SCRIPT: &str = "script";

/// What an installed extension is.
#[derive(Debug, Clone, PartialEq)]
pub enum Spec {
    /// extension.json: URL templates and JSON Pointers.
    Declarative(Box<Manifest>),
    /// manifest.json + index.js, run in the sandboxed engine.
    Script { manifest: Box<ScriptManifest>, code: String },
}

impl Spec {
    pub fn id(&self) -> &str {
        match self {
            Spec::Declarative(m) => &m.id,
            Spec::Script { manifest, .. } => &manifest.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Spec::Declarative(m) => &m.name,
            Spec::Script { manifest, .. } => &manifest.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Spec::Declarative(m) => &m.version,
            Spec::Script { manifest, .. } => &manifest.version,
        }
    }

    pub fn hosts(&self) -> &[String] {
        match self {
            Spec::Declarative(m) => &m.hosts,
            Spec::Script { manifest, .. } => &manifest.hosts,
        }
    }

    /// `requiredRuntimeFeatures` (none for declarative extensions).
    pub fn required_features(&self) -> &[String] {
        match self {
            Spec::Declarative(_) => &[],
            Spec::Script { manifest, .. } => &manifest.required_runtime_features,
        }
    }

    /// `Err(ExtensionUnsupportedFeature)` when it requires a feature Unison refuses.
    pub fn check_features(&self) -> AppResult<()> {
        crate::core::extensions::features::check(self.id(), self.required_features())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub spec: Spec,
    /// Stream quality the user picked (a `qualityOptions` id), if any.
    pub quality: Option<String>,
    pub repo_url: String,
    pub commit_sha: String,
    pub enabled: bool,
    pub installed_at: i64,
}

impl Installed {
    /// On, and every feature it requires is supported. The only gate for search and streams.
    pub fn usable(&self) -> bool {
        self.enabled && self.spec.check_features().is_ok()
    }
}

type Row = (String, String, String, bool, i64, String, Option<String>, Option<String>);

fn decode((manifest_json, repo_url, commit_sha, enabled, installed_at, kind, script, quality): Row) -> AppResult<Installed> {
    let bad = |e: serde_json::Error| AppError::internal(format!("stored extension manifest: {e}"));
    let spec = match kind.as_str() {
        KIND_SCRIPT => Spec::Script {
            manifest: Box::new(serde_json::from_str(&manifest_json).map_err(bad)?),
            code: script.ok_or_else(|| AppError::internal("stored script extension has no code"))?,
        },
        _ => Spec::Declarative(Box::new(serde_json::from_str(&manifest_json).map_err(bad)?)),
    };
    Ok(Installed { spec, quality, repo_url, commit_sha, enabled, installed_at })
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Installed>> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT manifest_json, repo_url, commit_sha, enabled, installed_at, kind, script, quality
         FROM extensions ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(decode).collect()
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<Installed>> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT manifest_json, repo_url, commit_sha, enabled, installed_at, kind, script, quality
         FROM extensions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    row.map(decode).transpose()
}

/// Marks every saved source of extension `id` playable or not.
async fn set_sources_available(pool: &SqlitePool, id: &str, available: bool) -> AppResult<()> {
    sqlx::query(
        "UPDATE track_sources SET available = ?1
         WHERE provider = ?2 AND substr(item_id, 1, length(?3) + 1) = ?3 || ':'",
    )
    .bind(available)
    .bind(PROVIDER)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Installs or replaces an extension (enabled).
pub async fn upsert(pool: &SqlitePool, spec: &Spec, repo_url: &str, sha: &str, now: i64) -> AppResult<()> {
    let encode = |e: serde_json::Error| AppError::internal(format!("encode manifest: {e}"));
    let (kind, json, script) = match spec {
        Spec::Declarative(m) => (KIND_DECLARATIVE, serde_json::to_string(&**m).map_err(encode)?, None),
        Spec::Script { manifest, code } => (KIND_SCRIPT, serde_json::to_string(&**manifest).map_err(encode)?, Some(code)),
    };
    sqlx::query(
        "INSERT INTO extensions (id, name, version, repo_url, commit_sha, manifest_json, enabled, installed_at, kind, script)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, version = excluded.version,
           repo_url = excluded.repo_url, commit_sha = excluded.commit_sha,
           manifest_json = excluded.manifest_json, enabled = 1, installed_at = excluded.installed_at,
           kind = excluded.kind, script = excluded.script",
    )
    .bind(spec.id())
    .bind(spec.name())
    .bind(spec.version())
    .bind(repo_url)
    .bind(sha)
    .bind(json)
    .bind(now)
    .bind(kind)
    .bind(script)
    .execute(pool)
    .await?;
    set_sources_available(pool, spec.id(), true).await
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    let done = sqlx::query("UPDATE extensions SET enabled = ?2 WHERE id = ?1")
        .bind(id)
        .bind(enabled)
        .execute(pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::not_found("extension not installed"));
    }
    set_sources_available(pool, id, enabled).await
}

/// Removes an extension and its saved state; its saved sources stay but can't play (never
/// substituted).
pub async fn remove(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM extensions WHERE id = ?1").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM extension_state WHERE extension_id = ?1").bind(id).execute(pool).await?;
    set_sources_available(pool, id, false).await
}

fn object(raw: &str) -> Map<String, Value> {
    serde_json::from_str(raw).unwrap_or_default()
}

/// A script extension's storage values and cookie jar.
pub async fn load_state(pool: &SqlitePool, id: &str) -> AppResult<(Map<String, Value>, Map<String, Value>)> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT storage_json, cookies_json FROM extension_state WHERE extension_id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(s, c)| (object(&s), object(&c))).unwrap_or_default())
}

pub async fn save_state(
    pool: &SqlitePool,
    id: &str,
    storage: &Map<String, Value>,
    cookies: &Map<String, Value>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO extension_state (extension_id, storage_json, cookies_json) VALUES (?1, ?2, ?3)
         ON CONFLICT(extension_id) DO UPDATE SET storage_json = excluded.storage_json,
           cookies_json = excluded.cookies_json",
    )
    .bind(id)
    .bind(Value::Object(storage.clone()).to_string())
    .bind(Value::Object(cookies.clone()).to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Registry links the user added, oldest first.
pub async fn registries(pool: &SqlitePool) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar("SELECT url FROM extension_registries ORDER BY added_at, url").fetch_all(pool).await?)
}

pub async fn add_registry(pool: &SqlitePool, url: &str, now: i64) -> AppResult<()> {
    sqlx::query("INSERT INTO extension_registries (url, added_at) VALUES (?1, ?2) ON CONFLICT(url) DO NOTHING")
        .bind(url)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_registry(pool: &SqlitePool, url: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM extension_registries WHERE url = ?1").bind(url).execute(pool).await?;
    Ok(())
}

/// Remembers the stream quality picked for an extension (`None` = its default).
pub async fn set_quality(pool: &SqlitePool, id: &str, quality: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE extensions SET quality = ?2 WHERE id = ?1").bind(id).bind(quality).execute(pool).await?;
    Ok(())
}
