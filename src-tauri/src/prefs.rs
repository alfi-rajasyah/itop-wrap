//! Settings in the same defaults domain the Swift app used (PERS-1…9), through CFPreferences so it
//! works the same from the bundled app and from an unbundled `cargo run`.
//!
//! # Keys (the one list)
//!
//! Session domain ([`Prefs::session`]: the app's domain, or the debug suite, see below):
//!
//! | Key | Type | Meaning | Checklist |
//! |---|---|---|---|
//! | `server` | string | normalised base URL, set at sign-in | PERS-1 |
//! | `user` | string | trimmed username, set at sign-in | PERS-2 |
//! | `list.range` | integer | 0 any time, 7, 30, 90; unknown/missing → any time | PERS-3 |
//! | `list.includeClosed` | bool | default false | PERS-4 |
//! | `list.sort` | string | `priority`, `lastUpdate`, `newest`, `oldest`; unknown → priority | PERS-5 |
//! | `list.pageSize` | integer | 10/20/30/50/100, 0 = All; missing → 20 | PERS-6 |
//! | `timer.running` | data | UTF-8 JSON `{ticketClass, ticketKey, ref, title, orgID, start}`, `start` = seconds since 2001-01-01 UTC; removed when the timer stops. Write it with [`Prefs::set_data`] so the Swift app can read it back | PERS-7 |
//!
//! Standard domain only ([`Prefs::standard`], even with the debug switch):
//!
//! | Key | Type | Meaning | Checklist |
//! |---|---|---|---|
//! | `newTicket.assignToMe` | bool | default true | PERS-8 |
//! | `NSWindow Frame main` | string | the signed-in main window's frame, written by AppKit's frame autosave exactly as SwiftUI did (`native::mac::appkit::MAIN_FRAME_AUTOSAVE`) | PERS-12 |
//!
//! # Debug switch
//!
//! Debug builds honour `ITOPWRAP_SETTINGS=<name>` like Swift's `DebugLaunch` (DBG-7): the session
//! domain becomes the suite `ITopWrap.debug.<name>` and the Keychain service
//! `iTop Wrap debug <name>`. Release builds compile the switch out.

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::preferences::{CFPreferencesAppSynchronize, CFPreferencesCopyAppValue, CFPreferencesSetAppValue};

use itop_kit::session::SettingsStore;

/// The Swift app's bundle identifier: its standard `UserDefaults` domain.
pub const APP_DOMAIN: &str = "io.github.alfi-rajasyah.itopwrap";
/// The Keychain service for passwords (PERS-10).
pub const KEYCHAIN_SERVICE: &str = "iTop Wrap";

pub mod keys {
    pub const SERVER: &str = "server";
    pub const USER: &str = "user";
    pub const LIST_RANGE: &str = "list.range";
    pub const LIST_INCLUDE_CLOSED: &str = "list.includeClosed";
    pub const LIST_SORT: &str = "list.sort";
    pub const LIST_PAGE_SIZE: &str = "list.pageSize";
    pub const TIMER_RUNNING: &str = "timer.running";
    pub const NEW_TICKET_ASSIGN_TO_ME: &str = "newTicket.assignToMe";
    /// Written and read by AppKit (`NSWindow.setFrameAutosaveName("main")`), never by hand.
    pub const MAIN_WINDOW_FRAME: &str = "NSWindow Frame main";
}

/// The debug settings name from `ITOPWRAP_SETTINGS`, if any (debug builds only).
pub fn debug_settings_name() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var("ITOPWRAP_SETTINGS").ok().filter(|s| !s.is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// The Keychain service: "iTop Wrap", or "iTop Wrap debug <name>" with the debug switch (DBG-7).
pub fn keychain_service() -> String {
    match debug_settings_name() {
        Some(name) => format!("{KEYCHAIN_SERVICE} debug {name}"),
        None => KEYCHAIN_SERVICE.to_owned(),
    }
}

/// One defaults domain. Cheap to clone; CFPreferences is thread-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    domain: String,
}

