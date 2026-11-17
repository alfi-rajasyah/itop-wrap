//! Removes the password that versions before the memory-only password saved in the login
//! Keychain (docs/memory-only-password.md). The password now lives only in memory
//! (`itop_kit::MemorySecretStore`), so nothing here reads or writes one.
//!
//! The query is the old store's, attribute for attribute (class, service, account), so it finds
//! the item that the Swift app or an earlier Rust build saved.

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use security_framework_sys::base::{errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword,
};
use security_framework_sys::keychain_item::SecItemDelete;

use itop_kit::secret_store::KeychainError;
use itop_kit::{ServerAddress, SessionService, SettingsStore};

/// One of the Security framework's `CFString` constants (`kSecClass`, …) as a `CFString`.
macro_rules! sec {
    ($k:ident) => {{
        // SAFETY: an immutable CFString constant exported by the Security framework, valid for
        // the life of the process; the get rule retains it for the wrapper.
        unsafe { CFString::wrap_under_get_rule($k) }
    }};
}

/// Deletes the generic password saved under `service` for `account`. A missing item is not an
/// error.
pub fn delete_password(service: &str, account: &str) -> Result<(), KeychainError> {
    let pairs: Vec<(CFString, CFType)> = vec![
        (sec!(kSecClass), sec!(kSecClassGenericPassword).as_CFType()),
        (sec!(kSecAttrService), CFString::new(service).as_CFType()),
        (sec!(kSecAttrAccount), CFString::new(account).as_CFType()),
    ];
    let query = CFDictionary::from_CFType_pairs(&pairs);
    // SAFETY: `query` is valid for the duration of the call.
    let status = unsafe { SecItemDelete(query.as_concrete_TypeRef()) };
    if status == errSecSuccess || status == errSecItemNotFound {
        Ok(())
    } else {
        Err(KeychainError { status })
    }
}

/// The Keychain account the old versions used for the remembered server and username, if both
/// are remembered.
pub fn legacy_account(settings: &dyn SettingsStore) -> Option<String> {
    let server = settings.string(itop_kit::SessionSettings::SERVER_KEY)?;
    let user = settings.string(itop_kit::SessionSettings::USER_KEY)?;
    let address = ServerAddress::new(&server).ok()?;
    Some(SessionService::account(&user, &address))
}

/// Deletes the old versions' saved password for the remembered account, if there is one. macOS
/// may ask to allow it once; until it's gone this runs at every launch, and once it's gone the
/// delete finds nothing and asks nothing.
pub fn remove_legacy_password(service: &str, settings: &dyn SettingsStore) {
    if let Some(account) = legacy_account(settings)
        && let Err(e) = delete_password(service, &account)
    {
        eprintln!(
            "iTop Wrap couldn't remove the old saved password from the Keychain (OSStatus {}).",
            e.status
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itop_kit::MemorySettingsStore;

    #[test]
    fn legacy_account_is_the_old_stores() {
        let settings = MemorySettingsStore::new();
        assert_eq!(legacy_account(&settings), None, "nothing remembered");
        settings.set_string("server", Some("https://itop.example.com"));
        assert_eq!(legacy_account(&settings), None, "no username");
        settings.set_string("user", Some("agent"));
        assert_eq!(
            legacy_account(&settings).as_deref(),
            Some("agent@https://itop.example.com")
        );
    }

    #[test]
    fn swift_service_name() {
        assert_eq!(crate::prefs::KEYCHAIN_SERVICE, "iTop Wrap");
    }

    /// Against the real login Keychain with a throwaway service. Ignored by default: it touches
    /// the Keychain and may prompt. Run with `cargo test -p itop-wrap -- --ignored keychain`.
    #[test]
    #[ignore = "uses the real login Keychain"]
    fn keychain_delete_of_a_missing_item_is_ok() {
        let account = format!("missing-{}@https://itop.invalid", std::process::id());
        assert_eq!(delete_password("iTop Wrap tests", &account), Ok(()));
    }
}
