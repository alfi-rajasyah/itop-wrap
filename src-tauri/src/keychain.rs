//! Port of `KeychainStore.swift`: generic passwords in the login Keychain under one service name
//! (PERS-10). The query is the Swift one attribute for attribute (class, service, account; label
//! "<service> (<account>)" only when adding), so an item the Swift app saved is found and updated
//! in place, and vice versa.

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use security_framework_sys::base::{errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrLabel, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData, kSecValueData,
};
use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate};

use itop_kit::secret_store::{KeychainError, SecretStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeychainStore {
    pub service: String,
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self::new(crate::prefs::KEYCHAIN_SERVICE)
    }
}

/// One of the Security framework's `CFString` constants (`kSecClass`, …) as a `CFString`.
macro_rules! sec {
    ($k:ident) => {{
        // SAFETY: an immutable CFString constant exported by the Security framework, valid for
        // the life of the process; the get rule retains it for the wrapper.
        unsafe { CFString::wrap_under_get_rule($k) }
    }};
}

impl KeychainStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    fn query_pairs(&self, account: &str) -> Vec<(CFString, CFType)> {
        vec![
            (sec!(kSecClass), sec!(kSecClassGenericPassword).as_CFType()),
            (sec!(kSecAttrService), CFString::new(&self.service).as_CFType()),
            (sec!(kSecAttrAccount), CFString::new(account).as_CFType()),
        ]
    }

    fn dict(pairs: &[(CFString, CFType)]) -> CFDictionary<CFString, CFType> {
        CFDictionary::from_CFType_pairs(pairs)
    }
}

impl SecretStore for KeychainStore {
    fn password(&self, account: &str) -> Result<Option<String>, KeychainError> {
        let mut pairs = self.query_pairs(account);
        pairs.push((sec!(kSecReturnData), CFBoolean::true_value().as_CFType()));
        // kSecMatchLimitOne is SecItemCopyMatching's default; the Swift query set it explicitly.
        let query = Self::dict(&pairs);
        let mut result: core_foundation_sys::base::CFTypeRef = std::ptr::null();
        // SAFETY: `query` is a valid dictionary for the duration of the call and `result` a valid
        // out-pointer; on success it receives an owned (Create rule) CFData.
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
        if status == errSecItemNotFound {
            return Ok(None);
        }
        if status != errSecSuccess || result.is_null() {
            return Err(KeychainError { status });
        }
        // SAFETY: non-null and owned by us (Create rule).
        let value = unsafe { CFType::wrap_under_create_rule(result) };
        let data = value.downcast::<CFData>().ok_or(KeychainError { status })?;
        // Swift: String(decoding: data, as: UTF8.self), which replaces invalid bytes.
        Ok(Some(String::from_utf8_lossy(data.bytes()).into_owned()))
    }

    fn set_password(&self, password: &str, account: &str) -> Result<(), KeychainError> {
        let data = CFData::from_buffer(password.as_bytes());
        let query = Self::dict(&self.query_pairs(account));
        let update = Self::dict(&[(sec!(kSecValueData), data.as_CFType())]);
        // SAFETY: both dictionaries are valid for the duration of the call.
        let status = unsafe { SecItemUpdate(query.as_concrete_TypeRef(), update.as_concrete_TypeRef()) };
        if status == errSecSuccess {
            return Ok(());
        }
        if status != errSecItemNotFound {
            return Err(KeychainError { status });
        }
        let mut add = self.query_pairs(account);
        add.push((sec!(kSecValueData), data.as_CFType()));
        add.push((sec!(kSecAttrLabel), CFString::new(&format!("{} ({account})", self.service)).as_CFType()));
        let add = Self::dict(&add);
        // SAFETY: `add` is valid for the call; a null result pointer means "don't return the item".
        let status = unsafe { SecItemAdd(add.as_concrete_TypeRef(), std::ptr::null_mut()) };
        if status == errSecSuccess { Ok(()) } else { Err(KeychainError { status }) }
    }

    fn delete_password(&self, account: &str) -> Result<(), KeychainError> {
        let query = Self::dict(&self.query_pairs(account));
        // SAFETY: `query` is valid for the duration of the call.
        let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };
        if status == errSecSuccess || status == errSecItemNotFound { Ok(()) } else { Err(KeychainError { status }) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_defaults() {
        assert_eq!(KeychainStore::default().service, "iTop Wrap");
    }

    /// Port of the Swift `KeychainTests.roundTrip`, against the real login Keychain with a
    /// throwaway service and account. Ignored by default: it touches the Keychain and may prompt.
    /// Run with `cargo test -p itop-wrap -- --ignored keychain`.
    #[test]
    #[ignore = "uses the real login Keychain"]
    fn keychain_round_trip() {
        let store = KeychainStore::new("iTop Wrap tests");
        let account = format!("roundtrip-{}@https://itop.invalid", std::process::id());
        let _ = store.delete_password(&account);
        assert_eq!(store.password(&account), Ok(None));
        store.set_password("first", &account).expect("add");
        store.set_password("second", &account).expect("update in place");
        let read = store.password(&account);
        store.delete_password(&account).expect("delete");
        assert_eq!(read, Ok(Some("second".to_owned())));
        assert_eq!(store.password(&account), Ok(None));
        assert_eq!(store.delete_password(&account), Ok(()), "a missing item is not an error");
    }
}
