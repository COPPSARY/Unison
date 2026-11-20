//! Script extensions (EXT-02): `manifest.json` + `index.js`, installed from a package
//! (`.spotiflac-ext` / `.sflx` zip) or a GitHub folder. index.js runs in a sandboxed QuickJS
//! engine ([`runtime`]) that only reaches the hosts in `permissions.network`. Unison uses
//! `searchTracks` / `customSearch` for Search and `getDownloadUrl` as the stream URL that
//! NativeEngine plays; nothing is written to disk (no file, ffmpeg or download APIs).

pub mod package;
pub mod runtime;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

use super::manifest::{valid_host, valid_id, MappedItem, MAX_ITEMS, MAX_MANIFEST_BYTES};
use crate::core::db::library::TrackInput;
use crate::error::{AppError, AppResult, ErrorCode};

/// Largest index.js accepted.
pub const MAX_SCRIPT_BYTES: usize = 8 * 1024 * 1024;
const MAX_HOSTS: usize = 40;

/// Functions Unison calls for Search, in order of preference.
pub const SEARCH_FUNCTIONS: [&str; 2] = ["searchTracks", "customSearch"];
/// Function Unison calls for a playable URL.
pub const STREAM_FUNCTION: &str = "getDownloadUrl";

fn invalid(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionInvalid, why)
}

/// What Unison keeps from a script extension's manifest.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    /// https link to the extension's site.
    #[serde(default)]
    pub homepage: Option<String>,
    /// The package's icon as a `data:` URL (PNG, JPEG or WebP, at most 256 KB).
    #[serde(default)]
    pub icon: Option<String>,
    /// `permissions.network`: exact names or `*.domain` wildcards.
    pub hosts: Vec<String>,
    /// Manifest `type` (metadata_provider, download_provider, lyrics_provider…).
    pub types: Vec<String>,
    /// Setting defaults, passed to `initialize(config)`.
    pub settings: Map<String, Value>,
    /// First `qualityOptions` id, passed to `getDownloadUrl` unless the user picks another.
    pub quality: Option<String>,
    /// Every `qualityOptions` entry.
    #[serde(default)]
    pub qualities: Vec<QualityOption>,
    /// Functions index.js registered (read when it was installed).
    #[serde(default)]
    pub functions: Vec<String>,
}

/// One stream quality an extension offers (`qualityOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QualityOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// A manifest `name` → an id Unison accepts (`a–z 0–9 -`, 3–40 characters).
fn id_from(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(40).collect::<String>().trim_end_matches('-').to_owned()
}

fn text(obj: &Map<String, Value>, key: &str, max: usize) -> Option<String> {
    let v = obj.get(key)?;
    let s = v.as_str().or_else(|| v.get("name").and_then(Value::as_str))?.trim();
    (!s.is_empty()).then(|| s.chars().take(max).collect())
}

