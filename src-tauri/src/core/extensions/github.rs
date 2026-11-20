//! GitHub links → a manifest pinned to one commit.
//! Accepted: `github.com/{owner}/{repo}`, `…/tree/{ref}[/{dir}]`, `…/blob/{ref}/[{dir}/]{file}`
//! and `raw.githubusercontent.com/{owner}/{repo}/[refs/heads/]{ref}/[{dir}/]{file}`, where
//! `{file}` is extension.json or manifest.json (one extension) or registry.json (a list of
//! extension links). The ref is resolved to a commit SHA with the REST API (`GET /repos/{o}/{r}`
//! for the default branch, `GET /repos/{o}/{r}/commits/{ref}` for the SHA), then the folder's
//! extension.json is read from raw.githubusercontent.com at that SHA. When the folder has none,
//! its manifest.json (a script extension with a `unison` section) is read instead.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use url::Url;

use super::client::ExtHttp;
use super::manifest::{Manifest, MAX_MANIFEST_BYTES};
use super::script::{self, package, ScriptExtension, MAX_SCRIPT_BYTES};
use crate::core::db::extensions::Spec;
use crate::error::{AppError, AppResult, ErrorCode};

pub const MANIFEST_FILE: &str = "extension.json";
/// A script extension's manifest (read when a folder has no extension.json).
pub const SCRIPT_MANIFEST_FILE: &str = "manifest.json";
/// A script extension's code, next to its manifest.json.
pub const SCRIPT_FILE: &str = "index.js";
pub const REGISTRY_FILE: &str = "registry.json";
/// Largest registry.json read.
pub const MAX_REGISTRY_BYTES: usize = 256 * 1024;
/// Most extensions one registry may list.
pub const MAX_REGISTRY_ENTRIES: usize = 50;

/// What a pasted link points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Extension,
    Registry,
}

/// A parsed GitHub link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLink {
    pub owner: String,
    pub repo: String,
    /// Branch, tag or SHA; `None` = default branch.
    pub git_ref: Option<String>,
    /// Folder holding extension.json, without leading/trailing slashes (`""` = repo root).
    pub dir: String,
}

