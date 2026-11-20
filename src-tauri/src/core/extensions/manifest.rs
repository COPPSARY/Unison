//! `extension.json` (schema 1): a declarative description of a music source. Unison never runs
//! extension code; it only fills URL templates, calls the declared hosts and reads fields with
//! JSON Pointers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::core::db::library::TrackInput;
use crate::error::{AppError, AppResult, ErrorCode};

/// Largest accepted extension.json.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// Most results kept from one search response.
pub const MAX_ITEMS: usize = 50;
const MAX_HOSTS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// https link to the extension's site.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Every host the extension may contact (exact names, no wildcards).
    pub hosts: Vec<String>,
    pub search: SearchSpec,
    pub stream: StreamSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchSpec {
    /// URL template containing `{query}`.
    pub url: String,
    /// JSON Pointer to the result array (`""` = the response itself).
    pub items: String,
    pub fields: Fields,
}

/// JSON Pointers into one result item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fields {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub duration_unit: DurationUnit,
    #[serde(default)]
    pub artwork: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    /// Web page of the track (for "Open in…").
    #[serde(default)]
    pub url: Option<String>,
    /// The track's licence (a URL, a name or `{ url, name }`); open licences allow Download.
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DurationUnit {
    #[default]
    Ms,
    S,
}

/// Either a direct stream URL template, or an endpoint whose JSON holds the stream URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamSpec {
    /// Direct audio URL template containing `{id}`.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub resolve: Option<ResolveSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveSpec {
    /// URL template containing `{id}`; answers with JSON.
    pub url: String,
    /// JSON Pointer to the stream URL in that answer.
    pub field: String,
}

/// The `unison` section of a script extension's manifest.json.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UnisonSection {
    /// Defaults to the manifest's `name`.
    #[serde(default)]
    id: Option<String>,
    /// Defaults to the manifest's `permissions.network`.
    #[serde(default)]
    hosts: Option<Vec<String>>,
    search: SearchSpec,
    stream: StreamSpec,
}

fn invalid(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionInvalid, why)
}

pub fn host_not_allowed(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionHostNotAllowed, why)
}

pub fn valid_id(id: &str) -> bool {
    (3..=40).contains(&id.len()) && id.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.'))
        && !host.starts_with('.')
        && !host.ends_with('.')
}

fn valid_pointer(p: &str) -> bool {
    p.is_empty() || p.starts_with('/')
}

/// Loopback hosts are allowed over plain http in debug builds only (local test fixtures).
fn is_loopback(host: &str) -> bool {
    host == "127.0.0.1" || host == "localhost"
}

/// Whether `host` is on the allowlist. `*.example.com` entries (script extensions only) match
/// subdomains of example.com.
pub fn host_allowed(host: &str, hosts: &[String]) -> bool {
    hosts.iter().any(|h| match h.strip_prefix("*.") {
        Some(base) => host.len() > base.len() + 1 && host.ends_with(base) && host[..host.len() - base.len()].ends_with('.'),
        None => h == host,
    })
}

/// Checks a URL against an allowlist: https (or debug-only loopback http) and a listed host.
pub fn check_url(url: &Url, hosts: &[String]) -> AppResult<()> {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let scheme_ok = match url.scheme() {
        "https" => true,
        "http" => cfg!(debug_assertions) && is_loopback(&host),
        _ => false,
    };
    if !scheme_ok {
        return Err(host_not_allowed(format!("{} is not https", url.scheme())));
    }
    if !host_allowed(&host, hosts) {
        return Err(host_not_allowed(format!("host {host} is not declared")));
    }
    Ok(())
}

/// Percent-encodes everything except RFC 3986 unreserved characters (safe in path and query).
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Fills `{name}` in a template and checks the result against the allowlist.
pub fn fill(template: &str, name: &str, value: &str, hosts: &[String]) -> AppResult<Url> {
    let raw = template.replace(&format!("{{{name}}}"), &encode(value));
    let url = Url::parse(&raw).map_err(|e| invalid(format!("bad URL template: {e}")))?;
    check_url(&url, hosts)?;
    Ok(url)
}

