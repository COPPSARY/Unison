//! Open licences on extension results (EXT-07). A source may be saved to disk only when its
//! extension says the track is under an open licence Unison recognises: a Creative Commons
//! licence, CC0 or the public domain. Anything else (no licence, "all rights reserved", a
//! licence Unison doesn't know) can't be downloaded.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use url::Url;

/// A recognised open licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenLicense {
    /// Short name shown to the user: "CC BY-SA 4.0", "CC0 1.0", "Public domain".
    pub name: String,
    /// The licence's page (https, creativecommons.org) when known.
    pub url: Option<String>,
}

const CC_KINDS: &[&str] = &["by", "by-sa", "by-nd", "by-nc", "by-nc-sa", "by-nc-nd"];

fn version_ok(v: &str) -> bool {
    !v.is_empty() && v.len() <= 4 && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// A creativecommons.org licence or public-domain URL.
fn from_url(raw: &str) -> Option<OpenLicense> {
    let url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "https" | "http") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host != "creativecommons.org" && host != "www.creativecommons.org" {
        return None;
    }
    let parts: Vec<String> = url.path_segments()?.filter(|s| !s.is_empty()).map(str::to_ascii_lowercase).collect();
    let page = |path: &str| Some(format!("https://creativecommons.org/{path}/"));
    match parts.as_slice() {
        [l, kind, v, ..] if l == "licenses" && CC_KINDS.contains(&kind.as_str()) && version_ok(v) => Some(OpenLicense {
            name: format!("CC {} {v}", kind.to_ascii_uppercase()),
            url: page(&format!("licenses/{kind}/{v}")),
        }),
        [p, z, v, ..] if p == "publicdomain" && z == "zero" && version_ok(v) => {
            Some(OpenLicense { name: format!("CC0 {v}"), url: page(&format!("publicdomain/zero/{v}")) })
        }
        [p, m, ..] if p == "publicdomain" && m == "mark" => {
            Some(OpenLicense { name: "Public domain".to_owned(), url: page("publicdomain/mark/1.0") })
        }
        _ => None,
    }
}

/// A licence name: "CC BY-SA 4.0", "cc-by-nc-3.0", "CC0", "Creative Commons Attribution 4.0",
/// "Public Domain".
fn from_name(raw: &str) -> Option<OpenLicense> {
    let text = raw.trim().to_ascii_lowercase();
    if text.is_empty() || text.len() > 120 {
        return None;
    }
    if text.contains("public domain") || text == "pd" || text == "pdm" {
        return Some(OpenLicense { name: "Public domain".to_owned(), url: None });
    }
    let words: Vec<&str> = text.split(|c: char| c.is_whitespace() || c == '_').filter(|w| !w.is_empty()).collect();
    let flat = words.join("-");
    if flat == "cc0" || flat.starts_with("cc0-") || flat.starts_with("cc-zero") {
        return Some(OpenLicense { name: "CC0".to_owned(), url: None });
    }
    let rest = flat.strip_prefix("cc-")?;
    // "by-sa-4.0" → kind "by-sa", version "4.0".
    let (kind, version) = match rest.rsplit_once('-') {
        Some((k, v)) if version_ok(v) => (k, Some(v)),
        _ => (rest, None),
    };
    if !CC_KINDS.contains(&kind) {
        return None;
    }
    let name = match version {
        Some(v) => format!("CC {} {v}", kind.to_ascii_uppercase()),
        None => format!("CC {}", kind.to_ascii_uppercase()),
    };
    let url = version.map(|v| format!("https://creativecommons.org/licenses/{kind}/{v}/"));
    Some(OpenLicense { name, url })
}

/// Reads a licence from one value: a URL, a name, or `{ url, name }`.
pub fn parse(value: &Value) -> Option<OpenLicense> {
    match value {
        Value::String(s) => from_url(s).or_else(|| from_name(s)),
        Value::Object(o) => ["url", "href", "link"]
            .iter()
            .filter_map(|k| o.get(*k).and_then(Value::as_str))
            .find_map(from_url)
            .or_else(|| ["name", "label", "title", "id"].iter().filter_map(|k| o.get(*k).and_then(Value::as_str)).find_map(from_name)),
        _ => None,
    }
}

/// The open licence a script search result declares (`license`, `license_url`, `licenseUrl`).
pub fn of_item(item: &Value) -> Option<OpenLicense> {
    ["license_url", "licenseUrl", "license", "licence"].iter().filter_map(|k| item.get(*k)).find_map(parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn name(v: Value) -> Option<String> {
        parse(&v).map(|l| l.name)
    }

    #[test]
    fn recognises_creative_commons_urls() {
        let l = parse(&json!("https://creativecommons.org/licenses/by-nc-sa/3.0/de/")).unwrap();
        assert_eq!(l.name, "CC BY-NC-SA 3.0");
        assert_eq!(l.url.as_deref(), Some("https://creativecommons.org/licenses/by-nc-sa/3.0/"));
        assert_eq!(name(json!("http://creativecommons.org/publicdomain/zero/1.0/")).as_deref(), Some("CC0 1.0"));
        assert_eq!(name(json!("https://creativecommons.org/publicdomain/mark/1.0/")).as_deref(), Some("Public domain"));
    }

    #[test]
    fn recognises_licence_names() {
        assert_eq!(name(json!("CC BY-SA 4.0")).as_deref(), Some("CC BY-SA 4.0"));
        assert_eq!(name(json!("cc-by-nc-3.0")).as_deref(), Some("CC BY-NC 3.0"));
        assert_eq!(name(json!("CC0")).as_deref(), Some("CC0"));
        assert_eq!(name(json!("Public Domain")).as_deref(), Some("Public domain"));
        assert_eq!(name(json!({ "name": "CC BY 4.0" })).as_deref(), Some("CC BY 4.0"));
        assert_eq!(name(json!({ "url": "https://creativecommons.org/licenses/by/4.0/" })).as_deref(), Some("CC BY 4.0"));
    }

    #[test]
    fn anything_else_is_not_open() {
        for v in [
            json!("All rights reserved"),
            json!("https://example.com/licenses/by/4.0/"),
            json!("https://creativecommons.org/licenses/evil/4.0/"),
            json!("CC-BY-FREE"),
            json!("proprietary"),
            json!(true),
            json!(null),
            json!(""),
        ] {
            assert_eq!(parse(&v), None, "{v}");
        }
    }

    #[test]
    fn reads_the_licence_fields_of_a_result() {
        assert_eq!(of_item(&json!({ "license": "CC BY 3.0" })).map(|l| l.name).as_deref(), Some("CC BY 3.0"));
        assert_eq!(
            of_item(&json!({ "licenseUrl": "https://creativecommons.org/licenses/by-sa/2.0/" })).map(|l| l.name).as_deref(),
            Some("CC BY-SA 2.0")
        );
        assert_eq!(of_item(&json!({ "downloadable": true })), None, "a flag alone is not a licence");
    }
}
