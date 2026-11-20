//! Source extensions.
//! - EXT-01 declarative: `extension.json` from GitHub. Unison fills its URL templates and reads
//!   fields with JSON Pointers.
//! - EXT-02 script: `manifest.json` + `index.js`, from a package file (`.spotiflac-ext` /
//!   `.sflx`) or a GitHub folder. index.js runs in a sandboxed QuickJS engine ([`script`]).
//!
//! Either kind talks only to the hosts it declares (checked on every redirect), and audio is
//! streamed through NativeEngine's in-memory HTTP source. Nothing is downloaded to disk.

pub mod client;
pub mod github;
pub mod manifest;
pub mod script;
pub mod songs;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use specta::Type;
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use url::Url;

use self::client::{ExtHttp, MAX_IMAGE_BYTES};
pub use self::github::RegistryListing;
use self::github::{GitHub, LinkKind, RepoLink, Source};
use self::manifest::{fill, Manifest, MappedItem};
use self::script::package::{self, MAX_PACKAGE_BYTES};
use self::script::runtime::{self, Env, ScriptState};
use self::script::{ScriptManifest, SEARCH_FUNCTIONS, STREAM_FUNCTION};
use crate::core::auth::keychain::{self, SecretStore};
use crate::core::now_unix;
use crate::core::db;
use crate::core::db::extensions::Spec;
use crate::core::db::library::TrackInput;
use crate::core::playback::native::StreamResolver;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::{SourceKind, Track, TrackSource};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
/// Script extensions get longer: their own code may make several requests.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(45);
const ARTWORK_TIMEOUT: Duration = Duration::from_secs(5);
/// Search results remembered so a click can save and play them.
const MAX_REMEMBERED: usize = 2000;
const SEARCH_LIMIT: u32 = 25;

/// What an extension adds to Unison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    /// Results in Search.
    Search,
    /// Plays its tracks.
    Play,
}

fn capabilities(spec: &Spec) -> Vec<Capability> {
    match spec {
        Spec::Declarative(_) => vec![Capability::Search, Capability::Play],
        Spec::Script { manifest, .. } => [(manifest.can_search(), Capability::Search), (manifest.can_stream(), Capability::Play)]
            .into_iter()
            .filter_map(|(yes, c)| yes.then_some(c))
            .collect(),
    }
}

/// An installed extension as shown in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub hosts: Vec<String>,
    pub repo_url: String,
    /// GitHub commit (40 hex) or package SHA-256 (64 hex).
    pub commit_sha: String,
    pub enabled: bool,
    /// Unix seconds.
    pub installed_at: i64,
    /// Runs its own code (index.js) in Unison's sandbox.
    pub runs_code: bool,
    pub capabilities: Vec<Capability>,
    /// What the extension says it is (`metadata_provider`, `download_provider`,
    /// `lyrics_provider`, …); empty for declarative extensions.
    pub types: Vec<String>,
    pub homepage: Option<String>,
    /// `data:` URL of its icon.
    pub icon: Option<String>,
}

/// What an install would add, shown before the user confirms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPreview {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// Every host the extension will contact (`*.domain` = any subdomain).
    pub hosts: Vec<String>,
    pub repo_url: String,
    /// What the install is pinned to (GitHub commit or package SHA-256); pass it back to
    /// `extension_install`.
    pub commit_sha: String,
    /// Version already installed under the same id, if any.
    pub installed_version: Option<String>,
    pub runs_code: bool,
    pub capabilities: Vec<Capability>,
    pub types: Vec<String>,
    pub homepage: Option<String>,
    pub icon: Option<String>,
}

/// One extension listed in a registry.json. Exactly one of `preview` / `error` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegistryItem {
    /// The entry's link (or id) as written in registry.json.
    pub entry: String,
    /// What the registry itself says about the entry (shown when it can't be previewed).
    pub listed: RegistryListing,
    pub preview: Option<ExtensionPreview>,
    pub error: Option<AppError>,
}

/// Everything a registry.json lists, previewed (nothing is stored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPreview {
    pub url: String,
    /// The commit registry.json was read at.
    pub commit_sha: String,
    pub items: Vec<RegistryItem>,
}

/// One extension's search answer. `tracks[].id` is the source key (`<extension>:<item>`), not a
/// library id; pass keys to `extension_save_tracks` before playing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionResults {
    pub extension_id: String,
    pub extension_name: String,
    pub tracks: Vec<Track>,
    pub error: Option<AppError>,
}

fn info(i: db::extensions::Installed) -> ExtensionInfo {
    let capabilities = capabilities(&i.spec);
    let (author, description, homepage, runs_code, types, icon) = match &i.spec {
        Spec::Declarative(m) => (m.author.clone(), m.description.clone(), m.homepage.clone(), false, Vec::new(), None),
        Spec::Script { manifest, .. } => (
            manifest.author.clone(),
            manifest.description.clone(),
            manifest.homepage.clone(),
            true,
            manifest.types.clone(),
            manifest.icon.clone(),
        ),
    };
    ExtensionInfo {
        id: i.spec.id().to_owned(),
        name: i.spec.name().to_owned(),
        version: i.spec.version().to_owned(),
        author,
        description,
        hosts: i.spec.hosts().to_vec(),
        repo_url: i.repo_url,
        commit_sha: i.commit_sha,
        enabled: i.enabled,
        installed_at: i.installed_at,
        runs_code,
        capabilities,
        types,
        homepage,
        icon,
    }
}

/// Splits a source key `<extension id>:<item id>`.
pub fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':').filter(|(ext, item)| !ext.is_empty() && !item.is_empty())
}

/// Remote artwork → `artwork://` URL cache (fetched only from the extension's hosts).
struct ArtCache {
    dir: Option<PathBuf>,
    urls: Mutex<HashMap<String, Option<String>>>,
}

impl ArtCache {
    fn cached(&self, remote: &str) -> Option<Option<String>> {
        self.urls.lock().ok()?.get(remote).cloned()
    }

    fn remember(&self, remote: String, local: Option<String>) {
        if let Ok(mut map) = self.urls.lock() {
            if map.len() > MAX_REMEMBERED {
                map.clear();
            }
            map.insert(remote, local);
        }
    }

