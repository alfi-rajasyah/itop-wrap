//! Port of `KeychainStore.swift`: the `SecretStore` protocol and an in-memory store.
//!
//! The app keeps the password in memory only. The app crate's `keychain` module only deletes the
//! item older versions saved (generic passwords under the service "iTop Wrap").

use std::collections::HashMap;
use std::sync::Mutex;

/// A Keychain failure, with its `OSStatus`.
///
/// Swift's `KeychainError` isn't a `LocalizedError`, so the app showed Foundation's generic
/// description of a bridged Swift struct error (the status isn't in it); this `Display` is that
/// exact text, as `error.localizedDescription` printed it (AUTH-4, ERR-12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("The operation couldn’t be completed. (ITopKit.KeychainError error 1.)")]
pub struct KeychainError {
    pub status: i32,
}

/// Where the password lives. The app and the tests both use the in-memory store: the password
/// is never saved to disk (docs/memory-only-password.md).
pub trait SecretStore: Send + Sync {
    fn password(&self, account: &str) -> Result<Option<String>, KeychainError>;
    fn set_password(&self, password: &str, account: &str) -> Result<(), KeychainError>;
    fn delete_password(&self, account: &str) -> Result<(), KeychainError>;
}

/// Passwords kept in memory, for tests and previews.
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    items: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many passwords are stored.
    pub fn count(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SecretStore for MemorySecretStore {
    fn password(&self, account: &str) -> Result<Option<String>, KeychainError> {
        Ok(self.lock().get(account).cloned())
    }

    fn set_password(&self, password: &str, account: &str) -> Result<(), KeychainError> {
        self.lock()
            .insert(account.to_string(), password.to_string());
        Ok(())
    }

    fn delete_password(&self, account: &str) -> Result<(), KeychainError> {
        self.lock().remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Swift `KeychainTests.roundTrip` runs against the real Keychain, which lives in the app
    /// crate now; this is the same sequence against the in-memory store.
    #[test]
    fn round_trip() {
        let store = MemorySecretStore::new();
        let account = "agent@itop.example.com";
        store.set_password("first", account).unwrap();
        store.set_password("second", account).unwrap();
        assert_eq!(store.password(account).unwrap().as_deref(), Some("second"));
        store.delete_password(account).unwrap();
        assert_eq!(store.password(account).unwrap(), None);
    }
}
