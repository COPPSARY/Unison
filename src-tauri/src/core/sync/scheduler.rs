//! Background sync: runs a provider sync in a task, and re-syncs connected accounts whose last
//! sync is older than the interval in Settings (spec §8.5, default 30 min).

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use super::{SyncFinished, SyncReporter, SyncSummary};
use crate::core::auth::oauth::now_unix;
use crate::core::db;
use crate::model::{AccountStatus, ProviderId};
use crate::providers::spotify;
use crate::state::AppState;

const FIRST_CHECK: Duration = Duration::from_secs(5);
const CHECK_EVERY: Duration = Duration::from_secs(60);

/// Starts a sync for `provider` in the background unless one is already running.
/// Returns false if it was already running.
pub fn spawn_sync<R: Runtime>(app: &AppHandle<R>, provider: ProviderId) -> bool {
    let state = app.state::<AppState>();
    let Some(cancel) = state.sync.begin(provider) else {
        return false;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        tracing::info!(provider = provider.as_str(), "sync started");
        let result = match provider {
            ProviderId::Spotify => spotify::sync::run(&state.spotify, &state.db, &app, &cancel).await,
            // YouTube sync arrives with T-22.
            ProviderId::Youtube => Ok(SyncSummary::default()),
        };
        // Owned playlists load their items in the background after a successful sync (§8.5).
        if result.is_ok() && provider == ProviderId::Spotify {
            state.playlist_loader().prefetch_owned().await;
        }
        state.sync.end(provider);
        let event = match result {
            Ok(s) => SyncFinished { provider, added: s.added, removed: s.removed, error: None },
            Err(err) => {
                tracing::warn!(provider = provider.as_str(), error = %err, "sync failed");
                SyncFinished { provider, added: 0, removed: 0, error: Some(err) }
            }
        };
        app.finished(event);
    });
    true
}

/// Whether an account is due for a background sync.
pub fn is_due(status: AccountStatus, last_synced_at: Option<i64>, interval_min: u32, now: i64) -> bool {
    status == AccountStatus::Connected
        && last_synced_at.is_none_or(|last| now - last >= i64::from(interval_min) * 60)
}

async fn tick<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let Ok(settings) = db::settings::load(&state.db).await else { return };
    let Ok(accounts) = db::accounts::list(&state.db).await else { return };
    let now = now_unix();
    for account in accounts {
        // YouTube is parked (no Google project yet); skip it until T-22.
        // A failed attempt counts too, so errors are not retried every minute.
        let last = account.last_synced_at.max(state.sync.last_attempt(account.provider));
        if account.provider == ProviderId::Spotify
            && is_due(account.status, last, settings.sync_interval_min, now)
        {
            spawn_sync(app, account.provider);
        }
    }
}

/// Spawns the periodic checker. Call once from `setup`.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            tick(&app).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_rules() {
        assert!(is_due(AccountStatus::Connected, None, 30, 1_000));
        assert!(is_due(AccountStatus::Connected, Some(0), 30, 1_800));
        assert!(!is_due(AccountStatus::Connected, Some(1_000), 30, 2_000));
        assert!(!is_due(AccountStatus::NeedsReconnect, None, 30, 1_000));
        assert!(!is_due(AccountStatus::NotConnected, None, 30, 1_000));
    }
}