fn link_invalid(why: &str) -> AppError {
    AppError::new(ErrorCode::ExtensionLinkInvalid, why)
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn valid_segment(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains('\\')
}

impl RepoLink {
    /// Parses a link to one extension (registry links are rejected).
    pub fn parse(input: &str) -> AppResult<RepoLink> {
        match Self::parse_any(input)? {
            (link, LinkKind::Extension) => Ok(link),
            (_, LinkKind::Registry) => Err(link_invalid("that is a registry link, not an extension")),
        }
    }

    /// Parses an extension or registry link; `dir` is the folder holding the file.
    pub fn parse_any(input: &str) -> AppResult<(RepoLink, LinkKind)> {
        let trimmed = input.trim();
        let with_scheme = if trimmed.starts_with("github.com/") || trimmed.starts_with("raw.githubusercontent.com/") {
            format!("https://{trimmed}")
        } else {
            trimmed.to_owned()
        };
        let url = Url::parse(&with_scheme).map_err(|_| link_invalid("not a URL"))?;
        let raw = match url.host_str() {
            Some("github.com" | "www.github.com") => false,
            Some("raw.githubusercontent.com") => true,
            _ => return Err(link_invalid("not a GitHub link")),
        };
        if url.scheme() != "https" {
            return Err(link_invalid("not an https link"));
        }
        let segs: Vec<&str> = url.path_segments().map(|s| s.filter(|p| !p.is_empty()).collect()).unwrap_or_default();
        let (owner, repo) = match segs.as_slice() {
            [o, r, ..] => (*o, r.trim_end_matches(".git")),
            _ => return Err(link_invalid("link needs an owner and a repository")),
        };
        if !valid_name(owner) || !valid_name(repo) {
            return Err(link_invalid("invalid owner or repository name"));
        }
        let rest = &segs[2..];
        let (git_ref, dir_segs, file): (Option<&str>, &[&str], &str) = if raw {
            let (r, path) = match rest {
                ["refs", "heads" | "tags", r, path @ ..] => (*r, path),
                [r, path @ ..] => (*r, path),
                [] => return Err(link_invalid("raw links need a branch and a file")),
            };
            match path.split_last() {
                Some((file, dir)) => (Some(r), dir, *file),
                None => return Err(link_invalid("raw links must point at extension.json or registry.json")),
            }
        } else {
            match rest {
                [] => (None, &[], MANIFEST_FILE),
                ["tree", r, dir @ ..] => (Some(*r), dir, MANIFEST_FILE),
                ["blob", r, path @ ..] => match path.split_last() {
                    Some((file, dir)) => (Some(*r), dir, *file),
                    None => return Err(link_invalid("blob links must point at extension.json or registry.json")),
                },
                _ => return Err(link_invalid("unsupported GitHub link")),
            }
        };
        let kind = match file {
            MANIFEST_FILE | SCRIPT_MANIFEST_FILE => LinkKind::Extension,
            REGISTRY_FILE => LinkKind::Registry,
            _ => return Err(link_invalid("link must point at extension.json, manifest.json or registry.json")),
        };
        if git_ref.is_some_and(|r| !valid_segment(r)) {
            return Err(link_invalid("invalid branch or tag"));
        }
        if !dir_segs.iter().all(|s| valid_segment(s)) {
            return Err(link_invalid("invalid folder in link"));
        }
        let link = RepoLink {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            git_ref: git_ref.map(str::to_owned),
            dir: dir_segs.join("/"),
        };
        Ok((link, kind))
    }

    /// A github.com link to this folder at `sha` (what a relative registry entry installs from).
    pub fn tree_url(&self, sha: &str) -> String {
        let mut out = format!("https://github.com/{}/{}/tree/{sha}", self.owner, self.repo);
        if !self.dir.is_empty() {
            out.push('/');
            out.push_str(&self.dir);
        }
        out
    }
}

// registry.json: `[...]` or `{ "extensions": [...] }` (extra top-level fields are ignored).
// Each entry is a link string, or an object with a link in `url` / `link` / `repo` /
// `manifest_url` / `download_url` plus optional listing fields (`id`, `name`, `display_name`,
// `version`, `description`, `category`, `tags`, `sha256`). Links are GitHub extension links,
// package files (`.spotiflac-ext` / `.sflx` / `.zip`, https, pinned to `sha256` when the
// registry gives one), or paths relative to the registry's folder in the same repository.
// Anything else is listed but not installable.

fn registry_invalid(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionRegistryInvalid, why)
}

fn unsupported(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionUnsupported, why)
}

/// What a registry says about an entry, shown even when the entry can't be installed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegistryListing {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
}

/// A short, trimmed, non-empty string field (first of `keys` that is set).
fn text_field(obj: &serde_json::Map<String, Value>, keys: &[&str], max: usize) -> Option<String> {
    keys.iter()
        .filter_map(|k| obj.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(|s| s.chars().take(max).collect())
}

fn listing(obj: &serde_json::Map<String, Value>) -> RegistryListing {
    let tags = obj
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|t| t.trim().chars().take(32).collect::<String>())
                .filter(|t| !t.is_empty())
                .take(8)
                .collect()
        })
        .unwrap_or_default();
    RegistryListing {
        id: text_field(obj, &["id"], 100),
        name: text_field(obj, &["display_name", "displayName", "name", "id"], 100),
        version: text_field(obj, &["version"], 40),
        description: text_field(obj, &["description"], 400),
        category: text_field(obj, &["category"], 40),
        tags,
    }
}