impl Prefs {
    /// A domain by name (an app id or a suite name).
    pub fn domain(domain: impl Into<String>) -> Self {
        Self { domain: domain.into() }
    }

    /// The app's standard domain (`UserDefaults.standard` of the Swift app).
    pub fn standard() -> Self {
        Self::domain(APP_DOMAIN)
    }

    /// Where server, user, list options and the timer live: the standard domain, or
    /// `ITopWrap.debug.<name>` with `ITOPWRAP_SETTINGS` in a debug build.
    pub fn session() -> Self {
        match debug_settings_name() {
            Some(name) => Self::domain(format!("ITopWrap.debug.{name}")),
            None => Self::standard(),
        }
    }

    pub fn name(&self) -> &str {
        &self.domain
    }

    fn get(&self, key: &str) -> Option<CFType> {
        let key = CFString::new(key);
        let app = CFString::new(&self.domain);
        // SAFETY: both arguments are valid CFStrings that outlive the call; the result follows the
        // Create rule (we own it), or is null when there is no value.
        let raw = unsafe { CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), app.as_concrete_TypeRef()) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: non-null and owned by us (Create rule), so wrapping without retaining is right.
        Some(unsafe { CFType::wrap_under_create_rule(raw) })
    }

    fn put(&self, key: &str, value: Option<&CFType>) {
        let key = CFString::new(key);
        let app = CFString::new(&self.domain);
        let value = value.map_or(std::ptr::null(), |v| v.as_CFTypeRef());
        // SAFETY: valid CFStrings and a valid property list value (or null to remove the key); the
        // call retains what it keeps. Synchronize flushes to disk like UserDefaults does eventually.
        unsafe {
            CFPreferencesSetAppValue(key.as_concrete_TypeRef(), value, app.as_concrete_TypeRef());
            CFPreferencesAppSynchronize(app.as_concrete_TypeRef());
        }
    }

    /// A string value. Like `UserDefaults.string(forKey:)` it also answers for numbers; unlike it,
    /// it decodes a `Data` value as UTF-8, because the Swift app stored `timer.running` as JSON data.
    pub fn string(&self, key: &str) -> Option<String> {
        let value = self.get(key)?;
        if let Some(s) = value.downcast::<CFString>() {
            return Some(s.to_string());
        }
        if let Some(d) = value.downcast::<CFData>() {
            return String::from_utf8(d.bytes().to_vec()).ok();
        }
        if let Some(n) = value.downcast::<CFNumber>() {
            return n.to_i64().map(|i| i.to_string()).or_else(|| n.to_f64().map(|f| f.to_string()));
        }
        None
    }

    /// An integer: numbers, booleans (1/0) and numeric strings, like `UserDefaults.integer(forKey:)`
    /// but `None` when missing so callers can apply their own defaults (PERS-6: missing → 20).
    pub fn i64(&self, key: &str) -> Option<i64> {
        let value = self.get(key)?;
        if let Some(b) = value.downcast::<CFBoolean>() {
            return Some(i64::from(bool::from(b)));
        }
        if let Some(n) = value.downcast::<CFNumber>() {
            return n.to_i64().or_else(|| n.to_f64().map(|f| f as i64));
        }
        if let Some(s) = value.downcast::<CFString>() {
            return s.to_string().trim().parse().ok();
        }
        None
    }

    /// A boolean: booleans, numbers (non-zero) and the strings "YES"/"true"/"1".
    pub fn bool(&self, key: &str) -> Option<bool> {
        let value = self.get(key)?;
        if let Some(b) = value.downcast::<CFBoolean>() {
            return Some(b.into());
        }
        if let Some(n) = value.downcast::<CFNumber>() {
            return n.to_i64().map(|i| i != 0);
        }
        if let Some(s) = value.downcast::<CFString>() {
            let s = s.to_string().to_ascii_lowercase();
            return Some(s == "yes" || s == "true" || s == "1");
        }
        None
    }

    /// Raw bytes: a `Data` value, or a string's UTF-8.
    pub fn data(&self, key: &str) -> Option<Vec<u8>> {
        let value = self.get(key)?;
        if let Some(d) = value.downcast::<CFData>() {
            return Some(d.bytes().to_vec());
        }
        value.downcast::<CFString>().map(|s| s.to_string().into_bytes())
    }

    pub fn set_string(&self, key: &str, value: Option<&str>) {
        self.put(key, value.map(|v| CFString::new(v).as_CFType()).as_ref());
    }

    pub fn set_i64(&self, key: &str, value: i64) {
        self.put(key, Some(&CFNumber::from(value).as_CFType()));
    }

    pub fn set_bool(&self, key: &str, value: bool) {
        self.put(key, Some(&CFBoolean::from(value).as_CFType()));
    }

    pub fn set_data(&self, key: &str, value: Option<&[u8]>) {
        self.put(key, value.map(|v| CFData::from_buffer(v).as_CFType()).as_ref());
    }

    pub fn remove(&self, key: &str) {
        self.put(key, None);
    }
}