fn check_template(template: &str, placeholder: &str, hosts: &[String]) -> AppResult<()> {
    if !template.contains(&format!("{{{placeholder}}}")) {
        return Err(invalid(format!("URL template must contain {{{placeholder}}}")));
    }
    fill(template, placeholder, "x", hosts).map(|_| ())
}

impl Manifest {
    /// Parses and validates raw extension.json bytes.
    pub fn parse(bytes: &[u8]) -> AppResult<Manifest> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("extension.json is larger than 64 KB"));
        }
        let mut m: Manifest =
            serde_json::from_slice(bytes).map_err(|e| invalid(format!("extension.json: {e}")))?;
        m.hosts = m.hosts.iter().map(|h| h.trim().to_ascii_lowercase()).collect();
        m.validate()?;
        Ok(m)
    }

    /// Converts a script extension's `manifest.json` that carries a `unison` section (extension.json's
    /// `search` and `stream`, plus optional `id` and `hosts`) into a declarative manifest, so it
    /// runs without its index.js. `name`/`displayName`, `version`, `author` and `description`
    /// come from the manifest's own fields; hosts default to `permissions.network` (exact names
    /// only). `None`: no `unison` section (the extension runs as a script instead).
    pub fn from_script_manifest(bytes: &[u8]) -> AppResult<Option<Manifest>> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("manifest.json is larger than 64 KB"));
        }
        let doc: Value = serde_json::from_slice(bytes).map_err(|e| invalid(format!("manifest.json: {e}")))?;
        let obj = doc.as_object().ok_or_else(|| invalid("manifest.json must be an object"))?;
        let Some(section) = obj.get("unison") else { return Ok(None) };
        let unison: UnisonSection = serde_json::from_value(section.clone())
            .map_err(|e| invalid(format!("manifest.json \"unison\": {e}")))?;
        let field = |key: &str| {
            obj.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
        };
        let network = || -> Vec<String> {
            obj.get("permissions")
                .and_then(|p| p.get("network"))
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        let mut m = Manifest {
            schema: 1,
            id: unison.id.or_else(|| field("name")).unwrap_or_default(),
            name: field("displayName").or_else(|| field("name")).unwrap_or_default(),
            version: field("version").unwrap_or_default(),
            author: field("author"),
            description: field("description"),
            homepage: field("homepage"),
            hosts: unison.hosts.unwrap_or_else(network),
            search: unison.search,
            stream: unison.stream,
        };
        m.hosts = m.hosts.iter().map(|h| h.trim().to_ascii_lowercase()).collect();
        m.validate()?;
        Ok(Some(m))
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.schema != 1 {
            return Err(invalid(format!("unsupported schema {}", self.schema)));
        }
        if !valid_id(&self.id) {
            return Err(invalid("id must be 3–40 characters: a–z, 0–9 and -"));
        }
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err(invalid("name must be 1–80 characters"));
        }
        if self.version.trim().is_empty() || self.version.len() > 32 {
            return Err(invalid("version must be 1–32 characters"));
        }
        if self.homepage.as_deref().is_some_and(|u| !Url::parse(u).is_ok_and(|u| u.scheme() == "https")) {
            return Err(invalid("homepage must be an https link"));
        }
        if self.hosts.is_empty() || self.hosts.len() > MAX_HOSTS {
            return Err(invalid("hosts must list 1–10 host names"));
        }
        if let Some(bad) = self.hosts.iter().find(|h| !valid_host(h)) {
            return Err(invalid(format!("invalid host {bad:?} (exact names only, no wildcards)")));
        }
        check_template(&self.search.url, "query", &self.hosts)?;
        let f = &self.search.fields;
        let pointers = [Some(&self.search.items), Some(&f.id), Some(&f.title)]
            .into_iter()
            .chain([&f.artists, &f.album, &f.duration, &f.artwork, &f.isrc, &f.url].map(Option::as_ref));
        if pointers.flatten().any(|p| !valid_pointer(p)) {
            return Err(invalid("field paths must be JSON Pointers like /data/0/title"));
        }
        match (&self.stream.url, &self.stream.resolve) {
            (Some(url), None) => check_template(url, "id", &self.hosts),
            (None, Some(r)) => {
                if !valid_pointer(&r.field) {
                    return Err(invalid("stream.resolve.field must be a JSON Pointer"));
                }
                check_template(&r.url, "id", &self.hosts)
            }
            _ => Err(invalid("stream needs exactly one of url or resolve")),
        }
    }
}