/// Parses registry.json into entries. `registry` is the registry's own link and `sha` the
/// commit it was read at; relative entries resolve against them. Duplicates are dropped.
pub fn parse_registry(bytes: &[u8], registry: &RepoLink, sha: &str) -> AppResult<Vec<RegistryLink>> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| registry_invalid(format!("registry.json: {e}")))?;
    let entries = match &doc {
        Value::Array(a) => a,
        Value::Object(o) => o
            .get("extensions")
            .or_else(|| o.get("sources"))
            .and_then(Value::as_array)
            .ok_or_else(|| registry_invalid("registry.json has no \"extensions\" list"))?,
        _ => return Err(registry_invalid("registry.json must be a list or an object")),
    };
    if entries.is_empty() {
        return Err(registry_invalid("registry lists no extensions"));
    }
    if entries.len() > MAX_REGISTRY_ENTRIES {
        return Err(registry_invalid(format!("registry lists more than {MAX_REGISTRY_ENTRIES} extensions")));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for entry in entries {
        let (link_text, listed, sha256) = match entry {
            Value::String(s) => {
                (Some(s.trim().to_owned()).filter(|s| !s.is_empty()), RegistryListing::default(), None)
            }
            Value::Object(o) => (
                text_field(o, &["url", "link", "repo", "manifest_url", "download_url"], 2048),
                listing(o),
                text_field(o, &["sha256"], 64).map(|s| s.to_ascii_lowercase()),
            ),
            _ => return Err(registry_invalid("registry entries must be links or objects")),
        };
        let entry = link_text.clone().or_else(|| listed.id.clone()).or_else(|| listed.name.clone()).unwrap_or_default();
        if !seen.insert(entry.clone()) {
            continue;
        }
        let link = match &link_text {
            Some(text) => resolve_entry(text, sha256, registry, sha),
            None => Err(unsupported("entry has no link")),
        };
        out.push(RegistryLink { entry, listed, link });
    }
    Ok(out)
}

/// Where a registry entry installs from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A GitHub folder (extension.json, or manifest.json + index.js).
    Repo(RepoLink),
    /// A package file; `sha256` pins it when the registry gives one.
    Package { url: Url, sha256: Option<String> },
}

/// One registry entry: its link (or id) as written, what the registry says about it, and the
/// source it resolves to (or why it can't be installed).
#[derive(Debug)]
pub struct RegistryLink {
    pub entry: String,
    pub listed: RegistryListing,
    /// (link to install from, source).
    pub link: AppResult<(String, Source)>,
}

/// One registry entry → (link to install from, source).
fn resolve_entry(text: &str, sha256: Option<String>, registry: &RepoLink, sha: &str) -> AppResult<(String, Source)> {
    let sha256 = match sha256 {
        Some(s) if !package::valid_sha256(&s) => return Err(registry_invalid("entry has an invalid sha256")),
        other => other,
    };
    if let Some(url) = package::parse_link(text) {
        return Ok((url.to_string(), Source::Package { url, sha256 }));
    }
    if text.contains("://") || text.starts_with("github.com/") || text.starts_with("raw.githubusercontent.com/") {
        // GitHub extension links and package files install; other sites are listed only.
        let link = RepoLink::parse(text).map_err(|e| unsupported(e.message))?;
        return Ok((text.to_owned(), Source::Repo(link)));
    }
    // Relative path inside the registry's repository, at the registry's commit.
    let mut dir: Vec<&str> = registry.dir.split('/').filter(|s| !s.is_empty()).collect();
    let mut parts: Vec<&str> = text.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    if parts.last().is_some_and(|f| *f == MANIFEST_FILE || *f == SCRIPT_MANIFEST_FILE) {
        parts.pop();
    }
    for part in parts {
        if part == ".." {
            dir.pop().ok_or_else(|| link_invalid("registry path leaves the repository"))?;
        } else if valid_segment(part) {
            dir.push(part);
        } else {
            return Err(link_invalid("invalid folder in registry path"));
        }
    }
    let path = dir.join("/");
    if let Some(url) = package::parse_link(&format!(
        "https://raw.githubusercontent.com/{}/{}/{sha}/{path}",
        registry.owner, registry.repo
    )) {
        // A package file committed next to the registry.
        return Ok((url.to_string(), Source::Package { url, sha256 }));
    }
    let link =
        RepoLink { owner: registry.owner.clone(), repo: registry.repo.clone(), git_ref: Some(sha.to_owned()), dir: path };
    Ok((link.tree_url(sha), Source::Repo(link)))
}

pub fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// API and raw-content base URLs (overridable in tests).
#[derive(Debug, Clone)]
pub struct GitHub {
    pub api: Url,
    pub raw: Url,
}