impl ScriptManifest {
    /// Parses a script extension's manifest.json (as documented for that format).
    pub fn parse(bytes: &[u8]) -> AppResult<ScriptManifest> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(invalid("manifest.json is larger than 64 KB"));
        }
        let doc: Value = serde_json::from_slice(bytes).map_err(|e| invalid(format!("manifest.json: {e}")))?;
        let obj = doc.as_object().ok_or_else(|| invalid("manifest.json must be an object"))?;
        let raw_name = text(obj, "name", 200).ok_or_else(|| invalid("manifest.json has no name"))?;
        let id = id_from(&raw_name);
        if !valid_id(&id) {
            return Err(invalid("name must give an id of 3–40 characters"));
        }
        let version = text(obj, "version", 32).ok_or_else(|| invalid("manifest.json has no version"))?;
        let hosts: Vec<String> = obj
            .get("permissions")
            .and_then(|p| p.get("network"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(|h| h.trim().to_ascii_lowercase()).collect())
            .unwrap_or_default();
        if hosts.len() > MAX_HOSTS {
            return Err(invalid(format!("permissions.network lists more than {MAX_HOSTS} hosts")));
        }
        if let Some(bad) = hosts.iter().find(|h| !valid_host(h.strip_prefix("*.").unwrap_or(h))) {
            return Err(invalid(format!("invalid host {bad:?} in permissions.network")));
        }
        let types = match obj.get("type") {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
            _ => Vec::new(),
        };
        let mut settings = Map::new();
        for s in obj.get("settings").and_then(Value::as_array).into_iter().flatten() {
            if let (Some(key), Some(default)) = (s.get("key").and_then(Value::as_str), s.get("default")) {
                settings.insert(key.to_owned(), default.clone());
            }
        }
        let qualities: Vec<QualityOption> = obj
            .get("qualityOptions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|q| {
                let o = q.as_object()?;
                let id = text(o, "id", 64)?;
                Some(QualityOption { label: text(o, "label", 80).unwrap_or_else(|| id.clone()), description: text(o, "description", 200), id })
            })
            .take(20)
            .collect();
        let quality = qualities.first().map(|q| q.id.clone());
        Ok(ScriptManifest {
            id,
            name: text(obj, "displayName", 80).unwrap_or_else(|| raw_name.chars().take(80).collect()),
            version,
            author: text(obj, "author", 100),
            description: text(obj, "description", 2000),
            homepage: text(obj, "homepage", 500).filter(|u| Url::parse(u).is_ok_and(|u| u.scheme() == "https")),
            icon: None,
            hosts,
            types,
            settings,
            quality,
            qualities,
            functions: Vec::new(),
        })
    }

    pub fn can_search(&self) -> bool {
        SEARCH_FUNCTIONS.iter().any(|f| self.functions.iter().any(|g| g == f))
    }

    pub fn can_stream(&self) -> bool {
        self.functions.iter().any(|f| f == STREAM_FUNCTION)
    }
}

/// Largest icon kept.
pub const MAX_ICON_BYTES: usize = 256 * 1024;

/// The icon file a manifest names (`icon`, default `icon.png`), if it is a safe relative path.
pub fn icon_name(manifest: &[u8]) -> Option<String> {
    let doc: Value = serde_json::from_slice(manifest).ok()?;
    let name = doc.get("icon").and_then(Value::as_str).unwrap_or("icon.png").trim().trim_start_matches("./");
    let safe = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('/')
        && !name.contains('\\')
        && name.split('/').all(|s| !s.is_empty() && s != "..");
    safe.then(|| name.to_owned())
}

/// Image bytes → a `data:` URL (PNG, JPEG or WebP only, checked by their magic bytes).
pub fn icon_data_url(bytes: &[u8]) -> Option<String> {
    use base64::Engine as _;
    if bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// An installable script extension: its manifest and index.js.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptExtension {
    pub manifest: ScriptManifest,
    pub code: String,
}

impl ScriptExtension {
    /// Builds an extension from manifest.json and index.js bytes, then runs index.js once (no
    /// function is called) to learn which functions it registers.
    pub fn load(manifest: &[u8], code: &[u8]) -> AppResult<ScriptExtension> {
        let mut manifest = ScriptManifest::parse(manifest)?;
        if code.len() > MAX_SCRIPT_BYTES {
            return Err(invalid("index.js is larger than 8 MB"));
        }
        let code = String::from_utf8(code.to_vec()).map_err(|_| invalid("index.js is not UTF-8"))?;
        manifest.functions = runtime::registered_functions(&code)?;
        Ok(ScriptExtension { manifest, code })
    }
}

// ---- Results → tracks ----

fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn first_text(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| item.get(*k).and_then(str_of))
}

fn artists(item: &Value) -> Vec<String> {
    match item.get("artists").or_else(|| item.get("artist")) {
        Some(Value::String(s)) => s.split(", ").map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect(),
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|v| str_of(v).or_else(|| v.get("name").and_then(str_of)))
            .collect(),
        _ => Vec::new(),
    }
}

