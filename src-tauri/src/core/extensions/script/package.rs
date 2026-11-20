//! Packaged script extensions: a zip (`.spotiflac-ext` / `.sflx`) with manifest.json and
//! index.js at its root (or inside one top-level folder). Only those two files are read.

use std::io::{Cursor, Read};

use sha2::{Digest, Sha256};
use url::Url;
use zip::ZipArchive;

use super::{icon_data_url, icon_name, ScriptExtension, MAX_ICON_BYTES, MAX_SCRIPT_BYTES};
use crate::core::extensions::manifest::MAX_MANIFEST_BYTES;
use crate::error::{AppError, AppResult, ErrorCode};

/// Largest package downloaded.
pub const MAX_PACKAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_ENTRIES: usize = 500;
/// File name endings of packages.
const SUFFIXES: [&str; 3] = [".spotiflac-ext", ".sflx", ".zip"];

fn invalid(why: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ExtensionInvalid, why)
}

/// Whether a link points at a package file (by its file name).
pub fn is_package_link(link: &Url) -> bool {
    let path = link.path().to_ascii_lowercase();
    SUFFIXES.iter().any(|s| path.trim_end_matches('/').ends_with(s))
}

/// Parses a pasted package link: https only (loopback http in debug builds, for fixtures).
pub fn parse_link(input: &str) -> Option<Url> {
    let url = Url::parse(input.trim()).ok()?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    let scheme_ok = url.scheme() == "https" || (cfg!(debug_assertions) && loopback && url.scheme() == "http");
    (scheme_ok && url.host_str().is_some() && is_package_link(&url)).then_some(url)
}

/// Hex SHA-256 of the package bytes (what an install is pinned to).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn read_entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str, cap: usize) -> AppResult<Vec<u8>> {
    let file = zip.by_name(name).map_err(|_| invalid(format!("package has no {name}")))?;
    if file.size() > cap as u64 {
        return Err(invalid(format!("{name} in the package is too large")));
    }
    let mut out = Vec::new();
    file.take(cap as u64 + 1).read_to_end(&mut out).map_err(|e| invalid(format!("unreadable {name}: {e}")))?;
    if out.len() > cap {
        return Err(invalid(format!("{name} in the package is too large")));
    }
    Ok(out)
}

/// Reads a package into an extension (runs index.js once to list its functions).
pub fn read(bytes: &[u8]) -> AppResult<ScriptExtension> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err(invalid("package is larger than 20 MB"));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| invalid(format!("not a zip package: {e}")))?;
    if zip.len() > MAX_ENTRIES {
        return Err(invalid("package has too many files"));
    }
    let names: Vec<String> = zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect();
    let prefix = if names.iter().any(|n| n == "manifest.json") {
        String::new()
    } else {
        // One wrapping folder (`my-ext/manifest.json`) is accepted too.
        names
            .iter()
            .find_map(|n| n.strip_suffix("manifest.json").filter(|p| p.ends_with('/') && p.matches('/').count() == 1))
            .map(str::to_owned)
            .ok_or_else(|| invalid("package has no manifest.json at its root"))?
    };
    let manifest = read_entry(&mut zip, &format!("{prefix}manifest.json"), MAX_MANIFEST_BYTES)?;
    let code = read_entry(&mut zip, &format!("{prefix}index.js"), MAX_SCRIPT_BYTES)?;
    let mut ext = ScriptExtension::load(&manifest, &code)?;
    // The icon is optional: a missing or unusable one is ignored.
    ext.manifest.icon = icon_name(&manifest)
        .and_then(|name| read_entry(&mut zip, &format!("{prefix}{name}"), MAX_ICON_BYTES).ok())
        .and_then(|bytes| icon_data_url(&bytes));
    Ok(ext)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Builds a package zip in memory (tests only; the app never writes zips).
    pub fn build(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            for (name, body) in files {
                w.start_file(*name, SimpleFileOptions::default()).unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    const CODE: &str = "registerExtension({ initialize(){ return true; }, searchTracks(q){ return []; } });";

    #[test]
    fn reads_root_and_wrapped_packages() {
        let manifest = super::super::tests::manifest_json("api.example.org").to_string();
        let ext = read(&build(&[("manifest.json", &manifest), ("index.js", CODE), ("icon.png", "x")])).unwrap();
        assert_eq!(ext.manifest.id, "mock-stream");
        assert_eq!(ext.manifest.icon, None, "not an image");
        assert_eq!(ext.manifest.functions, vec!["initialize", "searchTracks"]);
        assert!(ext.manifest.can_search() && !ext.manifest.can_stream());

        let wrapped = read(&build(&[("pkg/manifest.json", &manifest), ("pkg/index.js", CODE)])).unwrap();
        assert_eq!(wrapped.code, CODE);

        for bad in [
            build(&[("index.js", CODE)]),
            build(&[("manifest.json", &manifest)]),
            build(&[("a/b/manifest.json", &manifest), ("a/b/index.js", CODE)]),
            b"not a zip".to_vec(),
        ] {
            assert_eq!(read(&bad).unwrap_err().code, ErrorCode::ExtensionInvalid);
        }
        let silent = read(&build(&[("manifest.json", &manifest), ("index.js", "var x = 1;")])).unwrap_err();
        assert_eq!(silent.code, ErrorCode::ExtensionInvalid, "{}", silent.message);
    }

    #[test]
    fn recognises_package_links() {
        assert!(parse_link("https://github.com/a/b/releases/download/v1/x.spotiflac-ext").is_some());
        assert!(parse_link(" https://example.com/ext/X.SFLX ").is_some());
        assert!(parse_link("http://example.com/x.sflx").is_none());
        assert!(parse_link("https://github.com/a/b").is_none());
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