impl SettingsStore for Prefs {
    fn string(&self, key: &str) -> Option<String> {
        Prefs::string(self, key)
    }

    fn set_string(&self, key: &str, value: Option<&str>) {
        Prefs::set_string(self, key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway domain, removed again at the end (CFPreferences keeps a plist per domain).
    struct Scratch(Prefs);
    impl Scratch {
        fn new(tag: &str) -> Self {
            Self(Prefs::domain(format!("io.github.alfi-rajasyah.itopwrap.tests.{tag}.{}", std::process::id())))
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            for k in ["s", "i", "b", "d", "timer.running"] {
                self.0.remove(k);
            }
        }
    }

    #[test]
    fn typed_round_trips() {
        let p = Scratch::new("typed");
        let p = &p.0;
        assert_eq!(p.string("s"), None);
        p.set_string("s", Some("https://itop.example.com"));
        assert_eq!(p.string("s").as_deref(), Some("https://itop.example.com"));
        p.set_string("s", None);
        assert_eq!(p.string("s"), None);

        p.set_i64("i", 30);
        assert_eq!(p.i64("i"), Some(30));
        assert_eq!(p.string("i").as_deref(), Some("30"));

        p.set_bool("b", true);
        assert_eq!(p.bool("b"), Some(true));
        assert_eq!(p.i64("b"), Some(1));
        p.set_bool("b", false);
        assert_eq!(p.bool("b"), Some(false));

        p.set_data("d", Some(&[0, 159, 1]));
        assert_eq!(p.data("d"), Some(vec![0, 159, 1]));
        assert_eq!(p.string("d"), None, "not UTF-8");
    }

    /// PERS-7: the Swift app wrote the running timer as JSON `Data`; `string()` must still read it.
    #[test]
    fn timer_stored_as_data_reads_as_string() {
        let p = Scratch::new("timer");
        let p = &p.0;
        let json = r#"{"ticketClass":"UserRequest","ticketKey":"12","ref":"R-000012","title":"Café","orgID":"3","start":785000000}"#;
        p.set_data(keys::TIMER_RUNNING, Some(json.as_bytes()));
        assert_eq!(SettingsStore::string(p, keys::TIMER_RUNNING).as_deref(), Some(json));
        SettingsStore::set_string(p, keys::TIMER_RUNNING, None);
        assert_eq!(p.data(keys::TIMER_RUNNING), None);
    }

    #[test]
    fn keychain_service_is_the_swift_one_without_the_switch() {
        if debug_settings_name().is_none() {
            assert_eq!(keychain_service(), "iTop Wrap");
            assert_eq!(Prefs::session(), Prefs::standard());
        }
    }
}