fn artwork(item: &Value) -> Option<String> {
    let pick = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => a.iter().find_map(|i| i.as_str().map(str::to_owned).or_else(|| i.get("url").and_then(str_of))),
        _ => None,
    };
    ["images", "cover_url", "image_url", "artwork", "thumbnail"]
        .iter()
        .find_map(|k| item.get(*k).and_then(pick))
        .filter(|u| Url::parse(u).is_ok_and(|u| u.scheme() == "https"))
}

fn duration_ms(item: &Value) -> Option<u32> {
    let ms = item.get("duration_ms").and_then(Value::as_f64).or_else(|| {
        item.get("duration").and_then(Value::as_f64).map(|s| s * 1000.0)
    })?;
    (ms.is_finite() && ms > 0.0 && ms < f64::from(u32::MAX)).then(|| ms.round() as u32)
}

/// Maps a search answer (an array of Track objects, or `{ tracks: [...] }`) to tracks.
/// Non-track items (`item_type` album / artist / playlist) and items without id or name are
/// skipped.
pub fn map_search(ext_id: &str, body: &Value) -> AppResult<Vec<MappedItem>> {
    let list = body
        .as_array()
        .or_else(|| body.get("tracks").and_then(Value::as_array))
        .or_else(|| body.get("results").and_then(Value::as_array))
        .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "search did not return a list"))?;
    Ok(list
        .iter()
        .filter(|item| item.get("item_type").and_then(Value::as_str).is_none_or(|t| t == "track"))
        .filter_map(|item| {
            let item_id = first_text(item, &["id", "track_id"])?;
            let title = first_text(item, &["name", "title"])?;
            if item_id.len() > 200 {
                return None;
            }
            let web = first_text(item, &["external_url", "url", "share_url"])
                .filter(|u| Url::parse(u).is_ok_and(|u| u.scheme() == "https"))
                .unwrap_or_default();
            Some(MappedItem {
                track: TrackInput {
                    item_id: format!("{ext_id}:{item_id}"),
                    url: web,
                    title,
                    artists: artists(item),
                    album: first_text(item, &["album_name", "album"]),
                    duration_ms: duration_ms(item),
                    isrc: first_text(item, &["isrc"]),
                    artwork: None,
                    available: true,
                },
                artwork_remote: artwork(item),
                item_id,
            })
        })
        .take(MAX_ITEMS)
        .collect())
}