    async fn fetch(self: Arc<Self>, http: ExtHttp, hosts: Vec<String>, remote: String) -> Option<String> {
        if let Some(hit) = self.cached(&remote) {
            return hit;
        }
        let dir = self.dir.clone()?;
        let url = Url::parse(&remote).ok()?;
        let bytes = tokio::time::timeout(ARTWORK_TIMEOUT, http.bytes(url, &hosts, MAX_IMAGE_BYTES)).await.ok()?.ok()?;
        let local = tokio::task::spawn_blocking(move || {
            crate::core::local::artwork::save_cover(&bytes, &dir).and_then(|p| crate::core::local::artwork::url_for(&p))
        })
        .await
        .ok()
        .flatten();
        self.remember(remote, local.clone());
        local
    }
}

/// Downloads a package and reads it (runs index.js once to list its functions). `expected`
/// pins the SHA-256 the user saw in the preview (or the registry's).
async fn fetch_package(http: &ExtHttp, url: &Url, expected: Option<&str>) -> AppResult<(Spec, String)> {
    let bytes = http.package(url.clone(), MAX_PACKAGE_BYTES).await.map_err(|e| match e.code {
        ErrorCode::NotFound => AppError::new(ErrorCode::ExtensionInvalid, "no package at that link"),
        _ => e,
    })?;
    let sha = package::sha256_hex(&bytes);
    if expected.is_some_and(|e| e != sha) {
        return Err(AppError::new(ErrorCode::ExtensionInvalid, "the package changed since it was checked"));
    }
    let ext = tokio::task::spawn_blocking(move || package::read(&bytes))
        .await
        .map_err(|e| AppError::internal(format!("package check: {e}")))??;
    Ok((Spec::Script { manifest: Box::new(ext.manifest), code: ext.code }, sha))
}

fn credentials_key(id: &str) -> String {
    format!("extension.{id}.credentials")
}

