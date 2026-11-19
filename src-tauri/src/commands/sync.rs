use tauri::{AppHandle, State};

use crate::core::db;
use crate::core::sync::scheduler::spawn_sync;
use crate::error::AppResult;
use crate::model::{AccountStatus, ProviderId};
use crate::state::AppState;

/// Starts a sync now (all connected providers when `provider` is null). Returns immediately;
/// progress arrives as `sync://progress` / `sync://finished` events.
#[tauri::command]
#[specta::specta]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>, provider: Option<ProviderId>) -> AppResult<()> {
    let accounts = db::accounts::list(&state.db).await?;
    for account in accounts {
        let wanted = provider.is_none_or(|p| p == account.provider);
        if wanted && account.status == AccountStatus::Connected {
            spawn_sync(&app, account.provider);
        }
    }
    Ok(())
}

/// Cancels a running sync for `provider`.
#[tauri::command]
#[specta::specta]
pub fn sync_cancel(state: State<'_, AppState>, provider: ProviderId) {
    state.sync.cancel(provider);
}
