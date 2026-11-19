//! `accounts` table: one row per connected provider (no tokens here — keychain only).

use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::model::{Account, AccountStatus, ProviderId};

fn status_to_db(status: AccountStatus) -> &'static str {
    match status {
        AccountStatus::NeedsReconnect => "needs_reconnect",
        _ => "connected",
    }
}

fn status_from_db(value: &str) -> AccountStatus {
    match value {
        "needs_reconnect" => AccountStatus::NeedsReconnect,
        _ => AccountStatus::Connected,
    }
}

/// Inserts or replaces the account row after a successful connect.
pub async fn upsert_connected(
    pool: &SqlitePool,
    provider: ProviderId,
    account_id: &str,
    display_name: Option<&str>,
    now: i64,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO accounts (provider, account_id, display_name, status, connected_at)
         VALUES (?1, ?2, ?3, 'connected', ?4)
         ON CONFLICT(provider) DO UPDATE SET
           account_id = excluded.account_id,
           display_name = excluded.display_name,
           status = 'connected',
           connected_at = excluded.connected_at",
    )
    .bind(provider.as_str())
    .bind(account_id)
    .bind(display_name)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_status(pool: &SqlitePool, provider: ProviderId, status: AccountStatus) -> AppResult<()> {
    sqlx::query("UPDATE accounts SET status = ?1 WHERE provider = ?2")
        .bind(status_to_db(status))
        .bind(provider.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

/// One entry per known provider; missing rows are `NotConnected`.
pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Account>> {
    let rows: Vec<(String, Option<String>, String, Option<i64>)> = sqlx::query_as(
        "SELECT provider, display_name, status, last_synced_at FROM accounts",
    )
    .fetch_all(pool)
    .await?;
    Ok(ProviderId::ALL
        .into_iter()
        .map(|provider| {
            rows.iter()
                .find(|(p, ..)| p == provider.as_str())
                .map(|(_, name, status, synced)| Account {
                    provider,
                    status: status_from_db(status),
                    display_name: name.clone(),
                    last_synced_at: *synced,
                })
                .unwrap_or(Account {
                    provider,
                    status: AccountStatus::NotConnected,
                    display_name: None,
                    last_synced_at: None,
                })
        })
        .collect())
}

pub async fn delete(pool: &SqlitePool, provider: ProviderId) -> AppResult<()> {
    sqlx::query("DELETE FROM accounts WHERE provider = ?1")
        .bind(provider.as_str())
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db;

    #[tokio::test]
    async fn list_upsert_status_delete() {
        let dir = tempfile::tempdir().unwrap();
        let pool = db::open(&dir.path().join(db::DB_FILE)).await.unwrap();

        let all = list(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|a| a.status == AccountStatus::NotConnected));

        upsert_connected(&pool, ProviderId::Spotify, "u1", Some("Demo"), 100).await.unwrap();
        upsert_connected(&pool, ProviderId::Spotify, "u1", Some("Demo 2"), 200).await.unwrap();
        let sp = list(&pool).await.unwrap().into_iter().find(|a| a.provider == ProviderId::Spotify).unwrap();
        assert_eq!(sp.status, AccountStatus::Connected);
        assert_eq!(sp.display_name.as_deref(), Some("Demo 2"));

        set_status(&pool, ProviderId::Spotify, AccountStatus::NeedsReconnect).await.unwrap();
        let sp = list(&pool).await.unwrap().into_iter().find(|a| a.provider == ProviderId::Spotify).unwrap();
        assert_eq!(sp.status, AccountStatus::NeedsReconnect);

        delete(&pool, ProviderId::Spotify).await.unwrap();
        assert!(list(&pool).await.unwrap().iter().all(|a| a.status == AccountStatus::NotConnected));
    }
}