/// Runs one function of a script extension with its saved storage, cookies and credentials,
/// then saves what changed. `Ok(None)`: it registers none of `names`.
async fn run_script(
    db: &SqlitePool,
    secrets: &Arc<dyn SecretStore>,
    manifest: &ScriptManifest,
    code: &str,
    names: &[&'static str],
    args: Vec<Value>,
    cancel: Arc<AtomicBool>,
) -> AppResult<Option<Value>> {
    let (storage, cookies) = db::extensions::load_state(db, &manifest.id).await?;
    let key = credentials_key(&manifest.id);
    let credentials: Map<String, Value> = match keychain::get_json(secrets.as_ref(), &key) {
        Ok(found) => found.unwrap_or_default(),
        Err(e) => {
            tracing::warn!(extension = %manifest.id, error = %e, "extension credentials unreadable");
            Map::new()
        }
    };
    let env = Env {
        ext_id: manifest.id.clone(),
        hosts: manifest.hosts.clone(),
        settings: manifest.settings.clone(),
        state: ScriptState { storage, credentials: credentials.clone(), cookies },
        timeout: SCRIPT_TIMEOUT,
        cancel,
    };
    let (code, names) = (code.to_owned(), names.to_vec());
    let outcome = tokio::task::spawn_blocking(move || runtime::call(&code, env, &names, &args))
        .await
        .map_err(|e| AppError::internal(format!("script task: {e}")))?;
    let state = outcome.state;
    db::extensions::save_state(db, &manifest.id, &state.storage, &state.cookies).await?;
    if state.credentials != credentials {
        let saved = if state.credentials.is_empty() {
            secrets.delete(&key)
        } else {
            keychain::set_json(secrets.as_ref(), &key, &state.credentials)
        };
        if let Err(e) = saved {
            tracing::warn!(extension = %manifest.id, error = %e, "extension credentials not saved");
        }
    }
    outcome.result
}

pub struct ExtensionManager {
    db: SqlitePool,
    http: ExtHttp,
    github: GitHub,
    art: Arc<ArtCache>,
    /// Script extensions' credentials (OS keychain in the app).
    secrets: Arc<dyn SecretStore>,
    /// Last search results by source key, so saving never trusts metadata from the UI.
    remembered: Mutex<HashMap<String, TrackInput>>,
    /// Cancels the running search's script calls when a newer search starts (typing).
    search_cancel: Mutex<Arc<AtomicBool>>,
    /// Last merged search songs, by every source key they hold.
    songs: Mutex<HashMap<String, songs::Song>>,
}

impl ExtensionManager {
    /// `artwork_dir`: the `artwork://` cache folder (`None` = no artwork).
    pub fn new(db: SqlitePool, artwork_dir: Option<PathBuf>, secrets: Arc<dyn SecretStore>) -> AppResult<Self> {
        Self::with_github(db, artwork_dir, GitHub::public()?, secrets)
    }

    pub fn with_github(
        db: SqlitePool,
        artwork_dir: Option<PathBuf>,
        github: GitHub,
        secrets: Arc<dyn SecretStore>,
    ) -> AppResult<Self> {
        Ok(Self {
            db,
            http: ExtHttp::new()?,
            github,
            art: Arc::new(ArtCache { dir: artwork_dir, urls: Mutex::new(HashMap::new()) }),
            secrets,
            remembered: Mutex::new(HashMap::new()),
            search_cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
            songs: Mutex::new(HashMap::new()),
        })
    }

    pub async fn list(&self) -> AppResult<Vec<ExtensionInfo>> {
        Ok(db::extensions::list(&self.db).await?.into_iter().map(info).collect())
    }

    /// Reads the extension behind a GitHub or package link without storing anything.
    pub async fn preview(&self, link: &str) -> AppResult<ExtensionPreview> {
        if let Some(url) = package::parse_link(link) {
            let (spec, sha) = fetch_package(&self.http, &url, None).await?;
            return self.preview_of(spec, url.as_str(), sha).await;
        }
        let parsed = RepoLink::parse(link)?;
        let sha = self.github.resolve_sha(&self.http, &parsed).await?;
        let spec = self.github.spec_at(&self.http, &parsed, &sha).await?;
        self.preview_of(spec, link.trim(), sha).await
    }

    async fn preview_of(&self, spec: Spec, link: &str, sha: String) -> AppResult<ExtensionPreview> {
        let installed_version = db::extensions::get(&self.db, spec.id()).await?.map(|i| i.spec.version().to_owned());
        let i = info(db::extensions::Installed {
            spec,
            quality: None,
            repo_url: link.to_owned(),
            commit_sha: sha,
            enabled: true,
            installed_at: 0,
        });
        Ok(ExtensionPreview {
            id: i.id,
            name: i.name,
            version: i.version,
            author: i.author,
            description: i.description,
            hosts: i.hosts,
            repo_url: i.repo_url,
            commit_sha: i.commit_sha,
            installed_version,
            runs_code: i.runs_code,
            capabilities: i.capabilities,
            types: i.types,
            homepage: i.homepage,
            icon: i.icon,
        })
    }

    /// Reads a registry.json link and previews every extension it lists, in parallel. One
    /// broken entry never hides the others; each repository/ref is resolved to a SHA once.
    pub async fn registry(&self, link: &str) -> AppResult<RegistryPreview> {
        let (parsed, kind) = RepoLink::parse_any(link)?;
        if kind != LinkKind::Registry {
            return Err(AppError::new(ErrorCode::ExtensionLinkInvalid, "not a registry.json link"));
        }
        let sha = self.github.resolve_sha(&self.http, &parsed).await?;
        let entries = self.github.registry_at(&self.http, &parsed, &sha).await?;

        // Resolve each distinct (owner, repo, ref) once; the registry's own is already known.
        type RefKey = (String, String, Option<String>);
        let key = |l: &RepoLink| -> RefKey { (l.owner.clone(), l.repo.clone(), l.git_ref.clone()) };
        let mut shas: HashMap<RefKey, AppResult<String>> = HashMap::new();
        shas.insert(key(&parsed), Ok(sha.clone()));
        let mut resolving = JoinSet::new();
        for e in &entries {
            if let Ok((_, Source::Repo(l))) = &e.link {
                let k = key(l);
                if !shas.contains_key(&k) {
                    // Placeholder until the task below answers, so each key is resolved once.
                    shas.insert(k.clone(), Err(AppError::internal("pending")));
                    let (gh, http, l) = (self.github.clone(), self.http.clone(), l.clone());
                    resolving.spawn(async move { (k, gh.resolve_sha(&http, &l).await) });
                }
            }
        }
        while let Some(joined) = resolving.join_next().await {
            let (k, r) = joined.map_err(|e| AppError::internal(format!("registry task: {e}")))?;
            shas.insert(k, r);
        }

        let mut specs = JoinSet::new();
        for (index, e) in entries.iter().enumerate() {
            let (gh, http) = (self.github.clone(), self.http.clone());
            match &e.link {
                Ok((_, Source::Repo(l))) => {
                    let sha = match shas.get(&key(l)) {
                        Some(Ok(sha)) => sha.clone(),
                        _ => continue,
                    };
                    let l = l.clone();
                    specs.spawn(async move {
                        let s = tokio::time::timeout(SEARCH_TIMEOUT, gh.spec_at(&http, &l, &sha))
                            .await
                            .unwrap_or_else(|_| Err(AppError::new(ErrorCode::Offline, "GitHub timed out")));
                        (index, s.map(|s| (s, sha)))
                    });
                }
                Ok((_, Source::Package { url, sha256 })) => {
                    let (url, sha256) = (url.clone(), sha256.clone());
                    specs.spawn(async move {
                        let s = tokio::time::timeout(SCRIPT_TIMEOUT, fetch_package(&http, &url, sha256.as_deref()))
                            .await
                            .unwrap_or_else(|_| Err(AppError::new(ErrorCode::Offline, "package download timed out")));
                        (index, s)
                    });
                }
                Err(_) => {}
            }
        }
        let mut found: HashMap<usize, AppResult<(Spec, String)>> = HashMap::new();
        while let Some(joined) = specs.join_next().await {
            let (index, r) = joined.map_err(|e| AppError::internal(format!("registry task: {e}")))?;
            found.insert(index, r);
        }

        let mut items = Vec::with_capacity(entries.len());
        for (index, e) in entries.into_iter().enumerate() {
            let result = match e.link {
                Err(err) => Err(err),
                Ok((install_link, source)) => match (found.remove(&index), source) {
                    (Some(Ok((spec, sha))), _) => self.preview_of(spec, &install_link, sha).await,
                    (Some(Err(err)), _) => Err(err),
                    (None, Source::Repo(l)) => match shas.get(&key(&l)) {
                        Some(Err(err)) => Err(err.clone()),
                        _ => Err(AppError::internal("registry entry was not read")),
                    },
                    (None, Source::Package { .. }) => Err(AppError::internal("registry entry was not read")),
                },
            };
            let (preview, error) = match result {
                Ok(p) => (Some(p), None),
                Err(err) => {
                    tracing::warn!(entry = %e.entry, error = %err, "registry entry failed");
                    (None, Some(err))
                }
            };
            items.push(RegistryItem { entry: e.entry, listed: e.listed, preview, error });
        }
        Ok(RegistryPreview { url: link.trim().to_owned(), commit_sha: sha, items })
    }

    /// Registries the user added on the Extensions page, oldest first.
    pub async fn registries(&self) -> AppResult<Vec<String>> {
        db::extensions::registries(&self.db).await
    }

    /// Reads a registry link and, when it works, remembers it.
    pub async fn add_registry(&self, link: &str) -> AppResult<RegistryPreview> {
        let preview = self.registry(link).await?;
        db::extensions::add_registry(&self.db, &preview.url, now_unix()).await?;
        Ok(preview)
    }

    pub async fn remove_registry(&self, link: &str) -> AppResult<Vec<String>> {
        db::extensions::remove_registry(&self.db, link.trim()).await?;
        self.registries().await
    }

    /// Installs the previewed extension at `sha` (re-read and re-checked; a package must still
    /// have the SHA-256 the preview showed).
    pub async fn install(&self, link: &str, sha: &str) -> AppResult<Vec<ExtensionInfo>> {
        let (spec, url) = if let Some(url) = package::parse_link(link) {
            if !package::valid_sha256(sha) {
                return Err(AppError::new(ErrorCode::ExtensionLinkInvalid, "invalid package SHA-256"));
            }
            (fetch_package(&self.http, &url, Some(sha)).await?.0, url.to_string())
        } else {
            let parsed = RepoLink::parse(link)?;
            (self.github.spec_at(&self.http, &parsed, sha).await?, link.trim().to_owned())
        };
        db::extensions::upsert(&self.db, &spec, &url, sha, now_unix()).await?;
        tracing::info!(extension = %spec.id(), version = %spec.version(), "extension installed");
        self.list().await
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> AppResult<Vec<ExtensionInfo>> {
        db::extensions::set_enabled(&self.db, id, enabled).await?;
        self.list().await
    }

    pub async fn remove(&self, id: &str) -> AppResult<Vec<ExtensionInfo>> {
        db::extensions::remove(&self.db, id).await?;
        if let Err(e) = self.secrets.delete(&credentials_key(id)) {
            tracing::warn!(extension = %id, error = %e, "extension credentials not removed");
        }
        tracing::info!(extension = %id, "extension removed");
        self.list().await
    }

    /// Searches every enabled extension that can search, in parallel; one failing never hides
    /// the others.
    pub async fn search(&self, query: &str) -> AppResult<Vec<ExtensionResults>> {
        let query = query.trim();
        let enabled: Vec<Spec> = db::extensions::list(&self.db)
            .await?
            .into_iter()
            .filter(|i| i.enabled && capabilities(&i.spec).contains(&Capability::Search))
            .map(|i| i.spec)
            .collect();
        // A newer search replaces this one: older script calls stop downloading.
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut current) = self.search_cancel.lock() {
            current.store(true, Ordering::Relaxed);
            *current = cancel.clone();
        }
        if query.is_empty() || enabled.is_empty() {
            return Ok(Vec::new());
        }
        let mut set = JoinSet::new();
        for (order, spec) in enabled.into_iter().enumerate() {
            let (http, art, q) = (self.http.clone(), self.art.clone(), query.to_owned());
            let (db, secrets, cancel) = (self.db.clone(), self.secrets.clone(), cancel.clone());
            set.spawn(async move {
                let limit = match &spec {
                    Spec::Declarative(_) => SEARCH_TIMEOUT,
                    Spec::Script { .. } => SCRIPT_TIMEOUT + Duration::from_secs(5),
                };
                let result = match tokio::time::timeout(limit, search_one(&http, &art, &db, &secrets, &spec, &q, cancel)).await {
                    Ok(r) => r,
                    Err(_) => Err(AppError::new(ErrorCode::Offline, "extension search timed out")),
                };
                (order, spec, result)
            });
        }
        let mut out = Vec::new();
        while let Some(joined) = set.join_next().await {
            let (order, spec, result) = joined.map_err(|e| AppError::internal(format!("search task: {e}")))?;
            let (tracks, error) = match result {
                Ok(items) => {
                    self.remember(&items);
                    (items.iter().map(|(i, art)| as_track(i, art.clone())).collect(), None)
                }
                Err(e) => {
                    tracing::warn!(extension = %spec.id(), error = %e, "extension search failed");
                    (Vec::new(), Some(e))
                }
            };
            let (extension_id, extension_name) = (spec.id().to_owned(), spec.name().to_owned());
            out.push((order, ExtensionResults { extension_id, extension_name, tracks, error }));
        }
        out.sort_by_key(|(order, _)| *order);
        Ok(out.into_iter().map(|(_, r)| r).collect())
    }

    fn remember(&self, items: &[(MappedItem, Option<String>)]) {
        if let Ok(mut map) = self.remembered.lock() {
            if map.len() + items.len() > MAX_REMEMBERED {
                map.clear();
            }
            for (item, art) in items {
                let mut track = item.track.clone();
                track.artwork = art.clone();
                map.insert(track.item_id.clone(), track);
            }
        }
    }

    /// Saves remembered search results to the library tables and returns their track ids
    /// (same order), so `player_play` can queue them.
    pub async fn save_tracks(&self, keys: &[String]) -> AppResult<Vec<String>> {
        let inputs: Vec<TrackInput> = {
            let map = self.remembered.lock().map_err(|_| AppError::internal("extension results lock poisoned"))?;
            keys.iter()
                .map(|k| map.get(k).cloned().ok_or_else(|| AppError::not_found("search result expired; search again")))
                .collect::<AppResult<_>>()?
        };
        let now = now_unix();
        let mut tx = self.db.begin().await?;
        let mut ids = Vec::with_capacity(inputs.len());
        for input in &inputs {
            ids.push(db::library::upsert_track(&mut tx, db::extensions::PROVIDER, input, now).await?);
        }
        tx.commit().await?;
        Ok(ids)
    }

    async fn enabled(&self, ext_id: &str) -> AppResult<db::extensions::Installed> {
        match db::extensions::get(&self.db, ext_id).await? {
            Some(i) if i.enabled => Ok(i),
            _ => Err(AppError::new(ErrorCode::ExtensionDisabled, "extension is off or removed")),
        }
    }

    /// The allowlisted audio URL for a source key, resolved now (stream URLs may expire).
    pub async fn resolve_stream(&self, key: &str) -> AppResult<String> {
        let (ext_id, item_id) =
            split_key(key).ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "bad extension source key"))?;
        let installed = self.enabled(ext_id).await?;
        let chosen = installed.quality;
        let tier = db::settings::load(&self.db).await?.player.stream_quality;
        let (url, hosts) = match installed.spec {
            Spec::Declarative(m) => (self.declarative_stream(&m, item_id).await?, m.hosts),
            Spec::Script { manifest, code } => {
                if !manifest.can_stream() {
                    return Err(AppError::new(ErrorCode::ExtensionUnsupported, "extension has no getDownloadUrl"));
                }
                let quality = script::effective_quality(&manifest, chosen.as_deref(), tier)
                    .or_else(|| manifest.quality.clone())
                    .unwrap_or_else(|| "LOSSLESS".to_owned());
                let answer =
                    run_script(
                        &self.db,
                        &self.secrets,
                        &manifest,
                        &code,
                        &[STREAM_FUNCTION],
                        vec![json!(item_id), json!(quality)],
                        Arc::new(AtomicBool::new(false)),
                    )
                        .await?
                        .ok_or_else(|| AppError::new(ErrorCode::ExtensionUnsupported, "extension has no getDownloadUrl"))?;
                let url = script::stream_url(&answer)?;
                manifest::check_url(&url, &manifest.hosts)?;
                (url, manifest.hosts)
            }
        };
        Ok(self.http.final_stream_url(url, &hosts).await?.to_string())
    }

    async fn declarative_stream(&self, m: &Manifest, item_id: &str) -> AppResult<Url> {
        match (&m.stream.url, &m.stream.resolve) {
            (Some(template), _) => fill(template, "id", item_id, &m.hosts),
            (None, Some(r)) => {
                let body = self.http.json(fill(&r.url, "id", item_id, &m.hosts)?, &m.hosts).await?;
                let raw = body
                    .pointer(&r.field)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "no stream URL in answer"))?;
                let url = Url::parse(raw)
                    .map_err(|e| AppError::new(ErrorCode::ExtensionFailed, format!("bad stream URL: {e}")))?;
                manifest::check_url(&url, &m.hosts)?;
                Ok(url)
            }
            (None, None) => Err(AppError::new(ErrorCode::ExtensionInvalid, "manifest has no stream")),
        }
    }
}

