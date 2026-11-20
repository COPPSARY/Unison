//! Runtime features a script extension may require (`requiredRuntimeFeatures` in its
//! manifest.json). Unison only implements features it can support without getting around copy
//! protection. An extension that requires anything outside [`SUPPORTED`] can't be installed or
//! turned on, and is treated as off if it is already stored (see CLAUDE.md, "Extension runtime
//! features"). Adding a supported feature needs a decision record in docs/decisions.

use crate::error::{AppError, AppResult, ErrorCode};

/// Features this build implements. None yet: everything Unison offers scripts (HTTP to declared
/// hosts, storage, keychain credentials, utils) is always there and needs no feature flag.
pub const SUPPORTED: &[&str] = &[];

/// Features that exist to call a streaming service's private API with a signing secret or a
/// session handshake. Never implemented.
const REJECTED: &[&str] = &["signedSession@1", "sessionGrant@1", "preparedContext@1"];

/// Words that mark a feature as removing content protection (DRM, encrypted Dolby streams).
const DRM_WORDS: &[&str] =
    &["drm", "decrypt", "widevine", "playready", "fairplay", "cenc", "ac4", "ac-4", "eac3", "e-ac-3", "encrypted"];

/// Why a required feature can't be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Signed sessions / session grants against a service's private API.
    PrivateApi,
    /// Removing content protection.
    Drm,
    /// Not implemented in this build.
    Unknown,
}

/// Why `feature` can't be used, or `None` when it is supported.
pub fn refusal(feature: &str) -> Option<Refusal> {
    let f = feature.trim();
    if SUPPORTED.contains(&f) {
        return None;
    }
    let lower = f.to_ascii_lowercase();
    if REJECTED.iter().any(|r| r.eq_ignore_ascii_case(f)) {
        Some(Refusal::PrivateApi)
    } else if DRM_WORDS.iter().any(|w| lower.contains(w)) {
        Some(Refusal::Drm)
    } else {
        Some(Refusal::Unknown)
    }
}

/// The first required feature Unison won't run (rejected ones first), if any.
pub fn first_unsupported(required: &[String]) -> Option<&str> {
    let refused: Vec<(&str, Refusal)> =
        required.iter().filter_map(|f| refusal(f).map(|r| (f.as_str(), r))).collect();
    refused
        .iter()
        .find(|(_, r)| *r != Refusal::Unknown)
        .or_else(|| refused.first())
        .map(|(f, _)| *f)
}

/// `Err(ExtensionUnsupportedFeature)` naming the extension and the feature when `required`
/// holds anything Unison won't run.
pub fn check(extension: &str, required: &[String]) -> AppResult<()> {
    match first_unsupported(required) {
        None => Ok(()),
        Some(feature) => {
            let why = match refusal(feature) {
                Some(Refusal::PrivateApi) => "it signs requests to a streaming service's private API",
                Some(Refusal::Drm) => "it removes content protection",
                _ => "Unison doesn't support it",
            };
            Err(AppError::new(
                ErrorCode::ExtensionUnsupportedFeature { extension: extension.to_owned(), feature: feature.to_owned() },
                format!("extension {extension:?} requires runtime feature {feature:?}, which is refused: {why}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_required_features_is_fine() {
        assert!(check("jiosaavn", &[]).is_ok());
    }

    #[test]
    fn signed_session_is_refused_naming_extension_and_feature() {
        let err = check("amazon", &v(&["signedSession@1", "sessionGrant@1", "preparedContext@1"])).unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::ExtensionUnsupportedFeature { extension: "amazon".into(), feature: "signedSession@1".into() }
        );
        assert!(err.message.contains("amazon") && err.message.contains("signedSession@1"));
        assert!(!err.retryable);
    }

    #[test]
    fn each_rejected_feature_is_refused_on_its_own() {
        for f in ["signedSession@1", "sessionGrant@1", "preparedContext@1"] {
            assert_eq!(refusal(f), Some(Refusal::PrivateApi), "{f}");
            assert!(check("x", &v(&[f])).is_err());
        }
    }

    #[test]
    fn drm_features_are_refused() {
        for f in ["widevine@1", "ac4Decrypt@1", "encryptedEac3@2", "drmLicense@1"] {
            assert_eq!(refusal(f), Some(Refusal::Drm), "{f}");
        }
    }

    #[test]
    fn unknown_features_are_unsupported_and_rejected_ones_are_named_first() {
        assert_eq!(refusal("somethingNew@1"), Some(Refusal::Unknown));
        assert_eq!(first_unsupported(&v(&["somethingNew@1", "signedSession@1"])), Some("signedSession@1"));
        assert_eq!(first_unsupported(&v(&["somethingNew@1"])), Some("somethingNew@1"));
    }
}
