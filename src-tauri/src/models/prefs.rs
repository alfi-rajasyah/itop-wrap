//! Typed access to the app's settings (UserDefaults in the Swift app, checklist section 16).
//!
//! `itop_kit::SettingsStore` only reads and writes strings (`server`, `user`, `timer.running`).
//! The View options are an int and a bool in UserDefaults (`list.range`, `list.includeClosed`,
//! `list.pageSize`), so the store the native layer provides also implements [`Prefs`].

use std::collections::HashMap;
use std::sync::Mutex;

use itop_kit::SettingsStore;

/// The settings store: strings (from [`SettingsStore`]) plus integers and booleans.
pub trait Prefs: SettingsStore {
    /// `None` when the key is missing (or isn't an integer).
    fn int(&self, key: &str) -> Option<i64>;
    fn set_int(&self, key: &str, value: i64);
    /// `None` when the key is missing (or isn't a boolean).
    fn bool(&self, key: &str) -> Option<bool>;
    fn set_bool(&self, key: &str, value: bool);
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Text(String),
    Int(i64),
    Bool(bool),
}

/// Settings in memory, for tests.
#[derive(Debug, Default)]
pub struct MemoryPrefs {
    values: Mutex<HashMap<String, Value>>,
}

impl MemoryPrefs {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Value>> {
        self.values.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SettingsStore for MemoryPrefs {
    fn string(&self, key: &str) -> Option<String> {
        match self.lock().get(key) {
            Some(Value::Text(s)) => Some(s.clone()),
            _ => None,
        }
    }

    fn set_string(&self, key: &str, value: Option<&str>) {
        match value {
            Some(v) => self.lock().insert(key.to_owned(), Value::Text(v.to_owned())),
            None => self.lock().remove(key),
        };
    }
}

impl Prefs for MemoryPrefs {
    fn int(&self, key: &str) -> Option<i64> {
        match self.lock().get(key) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        }
    }

    fn set_int(&self, key: &str, value: i64) {
        self.lock().insert(key.to_owned(), Value::Int(value));
    }

    fn bool(&self, key: &str) -> Option<bool> {
        match self.lock().get(key) {
            Some(Value::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    fn set_bool(&self, key: &str, value: bool) {
        self.lock().insert(key.to_owned(), Value::Bool(value));
    }
}