async fn search_one(
    http: &ExtHttp,
    art: &Arc<ArtCache>,
    db: &SqlitePool,
    secrets: &Arc<dyn SecretStore>,
    spec: &Spec,
    query: &str,
    cancel: Arc<AtomicBool>,
) -> AppResult<Vec<(MappedItem, Option<String>)>> {
    let items = match spec {
        Spec::Declarative(m) => {
            let body = http.json(fill(&m.search.url, "query", query, &m.hosts)?, &m.hosts).await?;
            m.map_search(&body)?
        }
        Spec::Script { manifest, code } => {
            let args = vec![json!(query), json!(SEARCH_LIMIT)];
            match run_script(db, secrets, manifest, code, &SEARCH_FUNCTIONS, args, cancel).await? {
                Some(body) => script::map_search(&manifest.id, &body)?,
                None => Vec::new(),
            }
        }
    };
    // Artwork in parallel; a missing cover never fails a search.
    let mut covers = JoinSet::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(remote) = item.artwork_remote.clone() {
            let (art, http, hosts) = (art.clone(), http.clone(), spec.hosts().to_vec());
            covers.spawn(async move { (index, art.fetch(http, hosts, remote).await) });
        }
    }
    let mut found: Vec<Option<String>> = vec![None; items.len()];
    while let Some(Ok((index, local))) = covers.join_next().await {
        if let Some(slot) = found.get_mut(index) {
            *slot = local;
        }
    }
    Ok(items.into_iter().zip(found).collect())
}