/// Reads the URL out of a `getDownloadUrl` answer (`{ success, url }` or a bare string).
pub fn stream_url(body: &Value) -> AppResult<Url> {
    if body.get("success").and_then(Value::as_bool) == Some(false) {
        let why = body.get("error").and_then(Value::as_str).unwrap_or("extension found no stream");
        return Err(AppError::new(ErrorCode::ExtensionFailed, why.chars().take(300).collect::<String>()));
    }
    let raw = body
        .as_str()
        .or_else(|| body.get("url").and_then(Value::as_str))
        .ok_or_else(|| AppError::new(ErrorCode::ExtensionFailed, "no stream URL in answer"))?;
    Url::parse(raw).map_err(|e| AppError::new(ErrorCode::ExtensionFailed, format!("bad stream URL: {e}")))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub fn manifest_json(host: &str) -> Value {
        json!({
            "name": "Mock Stream", "displayName": "Mock Stream A", "version": "1.0.0",
            "description": "Mock provider", "author": { "name": "Someone" },
            "permissions": { "network": [host, "*.cdn.example"], "storage": true, "file": false },
            "type": ["metadata_provider", "download_provider"],
            "qualityOptions": [{ "id": "LOSSLESS", "label": "Lossless" }],
            "settings": [{ "key": "region", "type": "string", "label": "Region", "default": "US" },
                         { "key": "login", "type": "button", "label": "Log in", "action": "startLogin" }]
        })
    }

    #[test]
    fn parses_a_script_manifest() {
        let m = ScriptManifest::parse(manifest_json("API.example.org").to_string().as_bytes()).unwrap();
        assert_eq!(m.id, "mock-stream");
        assert_eq!(m.name, "Mock Stream A");
        assert_eq!(m.author.as_deref(), Some("Someone"));
        assert_eq!(m.hosts, vec!["api.example.org", "*.cdn.example"]);
        assert_eq!(m.types, vec!["metadata_provider", "download_provider"]);
        assert_eq!(m.settings.get("region"), Some(&json!("US")));
        assert_eq!(m.quality.as_deref(), Some("LOSSLESS"));
        assert_eq!(m.qualities, vec![QualityOption { id: "LOSSLESS".into(), label: "Lossless".into(), description: None }]);

        let bad = |f: &dyn Fn(&mut Value)| {
            let mut v = manifest_json("api.example.org");
            f(&mut v);
            ScriptManifest::parse(v.to_string().as_bytes()).unwrap_err().code
        };
        assert_eq!(bad(&|v| v["name"] = json!("!!")), ErrorCode::ExtensionInvalid);
        assert_eq!(bad(&|v| v["version"] = json!("")), ErrorCode::ExtensionInvalid);
        assert_eq!(bad(&|v| v["permissions"]["network"] = json!(["bad host"])), ErrorCode::ExtensionInvalid);
        assert_eq!(ScriptManifest::parse(b"[]").unwrap_err().code, ErrorCode::ExtensionInvalid);
    }

    #[test]
    fn maps_track_objects() {
        let body = json!([
            { "id": "t1", "name": "Song", "artists": "A, B", "album_name": "Album", "duration_ms": 200000,
              "isrc": "USABC1234567", "images": "https://img.example/1.jpg", "external_url": "https://open.example/t1" },
            { "id": 7, "title": "Other", "artists": [{ "name": "C" }], "cover_url": "http://insecure/x.jpg" },
            { "id": "al1", "name": "An album", "item_type": "album" },
            { "name": "no id" }
        ]);
        let items = map_search("ext", &body).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].track.item_id, "ext:t1");
        assert_eq!(items[0].track.artists, vec!["A", "B"]);
        assert_eq!(items[0].track.duration_ms, Some(200_000));
        assert_eq!(items[0].artwork_remote.as_deref(), Some("https://img.example/1.jpg"));
        assert_eq!(items[0].track.url, "https://open.example/t1");
        assert_eq!(items[1].item_id, "7");
        assert_eq!(items[1].artwork_remote, None, "http artwork is dropped");
        assert_eq!(map_search("ext", &json!({ "tracks": [{ "id": "x", "name": "y" }] })).unwrap().len(), 1);
        assert!(map_search("ext", &json!({ "nope": 1 })).is_err());
    }

    #[test]
    fn reads_stream_urls() {
        assert_eq!(stream_url(&json!({ "success": true, "url": "https://a.example/x.flac" })).unwrap().host_str(), Some("a.example"));
        assert_eq!(stream_url(&json!("https://a.example/y.mp3")).unwrap().path(), "/y.mp3");
        assert_eq!(
            stream_url(&json!({ "success": false, "error": "not found" })).unwrap_err().message,
            "not found"
        );
        assert!(stream_url(&json!({ "success": true })).is_err());
    }

    #[test]
    fn icons_become_data_urls() {
        assert_eq!(icon_name(br#"{"icon":"assets/logo.png"}"#).as_deref(), Some("assets/logo.png"));
        assert_eq!(icon_name(b"{}").as_deref(), Some("icon.png"));
        assert_eq!(icon_name(br#"{"icon":"../x.png"}"#), None);
        assert_eq!(icon_name(br#"{"icon":"/etc/x.png"}"#), None);
        let png = b"\x89PNG\r\n\x1a\nrest";
        assert!(icon_data_url(png).unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(icon_data_url(b"<svg/>"), None, "only raster images");
        assert_eq!(icon_data_url(&vec![0xff; MAX_ICON_BYTES + 1]), None);
    }

    #[test]
    fn ids_from_names() {
        assert_eq!(id_from("My Cool Ext!"), "my-cool-ext");
        assert_eq!(id_from("  tidal_hifi  "), "tidal-hifi");
        assert_eq!(id_from("ab"), "ab");
    }
}
