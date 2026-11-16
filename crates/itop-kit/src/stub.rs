//! Port of the test helper `StubURLProtocol.swift`: a [`Transport`] that serves canned
//! responses and records what it was sent. Public so the app crate's tests can use it too.
//!
//! Each `StubServer` is its own value (no global registry is needed), so tests can run in
//! parallel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::transport::{HttpRequest, HttpResponse, Transport, TransportError};

/// One request the stub received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedRequest {
    pub request: HttpRequest,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    /// The form fields of an application/x-www-form-urlencoded body.
    ///
    /// Decoded like `URLComponents.queryItems`: `%XX` escapes only, `+` stays a plus.
    pub fn form(&self) -> HashMap<String, String> {
        let text = String::from_utf8_lossy(&self.body);
        let mut out = HashMap::new();
        if text.is_empty() {
            return out;
        }
        for item in text.split('&') {
            let (name, value) = item.split_once('=').unwrap_or((item, ""));
            out.insert(percent_decode(name), percent_decode(value));
        }
        out
    }

    /// `json_data` parsed back into a JSON object (an empty object when missing or invalid).
    pub fn json_data(&self) -> serde_json::Value {
        let form = self.form();
        let text = form.get("json_data").map(String::as_str).unwrap_or("");
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(v @ serde_json::Value::Object(_)) => v,
            _ => serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// `json_data["operation"]` as a string.
    pub fn operation(&self) -> Option<String> {
        self.json_data()
            .get("operation")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    /// `json_data["class"]` as a string.
    pub fn class(&self) -> Option<String> {
        self.json_data()
            .get("class")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What the stub answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StubReply {
    Response {
        status: u16,
        content_type: String,
        body: Vec<u8>,
    },
    Failure(TransportError),
}

impl StubReply {
    /// A fixture from `tests/fixtures/<name>.json`, with status 200 (an empty body if missing).
    pub fn fixture(name: &str) -> StubReply {
        StubReply::Response {
            status: 200,
            content_type: "application/json".into(),
            body: Fixture::data(name).unwrap_or_default(),
        }
    }

    /// JSON text with status 200.
    pub fn json(text: &str) -> StubReply {
        Self::json_status(text, 200)
    }

    pub fn json_status(text: &str, status: u16) -> StubReply {
        StubReply::Response {
            status,
            content_type: "application/json".into(),
            body: text.as_bytes().to_vec(),
        }
    }

    /// A transport failure with this `URLError` code.
    pub fn failure(code: i64) -> StubReply {
        StubReply::Failure(TransportError::new(code))
    }
}

type Handler = dyn Fn(&RecordedRequest) -> StubReply + Send + Sync;

struct Inner {
    reply: Box<Handler>,
    requests: Mutex<Vec<RecordedRequest>>,
}

/// A fake server. Clones share the handler and the recorded requests.
#[derive(Clone)]
pub struct StubServer {
    inner: Arc<Inner>,
}

impl StubServer {
    /// Always answers with the same reply.
    pub fn new(reply: StubReply) -> Self {
        Self::with_handler(move |_| reply.clone())
    }

    /// Answers each request with what `handler` returns, for example by iTop class.
    pub fn with_handler(
        handler: impl Fn(&RecordedRequest) -> StubReply + Send + Sync + 'static,
    ) -> Self {
        StubServer {
            inner: Arc::new(Inner {
                reply: Box::new(handler),
                requests: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.inner
            .requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// A transport that routes every request to this stub (Swift's `stub.session`).
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.clone())
    }
}

#[async_trait]
impl Transport for StubServer {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let recorded = RecordedRequest {
            body: request.body.clone(),
            request,
        };
        self.inner
            .requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(recorded.clone());
        match (self.inner.reply)(&recorded) {
            StubReply::Failure(error) => Err(error),
            StubReply::Response {
                status,
                content_type,
                body,
            } => Ok(HttpResponse {
                status,
                headers: vec![("Content-Type".into(), content_type)],
                body,
            }),
        }
    }
}

/// The recorded iTop responses in `crates/itop-kit/tests/fixtures`.
pub struct Fixture;

impl Fixture {
    pub fn path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(format!("{name}.json"))
    }

    pub fn data(name: &str) -> std::io::Result<Vec<u8>> {
        std::fs::read(Self::path(name))
    }
}
