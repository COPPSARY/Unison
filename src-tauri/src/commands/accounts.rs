//! Account commands. Thin wrappers: provider logic lives in `providers/`.

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;

use crate::core::db;
use crate::error::{AppError, AppResult};
use crate::model::{Account, ProviderId};
use crate::providers::spotify::{self, connect::ConnectContext};
use crate::state::AppState;

/// One entry per provider with its status.
#[tauri::command]
#[specta::specta]
pub async fn accounts_list(state: State<'_, AppState>) -> AppResult<Vec<Account>> {
    db::accounts::list(&state.db).await
}

/// Validates and stores the Spotify Client ID in the keychain. Never echoed back.
#[tauri::command]
#[specta::specta]
pub async fn spotify_set_client_id(state: State<'_, AppState>, client_id: String) -> AppResult<()> {
    spotify::connect::set_client_id(state.secrets.as_ref(), &client_id)
}

/// Whether a Spotify Client ID is saved (the UI never sees the value).
#[tauri::command]
#[specta::specta]
pub async fn spotify_has_client_id(state: State<'_, AppState>) -> AppResult<bool> {
    spotify::connect::has_client_id(state.secrets.as_ref())
}

/// Spotify onboarding: redirect URIs to register in the developer app.
#[tauri::command]
#[specta::specta]
pub fn spotify_redirect_uris() -> Vec<String> {
    spotify::redirect_uris()
}

/// Opens the Spotify Developer Dashboard in the default browser.
#[tauri::command]
#[specta::specta]
pub fn spotify_open_dashboard(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url(spotify::DASHBOARD_URL, None::<&str>)
        .map_err(|e| AppError::internal(format!("open dashboard: {e}")))
}

/// Runs the interactive login for `provider` (5-minute timeout, cancellable).
#[tauri::command]
#[specta::specta]
pub async fn account_connect(app: AppHandle, state: State<'_, AppState>, provider: ProviderId) -> AppResult<Account> {
    let (tx, rx) = oneshot::channel();
    state.replace_pending_auth(tx);

    let open = |url: &str| {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| AppError::internal(format!("open browser: {e}")))
    };
    let result = match provider {
        ProviderId::Spotify => {
            let ctx = ConnectContext::new(&state.http, state.secrets.as_ref(), &state.db);
            spotify::connect::connect(&ctx, rx, open).await
        }
        ProviderId::Youtube => Err(AppError::internal("YouTube sign-in arrives in a later update")),
    };
    state.clear_pending_auth();
    if let Err(err) = &result {
        tracing::warn!(provider = provider.as_str(), code = ?err.code, "connect failed");
    }
    result
}

/// Cancels a connect that is waiting for the browser.
#[tauri::command]
#[specta::specta]
pub fn account_cancel_connect(state: State<'_, AppState>) {
    state.cancel_pending_auth();
}
