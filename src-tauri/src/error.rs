//! App-wide error type (spec §9). Every command returns `Result<T, AppError>`; the UI maps
//! `code` to an i18n message and shows the one next step. Messages are for logs/devs and must
//! never contain tokens, client IDs or other secrets.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Machine-readable error kind, serialised as `{ "type": "rateLimited", "retryAfter": 30 }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ErrorCode {
    Offline,
    RateLimited {
        /// Seconds until the next request is allowed.
        #[serde(rename = "retryAfter")]
        retry_after: u32,
    },
    NotFound,
    Upstream {
        status: u16,
    },
    // Universal Player (addendum §6).
    NoPlayableSource,
    FileMissing,
    UnsupportedFormat,
    DrmProtected,
    AudiusUnavailable,
    OutputDeviceUnavailable,
    // Extensions (EXT-01).
    /// The pasted link is not an extension, package or registry link.
    ExtensionLinkInvalid,
    /// extension.json is missing, malformed, too large or breaks a rule.
    ExtensionInvalid,
    /// A URL (or a redirect) points at a host the extension didn't declare, or isn't https.
    ExtensionHostNotAllowed,
    /// The extension is turned off or was removed.
    ExtensionDisabled,
    /// The extension's service answered with something Unison couldn't use.
    ExtensionFailed,
    /// registry.json is missing, malformed, too large or lists no extensions.
    ExtensionRegistryInvalid,
    /// The entry or extension can't be used here (not an installable link, or it can't play).
    ExtensionUnsupported,
    /// The extension requires a runtime feature Unison refuses (e.g. signed sessions against
    /// a service's private API, or removing copy protection), so it can't be turned on.
    ExtensionUnsupportedFeature {
        extension: String,
        feature: String,
    },
    /// The source can't be downloaded: it has no open licence, or its extension is off,
    /// refused or gives no audio (EXT-07).
    DownloadNotAllowed,
    Internal,
}

impl ErrorCode {
    /// Whether retrying the same action later can succeed.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            ErrorCode::Offline
                | ErrorCode::RateLimited { .. }
                | ErrorCode::Upstream { .. }
        )
    }
}

/// Error returned by every command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let retryable = code.retryable();
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }

    /// Unexpected failure. The detail is logged; the UI shows a generic message.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::not_found("row not found"),
            other => AppError::internal(format!("database: {other}")),
        }
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(err: sqlx::migrate::MigrateError) -> Self {
        AppError::internal(format!("migration: {err}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::internal(format!("io: {err}"))
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::internal(format!("tauri: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialises_unit_and_data_codes_for_the_ui() {
        let err = AppError::new(ErrorCode::RateLimited { retry_after: 30 }, "slow down");
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({
                "code": { "type": "rateLimited", "retryAfter": 30 },
                "message": "slow down",
                "retryable": true
            })
        );
        let offline = serde_json::to_value(AppError::new(ErrorCode::Offline, "x")).unwrap();
        assert_eq!(offline["code"], json!({ "type": "offline" }));
    }

    #[test]
    fn retryable_follows_the_code() {
        assert!(!AppError::new(ErrorCode::ExtensionInvalid, "x").retryable);
        assert!(AppError::new(ErrorCode::Upstream { status: 503 }, "x").retryable);
        assert!(!AppError::internal("x").retryable);
    }

    #[test]
    fn row_not_found_maps_to_not_found() {
        assert_eq!(AppError::from(sqlx::Error::RowNotFound).code, ErrorCode::NotFound);
    }
}