fn as_track(item: &MappedItem, artwork: Option<String>) -> Track {
    let t = &item.track;
    Track {
        id: t.item_id.clone(),
        title: t.title.clone(),
        artists: t.artists.clone(),
        album: t.album.clone(),
        duration_ms: t.duration_ms,
        artwork,
        isrc: t.isrc.clone(),
        sources: vec![TrackSource {
            provider: SourceKind::Extension,
            item_id: t.item_id.clone(),
            url: t.url.clone(),
            available: true,
        }],
        available: true,
        added_at: None,
    }
}

#[async_trait]
impl StreamResolver for ExtensionManager {
    async fn available(&self, key: &str) -> bool {
        match split_key(key) {
            Some((ext, _)) => self.enabled(ext).await.is_ok(),
            None => false,
        }
    }

    async fn resolve(&self, key: &str) -> AppResult<String> {
        self.resolve_stream(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::auth::keychain::MemoryStore;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    async fn setup() -> (tempfile::TempDir, MockServer, ExtensionManager) {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();
        let server = MockServer::start().await;
        let base = Url::parse(&format!("{}/", server.uri())).unwrap();
        let gh = GitHub { api: base.clone(), raw: base };
        let mgr = ExtensionManager::with_github(pool, None, gh, Arc::new(MemoryStore::default())).unwrap();
        (dir, server, mgr)
    }

    async fn publish(server: &MockServer, repo: &str, manifest: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path(format!("/repos/acme/{repo}/commits/main")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "sha": SHA })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/acme/{repo}/{SHA}/extension.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest))
            .mount(server)
            .await;
    }

    fn manifest(id: &str, base: &str, prefix: &str) -> serde_json::Value {
        let mut m = manifest::tests::sample(&format!("{base}/{prefix}"), "127.0.0.1");
        m["id"] = json!(id);
        m["name"] = json!(id.to_uppercase());
        m
    }

    #[tokio::test]
    async fn preview_install_search_save_and_stream() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        publish(&server, "good", manifest("good-ext", &base, "good")).await;
        publish(&server, "bad", manifest("bad-ext", &base, "bad")).await;
        Mock::given(method("GET"))
            .and(path("/good/search"))
            .and(query_param("q", "hello world"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [
                { "id": "t1", "title": "One", "user": { "name": "A" }, "duration": 10 },
                { "id": "t2", "title": "Two", "user": { "name": "B" } }
            ]})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/bad/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/good/tracks/t1/stream"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/cdn/t1.mp3"))
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/cdn/t1.mp3")).respond_with(ResponseTemplate::new(206)).mount(&server).await;

        let link = "https://github.com/acme/good/tree/main";
        let preview = mgr.preview(link).await.unwrap();
        assert_eq!((preview.id.as_str(), preview.commit_sha.as_str()), ("good-ext", SHA));
        assert_eq!(preview.hosts, vec!["127.0.0.1"]);
        assert!(!preview.runs_code);
        assert_eq!(preview.capabilities, vec![Capability::Search, Capability::Play]);
        assert!(mgr.list().await.unwrap().is_empty(), "preview stores nothing");

        mgr.install(link, SHA).await.unwrap();
        let list = mgr.install("https://github.com/acme/bad/tree/main", SHA).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(mgr.preview(link).await.unwrap().installed_version.as_deref(), Some("1.0.0"));

        let results = mgr.search(" hello world ").await.unwrap();
        assert_eq!(results.len(), 2);
        let good = results.iter().find(|r| r.extension_id == "good-ext").unwrap();
        let bad = results.iter().find(|r| r.extension_id == "bad-ext").unwrap();
        assert_eq!(good.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["good-ext:t1", "good-ext:t2"]);
        assert_eq!(good.tracks[0].duration_ms, Some(10_000));
        assert!(good.error.is_none());
        assert_eq!(bad.error.as_ref().map(|e| e.code.clone()), Some(ErrorCode::Upstream { status: 500 }));

        let ids = mgr.save_tracks(&["good-ext:t1".to_owned()]).await.unwrap();
        assert_eq!(ids.len(), 1);
        let again = mgr.save_tracks(&["good-ext:t1".to_owned()]).await.unwrap();
        assert_eq!(ids, again, "saving twice keeps one track row");
        assert!(mgr.save_tracks(&["good-ext:zzz".to_owned()]).await.is_err());

        let stream = mgr.resolve_stream("good-ext:t1").await.unwrap();
        assert!(stream.ends_with("/cdn/t1.mp3"), "{stream}");
        assert!(mgr.available("good-ext:t1").await);

        // Off → not available, sources greyed out, results hidden.
        mgr.set_enabled("good-ext", false).await.unwrap();
        assert!(!mgr.available("good-ext:t1").await);
        assert_eq!(mgr.resolve_stream("good-ext:t1").await.unwrap_err().code, ErrorCode::ExtensionDisabled);
        let sources = crate::core::playback::resolver::stored_sources(&mgr.db, &ids[0]).await.unwrap();
        assert!(sources.iter().all(|s| !s.available));
        assert!(mgr.search("hello world").await.unwrap().iter().all(|r| r.extension_id != "good-ext"));

        mgr.set_enabled("good-ext", true).await.unwrap();
        let sources = crate::core::playback::resolver::stored_sources(&mgr.db, &ids[0]).await.unwrap();
        assert!(sources.iter().all(|s| s.available));

        mgr.remove("good-ext").await.unwrap();
        assert_eq!(mgr.list().await.unwrap().len(), 1);
        let sources = crate::core::playback::resolver::stored_sources(&mgr.db, &ids[0]).await.unwrap();
        assert!(sources.iter().all(|s| !s.available && s.source.kind == SourceKind::Extension));
    }

    #[tokio::test]
    async fn resolves_stream_through_a_resolve_endpoint() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        let mut m = manifest("res-ext", &base, "r");
        m["stream"] = json!({ "resolve": { "url": format!("{base}/r/resolve/{{id}}"), "field": "/stream/url" } });
        publish(&server, "res", m).await;
        Mock::given(method("GET"))
            .and(path("/r/resolve/a%20b"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "stream": { "url": format!("{base}/audio/ab.mp3") } })),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/audio/ab.mp3")).respond_with(ResponseTemplate::new(200)).mount(&server).await;
        mgr.install("https://github.com/acme/res/tree/main", SHA).await.unwrap();
        let url = mgr.resolve_stream("res-ext:a b").await.unwrap();
        assert!(url.ends_with("/audio/ab.mp3"), "{url}");
    }

    /// A packaged script extension that searches and streams through the mock server, whose
    /// address comes in through a setting default.
    fn script_package(base: &str, code: &str) -> Vec<u8> {
        let manifest = json!({
            "name": "mock-stream", "displayName": "Mock Stream", "version": "2.0.0", "author": "Someone",
            "permissions": { "network": ["127.0.0.1"], "storage": true },
            "type": ["metadata_provider", "download_provider"],
            "qualityOptions": [{ "id": "HI_RES", "label": "Hi-Res" }],
            "settings": [{ "key": "base", "type": "string", "label": "Base", "default": base }]
        });
        script::package::tests::build(&[("manifest.json", &manifest.to_string()), ("index.js", code)])
    }

    const SCRIPT: &str = r#"
        let cfg = {};
        registerExtension({
          initialize(config) { cfg = config; return true; },
          searchTracks(query, limit) {
            storage.set("last", query);
            const r = http.get(cfg.base + "/s/search?q=" + encodeURIComponent(query));
            return JSON.parse(r.body).items.slice(0, limit).map((i) => ({
              id: i.id, name: i.title, artists: i.artist, duration_ms: i.ms, album_name: i.album,
            }));
          },
          getDownloadUrl(id, quality) {
            return { success: true, url: cfg.base + "/s/audio/" + id + ".flac?q=" + quality, format: "flac" };
          },
          download() { return { success: false, error: "never called" }; },
        });
    "#;

    #[tokio::test]
    async fn installs_searches_and_streams_a_script_package() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        let pkg = script_package(&base, SCRIPT);
        let digest = package::sha256_hex(&pkg);
        Mock::given(method("GET"))
            .and(path("/pkg/mock.spotiflac-ext"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/s/search"))
            .and(query_param("q", "hi there"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "items": [
                { "id": "t1", "title": "One", "artist": "A, B", "ms": 123000, "album": "LP" },
                { "id": "t2", "title": "Two", "artist": "C" }
            ]})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/s/audio/t1.flac"))
            .and(query_param("q", "HI_RES"))
            .respond_with(ResponseTemplate::new(206))
            .mount(&server)
            .await;

        let link = format!("{base}/pkg/mock.spotiflac-ext");
        let preview = mgr.preview(&link).await.unwrap();
        assert_eq!((preview.id.as_str(), preview.name.as_str(), preview.version.as_str()), ("mock-stream", "Mock Stream", "2.0.0"));
        assert_eq!(preview.commit_sha, digest);
        assert!(preview.runs_code);
        assert_eq!(preview.capabilities, vec![Capability::Search, Capability::Play]);
        assert!(mgr.list().await.unwrap().is_empty(), "preview stores nothing");

        let wrong = "f".repeat(64);
        assert_eq!(mgr.install(&link, &wrong).await.unwrap_err().code, ErrorCode::ExtensionInvalid, "pinned hash");
        let list = mgr.install(&link, &digest).await.unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].runs_code && list[0].enabled);

        let results = mgr.search("hi there").await.unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert!(r.error.is_none(), "{:?}", r.error);
        assert_eq!(r.tracks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["mock-stream:t1", "mock-stream:t2"]);
        assert_eq!(r.tracks[0].artists, vec!["A", "B"]);
        assert_eq!(r.tracks[0].duration_ms, Some(123_000));
        let (storage, _) = db::extensions::load_state(&mgr.db, "mock-stream").await.unwrap();
        assert_eq!(storage.get("last"), Some(&json!("hi there")), "storage persists");

        let ids = mgr.save_tracks(&["mock-stream:t1".to_owned()]).await.unwrap();
        assert_eq!(ids.len(), 1);
        let stream = mgr.resolve_stream("mock-stream:t1").await.unwrap();
        assert!(stream.ends_with("/s/audio/t1.flac?q=HI_RES"), "{stream}");

        mgr.remove("mock-stream").await.unwrap();
        assert_eq!(db::extensions::load_state(&mgr.db, "mock-stream").await.unwrap().0.len(), 0, "state removed");
    }

    #[tokio::test]
    async fn script_extensions_without_search_or_stream_stay_out_of_the_way() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        let pkg = script_package(&base, "registerExtension({ download() { return { success: false }; } });");
        let digest = package::sha256_hex(&pkg);
        Mock::given(method("GET"))
            .and(path("/pkg/dl.sflx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
            .mount(&server)
            .await;
        let link = format!("{base}/pkg/dl.sflx");
        let preview = mgr.preview(&link).await.unwrap();
        assert!(preview.capabilities.is_empty(), "download-only adds nothing Unison uses");
        mgr.install(&link, &digest).await.unwrap();
        assert!(mgr.search("anything").await.unwrap().is_empty(), "not shown in Search");
        assert_eq!(mgr.resolve_stream("mock-stream:t1").await.unwrap_err().code, ErrorCode::ExtensionUnsupported);
    }

    #[tokio::test]
    async fn previews_every_extension_in_a_registry() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        publish(&server, "good", manifest("good-ext", &base, "good")).await;
        let pkg = script_package(&base, SCRIPT);
        let digest = package::sha256_hex(&pkg);
        Mock::given(method("GET"))
            .and(path("/pkg/mock.spotiflac-ext"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
            .mount(&server)
            .await;
        // The registry repo: registry.json plus one extension in a sub-folder, same commit.
        Mock::given(method("GET"))
            .and(path("/repos/acme/hub/commits/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "sha": SHA })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/acme/hub/{SHA}/registry.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "extensions": [
                "https://github.com/acme/good/tree/main",
                { "url": "sources/inner" },
                "https://github.com/acme/missing/tree/main",
                "https://example.com/nope",
                { "id": "mock-stream", "download_url": format!("{base}/pkg/mock.spotiflac-ext"), "sha256": digest },
                { "id": "tampered", "download_url": format!("{base}/pkg/mock.spotiflac-ext?v=2"), "sha256": "0".repeat(64) }
            ]})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/acme/hub/{SHA}/sources/inner/extension.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest("inner-ext", &base, "inner")))
            .mount(&server)
            .await;

        let reg = mgr.registry("https://raw.githubusercontent.com/acme/hub/main/registry.json").await.unwrap();
        assert_eq!(reg.commit_sha, SHA);
        let ids: Vec<Option<&str>> = reg.items.iter().map(|i| i.preview.as_ref().map(|p| p.id.as_str())).collect();
        assert_eq!(ids, [Some("good-ext"), Some("inner-ext"), None, None, Some("mock-stream"), None]);
        assert!(reg.items[2].error.is_some() && reg.items[3].error.is_some());
        assert_eq!(reg.items[3].error.as_ref().map(|e| e.code.clone()), Some(ErrorCode::ExtensionUnsupported));
        assert_eq!(reg.items[5].error.as_ref().map(|e| e.code.clone()), Some(ErrorCode::ExtensionInvalid), "sha256 mismatch");
        assert!(mgr.list().await.unwrap().is_empty(), "previewing a registry stores nothing");

        // Every previewed entry installs through the normal path.
        for p in reg.items.iter().filter_map(|i| i.preview.as_ref()) {
            mgr.install(&p.repo_url, &p.commit_sha).await.unwrap();
        }
        assert_eq!(mgr.list().await.unwrap().len(), 3);

        // Saved registries: added only once it reads, listed, removed.
        let link = "https://raw.githubusercontent.com/acme/hub/main/registry.json";
        assert!(mgr.registries().await.unwrap().is_empty(), "previewing doesn't save");
        assert_eq!(mgr.add_registry(link).await.unwrap().items.len(), 6);
        mgr.add_registry(link).await.unwrap();
        assert_eq!(mgr.registries().await.unwrap(), vec![link]);
        assert!(mgr.add_registry("https://raw.githubusercontent.com/acme/none/main/registry.json").await.is_err());
        assert_eq!(mgr.registries().await.unwrap().len(), 1, "a broken registry isn't saved");
        assert!(mgr.remove_registry(link).await.unwrap().is_empty());
        assert_eq!(
            mgr.registry("https://github.com/acme/good").await.unwrap_err().code,
            ErrorCode::ExtensionLinkInvalid,
            "an extension link isn't a registry"
        );
    }

    #[tokio::test]
    async fn install_rejects_bad_links_and_unknown_shas() {
        let (_dir, _server, mgr) = setup().await;
        assert_eq!(mgr.preview("https://example.com/x").await.unwrap_err().code, ErrorCode::ExtensionLinkInvalid);
        assert_eq!(
            mgr.install("https://github.com/acme/good", "not-a-sha").await.unwrap_err().code,
            ErrorCode::ExtensionLinkInvalid
        );
        assert_eq!(
            mgr.install("https://github.com/acme/missing", SHA).await.unwrap_err().code,
            ErrorCode::ExtensionInvalid
        );
        assert_eq!(
            mgr.install("https://example.com/x.spotiflac-ext", SHA).await.unwrap_err().code,
            ErrorCode::ExtensionLinkInvalid,
            "a package needs its SHA-256"
        );
    }

    #[tokio::test]
    async fn a_newer_search_cancels_the_older_one() {
        let (_dir, server, mgr) = setup().await;
        let base = server.uri();
        let code = r#"registerExtension({ searchTracks(q) {
            if (q === "slow") { utils.sleep(10000); if (utils.isRequestCancelled()) throw new Error("cancelled"); }
            return [{ id: q, name: q }];
        } });"#;
        let pkg = script_package(&base, code);
        let digest = package::sha256_hex(&pkg);
        Mock::given(method("GET"))
            .and(path("/pkg/slow.sflx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
            .mount(&server)
            .await;
        mgr.install(&format!("{base}/pkg/slow.sflx"), &digest).await.unwrap();

        let mgr = Arc::new(mgr);
        let started = std::time::Instant::now();
        let older = tokio::spawn({
            let mgr = mgr.clone();
            async move { mgr.search("slow").await }
        });
        tokio::time::sleep(Duration::from_millis(400)).await;
        let newer = mgr.search("fast").await.unwrap();
        assert_eq!(newer[0].tracks[0].title, "fast");
        let older = older.await.unwrap().unwrap();
        assert!(older[0].error.is_some(), "the older search was cancelled");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    /// Two script packages that both find "Song One"; the second offers two qualities.
    async fn two_sources(server: &MockServer, mgr: &ExtensionManager) {
        let base = server.uri();
        for (name, qualities) in [("store-a", json!([])), ("store-b", json!([{ "id": "LOSSLESS", "label": "Lossless" }, { "id": "HI_RES", "label": "Hi-Res" }]))] {
            let manifest = json!({
                "name": name, "version": "1.0.0",
                "permissions": { "network": ["127.0.0.1"] }, "type": ["metadata_provider"],
                "qualityOptions": qualities,
                "settings": [{ "key": "base", "type": "string", "label": "Base", "default": base }]
            });
            let code = format!(r#"let cfg = {{}};
                registerExtension({{
                  initialize(c) {{ cfg = c; return true; }},
                  searchTracks(q) {{ return [{{ id: "{name}-1", name: "Song One", artists: "Artist", duration_ms: 200000, album_name: "Album" }}]; }},
                  getDownloadUrl(id, quality) {{ return {{ success: true, url: cfg.base + "/audio/" + id + "?q=" + quality }}; }},
                }});"#);
            let pkg = script::package::tests::build(&[("manifest.json", &manifest.to_string()), ("index.js", &code)]);
            let digest = package::sha256_hex(&pkg);
            Mock::given(method("GET"))
                .and(path(format!("/pkg/{name}.sflx")))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(pkg))
                .mount(server)
                .await;
            mgr.install(&format!("{base}/pkg/{name}.sflx"), &digest).await.unwrap();
        }
        Mock::given(method("GET")).and(path("/audio/store-b-1")).respond_with(ResponseTemplate::new(206)).mount(server).await;
    }

    #[tokio::test]
    async fn merged_search_song_page_play_from_a_source_and_like() {
        let (_dir, server, mgr) = setup().await;
        two_sources(&server, &mgr).await;

        let all = mgr.search_all("song").await.unwrap();
        assert!(all.failures.is_empty(), "{:?}", all.failures);
        assert_eq!(all.songs.len(), 1, "one song from two extensions");
        let song = &all.songs[0];
        let keys: Vec<&str> = song.sources.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["store-a:store-a-1", "store-b:store-b-1"]);
        assert_eq!(song.sources[1].qualities.len(), 2);
        assert_eq!(song.sources[1].quality.as_deref(), Some("LOSSLESS"));

        let detail = mgr.song("store-b:store-b-1").await.unwrap();
        assert_eq!((detail.song.key.as_str(), detail.liked, detail.track_id.clone()), ("store-a:store-a-1", false, None));

        // Play from the second source at Hi-Res: it streams first, at that quality.
        let id = mgr.prepare_play(&song.key, "store-b:store-b-1", Some("HI_RES")).await.unwrap();
        let candidates = crate::core::playback::resolver::Resolver { db: mgr.db.clone() };
        use crate::core::playback::engine::Candidates;
        let order = candidates.candidates(&id).await.unwrap();
        assert_eq!(order.iter().map(|c| c.item_id.as_str()).collect::<Vec<_>>(), ["store-b:store-b-1", "store-a:store-a-1"]);
        let url = mgr.resolve_stream("store-b:store-b-1").await.unwrap();
        assert!(url.ends_with("/audio/store-b-1?q=HI_RES"), "{url}");
        // An unknown quality is ignored: no match for the High tier here, so the extension's default.
        mgr.prepare_play(&song.key, "store-b:store-b-1", Some("8K")).await.unwrap();
        assert_eq!(mgr.song(&song.key).await.unwrap().song.sources[1].quality.as_deref(), Some("LOSSLESS"));
        mgr.prepare_play(&song.key, "store-b:store-b-1", Some("HI_RES")).await.unwrap();
        assert_eq!(mgr.song(&song.key).await.unwrap().song.sources[1].quality.as_deref(), Some("HI_RES"));

        // Like → Liked songs, Albums and Artists; the song page reads back from the library.
        assert!(mgr.like_song(&song.key, true).await.unwrap());
        let detail = mgr.song("store-b:store-b-1").await.unwrap();
        assert!(detail.liked);
        assert_eq!(detail.track_id.as_deref(), Some(id.as_str()));
        mgr.songs.lock().unwrap().clear();
        let stored = mgr.song("store-a:store-a-1").await.unwrap();
        assert_eq!(stored.song.sources.len(), 2, "both sources were saved on one track");
        assert_eq!(stored.song.sources[0].key, "store-b:store-b-1", "the picked source comes first");
        let liked = crate::core::db::views::liked(&mgr.db, None, crate::model::LibrarySort::DateAdded, None, 0, 10).await.unwrap();
        assert_eq!(liked.total, 1);
        let albums = crate::core::db::views::albums(&mgr.db, None, crate::model::LibrarySort::Title, None).await.unwrap();
        assert_eq!(albums.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Album"]);
        assert!(!mgr.like_song(&song.key, false).await.unwrap());
        assert!(crate::core::db::views::albums(&mgr.db, None, crate::model::LibrarySort::Title, None).await.unwrap().is_empty());
    }
}
