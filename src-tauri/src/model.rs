//! Types shared with the UI (exported to src/lib/bindings.ts by tauri-specta).
//! Library models (Track, Playlist, Account, …) arrive with T-06.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Build and schema information shown in Settings → About.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub schema_version: i64,
}
