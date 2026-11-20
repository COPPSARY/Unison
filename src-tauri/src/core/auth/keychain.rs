//! Secrets in the OS keychain (Windows Credential Manager), service `app.unison` (spec §7/§9).
//! Script extensions' credentials live only here — never SQLite, logs, events or the UI.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{AppError, AppResult};

pub const SERVICE: &str = "app.unison";

/// Storage for small secrets. `Keychain` in the app; `MemoryStore` in tests.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    fn set(&self, key: &str, value: &str) -> AppResult<()>;
    /// Deleting a missing key is not an error.
    fn delete(&self, key: &str) -> AppResult<()>;
}

/// OS keychain via the `keyring` crate.
pub struct Keychain;

fn keychain_error(err: keyring::Error) -> AppError {
    // keyring errors never contain the secret itself.
    AppError::internal(format!("keychain: {err}"))
}

impl SecretStore for Keychain {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(keychain_error)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keychain_error(err)),
        }
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        keyring::Entry::new(SERVICE, key)
            .and_then(|entry| entry.set_password(value))
            .map_err(keychain_error)
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(keychain_error)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keychain_error(err)),
        }
    }
}

/// In-memory store for tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        let map = self.0.lock().map_err(|_| AppError::internal("memory store poisoned"))?;
        Ok(map.get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        let mut map = self.0.lock().map_err(|_| AppError::internal("memory store poisoned"))?;
        map.insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        let mut map = self.0.lock().map_err(|_| AppError::internal("memory store poisoned"))?;
        map.remove(key);
        Ok(())
    }
}

/// JSON helpers on top of any store.
pub fn get_json<T: DeserializeOwned>(store: &dyn SecretStore, key: &str) -> AppResult<Option<T>> {
    match store.get(key)? {
        None => Ok(None),
        Some(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| AppError::internal(format!("keychain value {key} unreadable: {e}"))),
    }
}

pub fn set_json<T: Serialize>(store: &dyn SecretStore, key: &str, value: &T) -> AppResult<()> {
    let raw = serde_json::to_string(value)
        .map_err(|e| AppError::internal(format!("keychain encode {key}: {e}")))?;
    store.set(key, &raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip_and_idempotent_delete() {
        let store = MemoryStore::default();
        assert_eq!(store.get("k").unwrap(), None);
        store.set("k", "v").unwrap();
        assert_eq!(store.get("k").unwrap().as_deref(), Some("v"));
        store.delete("k").unwrap();
        store.delete("k").unwrap();
        assert_eq!(store.get("k").unwrap(), None);
    }

    #[test]
    fn json_round_trip() {
        let store = MemoryStore::default();
        let value = serde_json::json!({ "token": "x" });
        set_json(&store, "extension.demo.credentials", &value).unwrap();
        let back: serde_json::Value = get_json(&store, "extension.demo.credentials").unwrap().unwrap();
        assert_eq!(back, value);
    }

    /// Real Credential Manager round trip. Run manually: `cargo test -- --ignored keychain`.
    #[test]
    #[ignore = "touches the real OS keychain"]
    fn os_keychain_round_trip() {
        let key = "test.unison.roundtrip";
        Keychain.set(key, "value").unwrap();
        assert_eq!(Keychain.get(key).unwrap().as_deref(), Some("value"));
        Keychain.delete(key).unwrap();
        assert_eq!(Keychain.get(key).unwrap(), None);
    }
}