fn text(item: &Value, pointer: Option<&String>) -> Option<String> {
    match item.pointer(pointer?)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn artists(item: &Value, pointer: Option<&String>) -> Vec<String> {
    match pointer.and_then(|p| item.pointer(p)) {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_owned()],
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|v| match v {
                Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
                Value::Object(_) => v.get("name").and_then(Value::as_str).map(str::to_owned),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn duration_ms(item: &Value, pointer: Option<&String>, unit: DurationUnit) -> Option<u32> {
    let raw = item.pointer(pointer?)?.as_f64()?;
    let ms = match unit {
        DurationUnit::Ms => raw,
        DurationUnit::S => raw * 1000.0,
    };
    (ms.is_finite() && ms > 0.0 && ms < f64::from(u32::MAX)).then(|| ms.round() as u32)
}

/// One mapped search result (item id is the extension's own id).
#[derive(Debug, Clone, PartialEq)]
pub struct MappedItem {
    pub item_id: String,
    pub track: TrackInput,
    /// Remote artwork URL, fetched into the local cache by the manager.
    pub artwork_remote: Option<String>,
    /// An open licence the result declares (EXT-07).
    pub license: Option<super::license::OpenLicense>,
}

impl Manifest {
    /// Maps a search response to tracks. Items without an id or title are skipped.
    pub fn map_search(&self, body: &Value) -> AppResult<Vec<MappedItem>> {
        let list = body
            .pointer(&self.search.items)
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "search items is not an array"))?;
        let f = &self.search.fields;
        Ok(list
            .iter()
            .filter_map(|item| {
                let item_id = text(item, Some(&f.id))?;
                let title = text(item, Some(&f.title))?;
                if item_id.len() > 200 {
                    return None;
                }
                let web = text(item, f.url.as_ref())
                    .filter(|u| Url::parse(u).is_ok_and(|u| u.scheme() == "https"))
                    .unwrap_or_default();
                Some(MappedItem {
                    track: TrackInput {
                        item_id: format!("{}:{item_id}", self.id),
                        url: web,
                        title,
                        artists: artists(item, f.artists.as_ref()),
                        album: text(item, f.album.as_ref()),
                        duration_ms: duration_ms(item, f.duration.as_ref(), f.duration_unit),
                        isrc: text(item, f.isrc.as_ref()),
                        artwork: None,
                        available: true,
                    },
                    artwork_remote: text(item, f.artwork.as_ref()),
                    license: f.license.as_ref().and_then(|p| item.pointer(p)).and_then(super::license::parse),
                    item_id,
                })
            })
            .take(MAX_ITEMS)
            .collect())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub fn sample(base: &str, host: &str) -> Value {
        json!({
            "schema": 1, "id": "example-source", "name": "Example", "version": "1.0.0",
            "author": "Someone", "hosts": [host],
            "search": {
                "url": format!("{base}/search?q={{query}}"), "items": "/data",
                "fields": { "id": "/id", "title": "/title", "artists": "/user/name",
                            "duration": "/duration", "durationUnit": "s", "artwork": "/art" }
            },
            "stream": { "url": format!("{base}/tracks/{{id}}/stream") }
        })
    }

    fn parse(v: &Value) -> AppResult<Manifest> {
        Manifest::parse(v.to_string().as_bytes())
    }

    fn code(r: AppResult<Manifest>) -> ErrorCode {
        r.unwrap_err().code
    }

    #[test]
    fn accepts_a_valid_manifest() {
        let m = parse(&sample("https://api.example.org", "API.example.org")).unwrap();
        assert_eq!(m.id, "example-source");
        assert_eq!(m.hosts, vec!["api.example.org"]);
    }

    #[test]
    fn rejects_bad_manifests() {
        let base = sample("https://api.example.org", "api.example.org");
        let with = |f: &dyn Fn(&mut Value)| {
            let mut v = base.clone();
            f(&mut v);
            v
        };
        assert_eq!(code(parse(&with(&|v| v["schema"] = json!(2)))), ErrorCode::ExtensionInvalid);
        assert_eq!(code(parse(&with(&|v| v["id"] = json!("Bad Id")))), ErrorCode::ExtensionInvalid);
        assert_eq!(code(parse(&with(&|v| v["hosts"] = json!(["*.example.org"])))), ErrorCode::ExtensionInvalid);
        assert_eq!(code(parse(&with(&|v| v["hosts"] = json!([])))), ErrorCode::ExtensionInvalid);
        assert_eq!(code(parse(&with(&|v| v["extra"] = json!(1)))), ErrorCode::ExtensionInvalid);
        assert_eq!(
            code(parse(&with(&|v| v["search"]["url"] = json!("https://api.example.org/s")))),
            ErrorCode::ExtensionInvalid,
            "missing {{query}}"
        );
        assert_eq!(
            code(parse(&with(&|v| v["search"]["url"] = json!("https://other.example/s?q={query}")))),
            ErrorCode::ExtensionHostNotAllowed
        );
        assert_eq!(
            code(parse(&with(&|v| v["stream"]["url"] = json!("http://api.example.org/t/{id}")))),
            ErrorCode::ExtensionHostNotAllowed,
            "plain http"
        );
        assert_eq!(
            code(parse(&with(&|v| v["stream"] = json!({"url": "https://api.example.org/{id}",
                "resolve": {"url": "https://api.example.org/{id}", "field": "/u"}})))),
            ErrorCode::ExtensionInvalid
        );
        assert_eq!(
            code(parse(&with(&|v| v["search"]["fields"]["title"] = json!("title")))),
            ErrorCode::ExtensionInvalid,
            "not a pointer"
        );
        let big = vec![b' '; MAX_MANIFEST_BYTES + 1];
        assert_eq!(Manifest::parse(&big).unwrap_err().code, ErrorCode::ExtensionInvalid);
    }

    /// A script extension's manifest.json with a `unison` section.
    pub fn script_manifest(base: &str, host: &str) -> Value {
        let ext = sample(base, host);
        json!({
            "name": "example-source", "displayName": "Example Source", "version": "1.2.0",
            "description": "Example", "author": "Someone", "homepage": "https://example.org",
            "icon": "icon.png", "type": ["metadata_provider"], "minAppVersion": "3.0.1",
            "permissions": { "network": [host], "storage": true, "file": false },
            "settings": [{ "key": "q", "type": "select", "options": ["a"] }],
            "unison": { "search": ext["search"], "stream": ext["stream"] }
        })
    }

    fn from_script(v: &Value) -> AppResult<Manifest> {
        Manifest::from_script_manifest(v.to_string().as_bytes())?.ok_or_else(|| invalid("no unison section"))
    }

    #[test]
    fn converts_a_script_manifest_with_a_unison_section() {
        let m = from_script(&script_manifest("https://api.example.org", "API.example.org")).unwrap();
        assert_eq!(m.id, "example-source");
        assert_eq!(m.name, "Example Source");
        assert_eq!(m.version, "1.2.0");
        assert_eq!(m.author.as_deref(), Some("Someone"));
        assert_eq!(m.hosts, vec!["api.example.org"], "hosts come from permissions.network");
        let native = parse(&sample("https://api.example.org", "api.example.org")).unwrap();
        assert_eq!((&m.search, &m.stream), (&native.search, &native.stream));

        // `unison.id` / `unison.hosts` override the manifest's fields; no displayName → name.
        let mut v = script_manifest("https://api.example.org", "api.example.org");
        v["name"] = json!("Example Source");
        v.as_object_mut().unwrap().remove("displayName");
        v["permissions"]["network"] = json!(["*.example.org"]);
        v["unison"]["id"] = json!("example-src");
        v["unison"]["hosts"] = json!(["api.example.org"]);
        let m = from_script(&v).unwrap();
        assert_eq!((m.id.as_str(), m.name.as_str()), ("example-src", "Example Source"));
        assert_eq!(m.hosts, vec!["api.example.org"]);
    }

    #[test]
    fn rejects_script_manifests_unison_cannot_use() {
        let base = script_manifest("https://api.example.org", "api.example.org");
        let with = |f: &dyn Fn(&mut Value)| {
            let mut v = base.clone();
            f(&mut v);
            from_script(&v).unwrap_err().code
        };
        let mut js_only = base.clone();
        js_only.as_object_mut().unwrap().remove("unison");
        assert_eq!(Manifest::from_script_manifest(js_only.to_string().as_bytes()).unwrap(), None, "runs as a script");
        assert_eq!(
            with(&|v| v["permissions"]["network"] = json!(["*.example.org"])),
            ErrorCode::ExtensionInvalid,
            "wildcard hosts need unison.hosts"
        );
        assert_eq!(with(&|v| v["name"] = json!("Bad Name")), ErrorCode::ExtensionInvalid, "id needs unison.id");
        assert_eq!(with(&|v| v["unison"]["extra"] = json!(1)), ErrorCode::ExtensionInvalid);
        assert_eq!(
            with(&|v| v["unison"]["search"]["url"] = json!("https://other.example/s?q={query}")),
            ErrorCode::ExtensionHostNotAllowed
        );
        assert_eq!(Manifest::from_script_manifest(b"[]").unwrap_err().code, ErrorCode::ExtensionInvalid);
        assert_eq!(Manifest::from_script_manifest(b"nope").unwrap_err().code, ErrorCode::ExtensionInvalid);
    }

    #[test]
    fn wildcard_hosts_match_subdomains_only() {
        let hosts = vec!["*.example.org".to_owned(), "api.other.net".to_owned()];
        assert!(host_allowed("a.example.org", &hosts));
        assert!(host_allowed("cdn.eu.example.org", &hosts));
        assert!(!host_allowed("example.org", &hosts));
        assert!(!host_allowed("badexample.org", &hosts));
        assert!(host_allowed("api.other.net", &hosts));
        assert!(!host_allowed("x.api.other.net", &hosts));
    }

    #[test]
    fn fills_templates_with_encoded_values() {
        let hosts = vec!["api.example.org".to_owned()];
        let url = fill("https://api.example.org/s?q={query}", "query", "a b&c/é", &hosts).unwrap();
        assert_eq!(url.as_str(), "https://api.example.org/s?q=a%20b%26c%2F%C3%A9");
    }

    #[test]
    fn maps_search_results() {
        let m = parse(&sample("https://api.example.org", "api.example.org")).unwrap();
        let body = json!({ "data": [
            { "id": 42, "title": " Song ", "user": { "name": "Artist" }, "duration": 61.5, "art": "https://api.example.org/a.jpg" },
            { "id": "x", "title": "" },
            { "title": "no id" }
        ]});
        let items = m.map_search(&body).unwrap();
        assert_eq!(items.len(), 1);
        let first = &items[0];
        assert_eq!(first.item_id, "42");
        assert_eq!(first.track.item_id, "example-source:42");
        assert_eq!(first.track.title, "Song");
        assert_eq!(first.track.artists, vec!["Artist"]);
        assert_eq!(first.track.duration_ms, Some(61_500));
        assert_eq!(first.artwork_remote.as_deref(), Some("https://api.example.org/a.jpg"));
        assert!(m.map_search(&json!({ "data": {} })).is_err());
    }

    #[test]
    fn artists_accept_arrays_of_strings_or_objects() {
        let item = json!({ "a": ["X", "Y"], "b": [{ "name": "Z" }] });
        assert_eq!(artists(&item, Some(&"/a".to_owned())), vec!["X", "Y"]);
        assert_eq!(artists(&item, Some(&"/b".to_owned())), vec!["Z"]);
    }
}
