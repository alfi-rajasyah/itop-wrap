//! The one place snapshots leave Rust (contract.md §2). A model calls `publish` whenever anything a
//! view reads has changed; the hub keeps the latest snapshot per model id for `get_snapshot` and
//! hands it to the sink, which in the app emits the `snapshot` event to every window.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;

/// Where published snapshots go. The app's sink emits a Tauri event; tests record them.
pub trait SnapshotSink: Send + Sync {
    fn emit(&self, model: &str, snapshot: &Value);
}

/// A sink that drops everything, for tests that only read `Hub::get`.
pub struct NoSink;
impl SnapshotSink for NoSink {
    fn emit(&self, _: &str, _: &Value) {}
}

/// Records every emission in order, for tests that check intermediate states (spinners).
#[derive(Default)]
pub struct RecordingSink(Mutex<Vec<(String, Value)>>);
impl RecordingSink {
    pub fn take(&self) -> Vec<(String, Value)> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}
impl SnapshotSink for RecordingSink {
    fn emit(&self, model: &str, snapshot: &Value) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push((model.to_owned(), snapshot.clone()));
    }
}

#[derive(Clone)]
pub struct Hub {
    sink: Arc<dyn SnapshotSink>,
    latest: Arc<Mutex<HashMap<String, Value>>>,
}

impl Hub {
    pub fn new(sink: Arc<dyn SnapshotSink>) -> Self {
        Self { sink, latest: Arc::default() }
    }

    /// Stores and emits `snapshot` for `model`. An unchanged snapshot is not emitted again, so
    /// models can publish after every step without flooding the windows.
    pub fn publish<T: Serialize>(&self, model: &str, snapshot: &T) {
        let value = match serde_json::to_value(snapshot) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hub: {model} snapshot didn't serialize: {e}");
                return;
            }
        };
        {
            let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
            if latest.get(model) == Some(&value) {
                return;
            }
            latest.insert(model.to_owned(), value.clone());
        }
        self.sink.emit(model, &value);
    }

    /// The model stopped existing (sign-out): its snapshot becomes `null`.
    pub fn clear(&self, model: &str) {
        let had = self.latest.lock().unwrap_or_else(|e| e.into_inner()).remove(model).is_some();
        if had {
            self.sink.emit(model, &Value::Null);
        }
    }

    /// Every model id that currently has a snapshot.
    pub fn models(&self) -> Vec<String> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
    }

    /// The latest snapshot, or `null` when the model doesn't exist.
    pub fn get(&self, model: &str) -> Value {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).get(model).cloned().unwrap_or(Value::Null)
    }
}