impl GitHub {
    /// The real github.com endpoints.
    pub fn public() -> AppResult<Self> {
        let parse = |s: &str| Url::parse(s).map_err(|e| AppError::internal(format!("github url: {e}")));
        Ok(Self { api: parse("https://api.github.com/")?, raw: parse("https://raw.githubusercontent.com/")? })
    }
}

fn join(base: &Url, path: &str) -> AppResult<Url> {
    base.join(path).map_err(|e| AppError::internal(format!("github url: {e}")))
}

fn enc(s: &str) -> String {
    super::manifest::encode(s)
}

impl GitHub {
    fn hosts(&self) -> Vec<String> {
        [&self.api, &self.raw].iter().filter_map(|u| u.host_str().map(str::to_owned)).collect()
    }

    /// Resolves the link's ref (or the default branch) to a commit SHA.
    pub async fn resolve_sha(&self, http: &ExtHttp, link: &RepoLink) -> AppResult<String> {
        let hosts = self.hosts();
        let repo_path = format!("repos/{}/{}", enc(&link.owner), enc(&link.repo));
        let git_ref = match &link.git_ref {
            Some(r) => r.clone(),
            None => {
                let repo = http.json(join(&self.api, &repo_path)?, &hosts).await?;
                repo.get("default_branch")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "repository has no default branch"))?
            }
        };
        if valid_sha(&git_ref) {
            return Ok(git_ref);
        }
        let commit = http.json(join(&self.api, &format!("{repo_path}/commits/{}", enc(&git_ref)))?, &hosts).await?;
        commit
            .get("sha")
            .and_then(|v| v.as_str())
            .filter(|s| valid_sha(s))
            .map(str::to_owned)
            .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "GitHub returned no commit SHA"))
    }

    /// Reads `{dir}/{file}` at `sha` (`None` = no such file); an oversized file is reported
    /// with `invalid`.
    async fn file_at(
        &self,
        http: &ExtHttp,
        link: &RepoLink,
        sha: &str,
        file: &str,
        cap: usize,
        invalid: ErrorCode,
    ) -> AppResult<Option<Vec<u8>>> {
        if !valid_sha(sha) {
            return Err(link_invalid("invalid commit SHA"));
        }
        let dir: String = link.dir.split('/').filter(|s| !s.is_empty()).map(|s| format!("{}/", enc(s))).collect();
        let path = format!("{}/{}/{sha}/{dir}{file}", enc(&link.owner), enc(&link.repo));
        match http.bytes(join(&self.raw, &path)?, &self.hosts(), cap).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.code == ErrorCode::NotFound => Ok(None),
            Err(e) if e.code == ErrorCode::ExtensionFailed => Err(AppError::new(invalid, e.message)),
            Err(e) => Err(e),
        }
    }

    /// Reads the folder's extension at `sha`: extension.json, or else manifest.json. A
    /// manifest.json with a `unison` section is declarative; otherwise index.js is read too
    /// and the extension runs as a script.
    pub async fn spec_at(&self, http: &ExtHttp, link: &RepoLink, sha: &str) -> AppResult<Spec> {
        let read = |file: &'static str, cap: usize| self.file_at(http, link, sha, file, cap, ErrorCode::ExtensionInvalid);
        if let Some(bytes) = read(MANIFEST_FILE, MAX_MANIFEST_BYTES).await? {
            return Manifest::parse(&bytes).map(|m| Spec::Declarative(Box::new(m)));
        }
        let Some(manifest) = read(SCRIPT_MANIFEST_FILE, MAX_MANIFEST_BYTES).await? else {
            return Err(AppError::new(ErrorCode::ExtensionInvalid, "no extension.json or manifest.json at that link"));
        };
        if let Some(m) = Manifest::from_script_manifest(&manifest)? {
            return Ok(Spec::Declarative(Box::new(m)));
        }
        let code = read(SCRIPT_FILE, MAX_SCRIPT_BYTES)
            .await?
            .ok_or_else(|| AppError::new(ErrorCode::ExtensionInvalid, "manifest.json has no index.js next to it"))?;
        let icon = match script::icon_name(&manifest) {
            Some(name) => self.file_at(http, link, sha, &name, script::MAX_ICON_BYTES, ErrorCode::ExtensionInvalid).await.ok().flatten(),
            None => None,
        };
        let mut ext = tokio::task::spawn_blocking(move || ScriptExtension::load(&manifest, &code))
            .await
            .map_err(|e| AppError::internal(format!("script check: {e}")))??;
        ext.manifest.icon = icon.and_then(|b| script::icon_data_url(&b));
        Ok(Spec::Script { manifest: Box::new(ext.manifest), code: ext.code })
    }

    /// Reads registry.json at `sha` and resolves its entries to extension links.
    pub async fn registry_at(&self, http: &ExtHttp, link: &RepoLink, sha: &str) -> AppResult<Vec<RegistryLink>> {
        let bytes = self
            .file_at(http, link, sha, REGISTRY_FILE, MAX_REGISTRY_BYTES, ErrorCode::ExtensionRegistryInvalid)
            .await?
            .ok_or_else(|| registry_invalid("no registry.json at that link"))?;
        parse_registry(&bytes, link, sha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn link(owner: &str, repo: &str, r: Option<&str>, dir: &str) -> RepoLink {
        RepoLink { owner: owner.into(), repo: repo.into(), git_ref: r.map(Into::into), dir: dir.into() }
    }

    #[test]
    fn parses_repo_tree_and_blob_links() {
        assert_eq!(RepoLink::parse("https://github.com/acme/ext").unwrap(), link("acme", "ext", None, ""));
        assert_eq!(RepoLink::parse(" github.com/acme/ext.git/ ").unwrap(), link("acme", "ext", None, ""));
        assert_eq!(
            RepoLink::parse("https://github.com/acme/ext/tree/v1.2/sources/radio").unwrap(),
            link("acme", "ext", Some("v1.2"), "sources/radio")
        );
        assert_eq!(
            RepoLink::parse("https://github.com/acme/ext/blob/main/extension.json").unwrap(),
            link("acme", "ext", Some("main"), "")
        );
        assert_eq!(
            RepoLink::parse("https://github.com/acme/ext/blob/main/sources/radio/manifest.json").unwrap(),
            link("acme", "ext", Some("main"), "sources/radio"),
            "a script extension's manifest.json"
        );
        assert_eq!(
            RepoLink::parse("https://raw.githubusercontent.com/acme/ext/main/manifest.json").unwrap(),
            link("acme", "ext", Some("main"), "")
        );
        for package in [
            "https://github.com/acme/ext/releases/download/v1/radio.spotiflac-ext",
            "https://github.com/acme/ext/blob/main/dist/Radio.SPOTIFLAC-EXT",
            "https://example.com/radio.spotiflac-ext",
        ] {
            assert_eq!(RepoLink::parse(package).unwrap_err().code, ErrorCode::ExtensionLinkInvalid, "{package}");
            assert!(package::parse_link(package).is_some(), "{package} installs as a package");
        }
        for bad in [
            "https://gitlab.com/acme/ext",
            "http://github.com/acme/ext",
            "https://github.com/acme",
            "https://github.com/acme/ext/blob/main/README.md",
            "https://github.com/acme/ext/issues/1",
            "https://github.com/acme/ext/blob/main",
            "https://github.com/../ext",
            "not a link",
        ] {
            assert_eq!(RepoLink::parse(bad).unwrap_err().code, ErrorCode::ExtensionLinkInvalid, "{bad}");
        }
    }

    #[test]
    fn parses_raw_and_registry_links() {
        assert_eq!(
            RepoLink::parse("https://raw.githubusercontent.com/acme/ext/main/sources/radio/extension.json").unwrap(),
            link("acme", "ext", Some("main"), "sources/radio")
        );
        assert_eq!(
            RepoLink::parse("raw.githubusercontent.com/acme/ext/refs/heads/dev/extension.json").unwrap(),
            link("acme", "ext", Some("dev"), "")
        );
        assert_eq!(
            RepoLink::parse_any("https://raw.githubusercontent.com/acme/hub/main/registry.json").unwrap(),
            (link("acme", "hub", Some("main"), ""), LinkKind::Registry)
        );
        assert_eq!(
            RepoLink::parse_any("https://github.com/acme/hub/blob/v2/lists/registry.json").unwrap(),
            (link("acme", "hub", Some("v2"), "lists"), LinkKind::Registry)
        );
        assert_eq!(
            RepoLink::parse_any("https://github.com/acme/ext").unwrap().1,
            LinkKind::Extension,
            "a repo link is an extension"
        );
        for bad in [
            "https://raw.githubusercontent.com/acme/ext/main",
            "https://raw.githubusercontent.com/acme/ext/main/README.md",
            "http://raw.githubusercontent.com/acme/ext/main/registry.json",
            "https://raw.githubusercontent.com/acme",
        ] {
            assert_eq!(RepoLink::parse_any(bad).unwrap_err().code, ErrorCode::ExtensionLinkInvalid, "{bad}");
        }
        assert_eq!(
            RepoLink::parse("https://raw.githubusercontent.com/acme/hub/main/registry.json").unwrap_err().code,
            ErrorCode::ExtensionLinkInvalid,
            "registry links aren't single extensions"
        );
    }

    #[test]
    fn lists_package_registries_as_installable_packages() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let reg = link("acme", "hub", Some("main"), "");
        let doc = serde_json::json!({
            "version": 1,
            "updated_at": "2026-11-20T00:00:00Z",
            "extensions": [
                {
                    "id": "mock-stream-a",
                    "name": "mock-stream-a",
                    "display_name": "Mock Stream A",
                    "version": "9.9.9",
                    "description": "Mock metadata provider.",
                    "download_url": "https://example.com/mock/extensions/mock-stream-a.sflx",
                    "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                    "category": "integration",
                    "tags": ["mock", "stream", 3],
                    "downloads": 1001,
                    "min_app_version": "1.0.0"
                },
                { "id": "radio", "name": "Radio", "url": "https://github.com/acme/radio" },
                { "id": "nolink", "display_name": "No Link" }
            ]
        });
        let got = parse_registry(&serde_json::to_vec(&doc).unwrap(), &reg, sha).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].entry, "https://example.com/mock/extensions/mock-stream-a.sflx");
        assert_eq!(
            got[0].listed,
            RegistryListing {
                id: Some("mock-stream-a".into()),
                name: Some("Mock Stream A".into()),
                version: Some("9.9.9".into()),
                description: Some("Mock metadata provider.".into()),
                category: Some("integration".into()),
                tags: vec!["mock".into(), "stream".into()],
            }
        );
        let package_url = Url::parse("https://example.com/mock/extensions/mock-stream-a.sflx").unwrap();
        assert_eq!(
            got[0].link.as_ref().unwrap().1,
            Source::Package { url: package_url, sha256: Some("1".repeat(64)) },
            "packages install, pinned to the registry's sha256"
        );
        assert_eq!(got[1].link.as_ref().unwrap().1, Source::Repo(link("acme", "radio", None, "")));
        assert_eq!(got[2].entry, "nolink");
        assert_eq!(got[2].link.as_ref().unwrap_err().code, ErrorCode::ExtensionUnsupported);
    }

    #[test]
    fn parses_registry_lists_objects_and_relative_entries() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let reg = link("acme", "hub", Some("main"), "lists");
        let doc = serde_json::json!({ "extensions": [
            "https://github.com/acme/radio",
            { "name": "Archive", "url": "https://raw.githubusercontent.com/other/arch/v1/extension.json" },
            "../sources/radio",
            "./local/extension.json",
            "https://github.com/acme/radio",
            "https://example.com/nope",
            "../../../escape"
        ]});
        let got = parse_registry(&serde_json::to_vec(&doc).unwrap(), &reg, sha).unwrap();
        assert_eq!(got.len(), 6, "duplicates are dropped");
        let ok = |i: usize| match got[i].link.as_ref().unwrap().clone() {
            (text, Source::Repo(l)) => (text, l),
            other => panic!("not a repo entry: {other:?}"),
        };
        assert_eq!(ok(0), ("https://github.com/acme/radio".to_owned(), link("acme", "radio", None, "")));
        assert_eq!(ok(1).1, link("other", "arch", Some("v1"), ""));
        assert_eq!(
            ok(2),
            (format!("https://github.com/acme/hub/tree/{sha}/sources/radio"), link("acme", "hub", Some(sha), "sources/radio"))
        );
        assert_eq!(ok(3).1, link("acme", "hub", Some(sha), "lists/local"));
        assert_eq!(RepoLink::parse(&ok(2).0).unwrap(), ok(2).1, "relative entries install from a re-parseable link");
        assert_eq!(got[4].link.as_ref().unwrap_err().code, ErrorCode::ExtensionUnsupported);
        assert_eq!(got[5].entry, "../../../escape");
        assert!(got[5].link.is_err());

        let list = serde_json::json!(["https://github.com/acme/radio"]);
        assert_eq!(parse_registry(&serde_json::to_vec(&list).unwrap(), &reg, sha).unwrap().len(), 1);
        for bad in [&b"[]"[..], b"{\"extensions\":[]}", b"not json", b"{\"other\":1}", b"[1]"] {
            assert_eq!(parse_registry(bad, &reg, sha).unwrap_err().code, ErrorCode::ExtensionRegistryInvalid);
        }
    }

    #[tokio::test]
    async fn pins_default_branch_to_a_sha_and_reads_the_manifest() {
        let server = MockServer::start().await;
        let sha = "0123456789abcdef0123456789abcdef01234567";
        Mock::given(method("GET"))
            .and(path("/repos/acme/ext"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "default_branch": "main" })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/repos/acme/ext/commits/main"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "sha": sha })))
            .mount(&server)
            .await;
        let manifest = super::super::manifest::tests::sample("https://api.example.org", "api.example.org");
        Mock::given(method("GET"))
            .and(path(format!("/acme/ext/{sha}/src/extension.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(manifest))
            .mount(&server)
            .await;

        let base = Url::parse(&format!("{}/", server.uri())).unwrap();
        let gh = GitHub { api: base.clone(), raw: base };
        let http = ExtHttp::new().unwrap();
        let l = link("acme", "ext", None, "src");
        let got = gh.resolve_sha(&http, &l).await.unwrap();
        assert_eq!(got, sha);
        let m = gh.spec_at(&http, &l, &got).await.unwrap();
        assert_eq!(m.id(), "example-source");

        let missing = gh.spec_at(&http, &link("acme", "ext", None, "nope"), sha).await.unwrap_err();
        assert_eq!(missing.code, ErrorCode::ExtensionInvalid);

        // No extension.json: the folder's manifest.json is read instead.
        let script = super::super::manifest::tests::script_manifest("https://api.example.org", "api.example.org");
        Mock::given(method("GET"))
            .and(path(format!("/acme/ext/{sha}/script/manifest.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(&script))
            .mount(&server)
            .await;
        let m = gh.spec_at(&http, &link("acme", "ext", None, "script"), sha).await.unwrap();
        assert!(matches!(m, Spec::Declarative(_)), "a unison section makes it declarative");
        assert_eq!((m.id(), m.name()), ("example-source", "Example Source"));

        let mut js_only = script;
        js_only.as_object_mut().unwrap().remove("unison");
        Mock::given(method("GET"))
            .and(path(format!("/acme/ext/{sha}/js/manifest.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(js_only))
            .mount(&server)
            .await;
        let err = gh.spec_at(&http, &link("acme", "ext", None, "js"), sha).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::ExtensionInvalid, "no index.js next to it");
        Mock::given(method("GET"))
            .and(path(format!("/acme/ext/{sha}/js/index.js")))
            .respond_with(ResponseTemplate::new(200).set_body_string("registerExtension({ searchTracks() { return []; } });"))
            .mount(&server)
            .await;
        match gh.spec_at(&http, &link("acme", "ext", None, "js"), sha).await.unwrap() {
            Spec::Script { manifest, code } => {
                assert_eq!(manifest.functions, vec!["searchTracks"]);
                assert!(code.contains("registerExtension"));
            }
            other => panic!("expected a script extension, got {other:?}"),
        }
    }
}
