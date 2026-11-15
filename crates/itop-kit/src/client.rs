//! Port of `ITopClient.swift`: `ITopClient`, its write allowlist, `ITopResponse` and `ITopObject`.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::ITopError;
use crate::json::{self, JsonObject, JsonValue};
use crate::rest_operation::RestOperation;
use crate::server_address::{Credentials, ServerAddress};
use crate::ticket::TicketClass;
use crate::transport::{HttpRequest, Transport, TransportError, url_error};

/// The write allowlist, shared by every copy of a session's client, so a ticket the app
/// creates under the new-ticket switch can be worked on straight away.
#[derive(Debug)]
struct AllowlistBox {
    keys: Mutex<HashSet<String>>,
}

impl AllowlistBox {
    fn new(keys: HashSet<String>) -> Self {
        AllowlistBox {
            keys: Mutex::new(keys),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.keys.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn current(&self) -> HashSet<String> {
        self.lock().clone()
    }

    fn contains(&self, key: &str) -> bool {
        self.lock().contains(key)
    }

    fn insert(&self, key: String) {
        self.lock().insert(key);
    }
}

/// The single entry point to iTop's REST API (PLAN.md, Architecture Decisions).
///
/// Cheap to clone; clones share the transport and the write allowlist.
#[derive(Clone)]
pub struct ITopClient {
    pub server: ServerAddress,
    pub credentials: Credentials,
    pub allows_writes: bool,
    /// When set, writes are allowed only on these objects ("UserRequest::7"), and nothing can be
    /// created: the safety net for testing writes on chosen tickets of a real iTop.
    allowlist_box: Option<Arc<AllowlistBox>>,
    /// With an allowlist, also allow creating new tickets (UserRequest, Incident): for testing
    /// New ticket on a real iTop without opening writes to every existing ticket.
    pub allows_ticket_creation: bool,
    transport: Arc<dyn Transport>,
}

impl fmt::Debug for ITopClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ITopClient")
            .field("server", &self.server)
            .field("credentials", &self.credentials)
            .field("allows_writes", &self.allows_writes)
            .field("write_allowlist", &self.write_allowlist())
            .field("allows_ticket_creation", &self.allows_ticket_creation)
            .finish_non_exhaustive()
    }
}

impl ITopClient {
    /// Writes are off: during development the app only reads.
    pub fn new(
        server: ServerAddress,
        credentials: Credentials,
        transport: Arc<dyn Transport>,
    ) -> Self {
        Self::with_writes(server, credentials, transport, false, None, false)
    }

    /// The full Swift initialiser: `allowsWrites`, `writeAllowlist`, `allowsTicketCreation`.
    pub fn with_writes(
        server: ServerAddress,
        credentials: Credentials,
        transport: Arc<dyn Transport>,
        allows_writes: bool,
        write_allowlist: Option<HashSet<String>>,
        allows_ticket_creation: bool,
    ) -> Self {
        ITopClient {
            server,
            credentials,
            allows_writes,
            allowlist_box: write_allowlist.map(|keys| Arc::new(AllowlistBox::new(keys))),
            allows_ticket_creation,
            transport,
        }
    }

    /// The transport this client sends through.
    pub fn transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }

    /// The current allowlist, `None` when writes aren't limited to one.
    pub fn write_allowlist(&self) -> Option<HashSet<String>> {
        self.allowlist_box.as_ref().map(|b| b.current())
    }

    /// The same client with writes on, optionally limited to an allowlist.
    pub fn allowing_writes(&self, only: Option<HashSet<String>>, new_tickets: bool) -> ITopClient {
        Self::with_writes(
            self.server.clone(),
            self.credentials.clone(),
            self.transport.clone(),
            true,
            only,
            new_tickets,
        )
    }

    /// After creating a ticket under the new-ticket switch: allow writes on it too.
    pub fn allow_created_ticket(&self, key: &str) {
        if !self.allows_ticket_creation {
            return;
        }
        if let Some(b) = &self.allowlist_box {
            b.insert(key.to_string());
        }
    }

    /// Whether New ticket would be sent.
    pub fn can_create_tickets(&self) -> bool {
        self.allows_writes && (self.allowlist_box.is_none() || self.allows_ticket_creation)
    }

    /// Whether a write to this object would be sent (for enabling the composer and actions).
    pub fn can_write(&self, class_name: &str, key: &str) -> bool {
        if !self.allows_writes {
            return false;
        }
        match &self.allowlist_box {
            None => true,
            Some(b) => b.contains(&format!("{class_name}::{key}")),
        }
    }

    /// `ticket` is the "Class::key" a create belongs to (a TimeSpent or Activity on that ticket),
    /// so an allowlist can let it through; other creates need writes on without an allowlist.
    fn permits(&self, operation: &RestOperation, ticket: Option<&str>) -> bool {
        match operation {
            RestOperation::Update {
                class_name, key, ..
            }
            | RestOperation::ApplyStimulus {
                class_name, key, ..
            } => self.can_write(class_name, key),
            RestOperation::Create { class_name, .. } => {
                if !self.allows_writes {
                    return false;
                }
                let Some(b) = &self.allowlist_box else {
                    return true;
                };
                if let Some(ticket) = ticket {
                    return b.contains(ticket);
                }
                self.allows_ticket_creation && TicketClass::from_raw(class_name).is_some()
            }
            _ => true,
        }
    }

    /// Sends one operation and returns iTop's answer, or an `ITopError`.
    pub async fn send(&self, operation: RestOperation) -> Result<ITopResponse, ITopError> {
        self.send_on_ticket(operation, None).await
    }

    /// Sends a create that belongs to one ticket ("Class::key"), checked against the allowlist.
    pub async fn send_on_ticket(
        &self,
        operation: RestOperation,
        ticket: Option<&str>,
    ) -> Result<ITopResponse, ITopError> {
        if operation.is_write() && !self.permits(&operation, ticket) {
            return Err(ITopError::WritesDisabled);
        }

        let request = HttpRequest {
            method: "POST".into(),
            url: self.server.rest_endpoint(),
            headers: vec![(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            body: Self::form_body(&[
                ("auth_user", &self.credentials.user),
                ("auth_pwd", &self.credentials.password),
                ("json_data", &Self::encode(&operation.json())),
            ]),
        };

        let response = self
            .transport
            .send(request)
            .await
            .map_err(|e| Self::map(&e))?;

        if !(200..300).contains(&response.status) {
            return Err(ITopError::Http {
                status: response.status,
            });
        }
        let body = match JsonValue::parse(&response.body) {
            Ok(body @ JsonValue::Object(_)) => body,
            _ => return Err(ITopError::UnexpectedResponse),
        };
        let Some(code) = body.get("code").and_then(JsonValue::int_value) else {
            return Err(ITopError::UnexpectedResponse);
        };
        let message = body
            .get("message")
            .and_then(JsonValue::string_value)
            .unwrap_or("")
            .to_string();
        let response = ITopResponse { body };
        if cfg!(debug_assertions) && Self::tracing() {
            eprintln!("{}", Self::trace_line(&operation, &response));
        }
        match code {
            0 => Ok(response),
            1 => {
                if Self::is_rest_profile_message(&message) {
                    Err(ITopError::RestNotAllowed)
                } else if Self::is_permission_message(&message) {
                    Err(ITopError::NotAllowed { message })
                } else {
                    Err(ITopError::InvalidCredentials)
                }
            }
            _ => Err(ITopError::Server { code, message }),
        }
    }

    /// `true` when iTop accepts these credentials for REST.
    pub async fn check_credentials(&self) -> Result<bool, ITopError> {
        let response = self
            .send(RestOperation::CheckCredentials {
                user: self.credentials.user.clone(),
                password: self.credentials.password.clone(),
            })
            .await?;
        Ok(response.body.get("authorized") == Some(&JsonValue::Bool(true)))
    }

    // MARK: - Encoding

    /// Compact JSON, keys sorted, slashes not escaped.
    pub fn encode(value: &JsonValue) -> String {
        value.to_json_string()
    }

    /// application/x-www-form-urlencoded, escaping everything except RFC 3986 unreserved characters.
    ///
    /// Foundation's `.urlQueryAllowed` keeps `&`, `=` and `+`, which would split a password like
    /// "p&ss=w+rd" (TransportTests covers this).
    pub fn form_body(pairs: &[(&str, &str)]) -> Vec<u8> {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", Self::escape(k), Self::escape(v)))
            .collect::<Vec<_>>()
            .join("&")
            .into_bytes()
    }

    fn escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    /// Debug builds only: `ITOPWRAP_TRACE=1` prints one line per call to the terminal.
    fn tracing() -> bool {
        static TRACING: OnceLock<bool> = OnceLock::new();
        *TRACING.get_or_init(|| std::env::var("ITOPWRAP_TRACE").as_deref() == Ok("1"))
    }

    /// Operation, class, query, iTop's code and message, and how many objects came back.
    /// Never credentials, never field values.
    pub fn trace_line(operation: &RestOperation, response: &ITopResponse) -> String {
        let mut parts = vec![format!("[iTop] {}", operation.name())];
        match operation {
            RestOperation::Get {
                class_name,
                key,
                limit,
                ..
            } => {
                parts[0] += &format!(" {class_name}");
                parts.push(format!(
                    "{key}{}",
                    limit.map(|l| format!(" (limit {l})")).unwrap_or_default()
                ));
            }
            RestOperation::GetRelated {
                class_name,
                key,
                relation,
                direction,
                ..
            } => {
                parts[0] += &format!(" {class_name}");
                parts.push(format!("{key} {relation} {direction}"));
            }
            _ => {}
        }
        parts.push(format!(
            "code {}",
            response
                .body
                .get("code")
                .and_then(JsonValue::int_value)
                .unwrap_or(-1)
        ));
        parts.push(format!("{} objects", response.objects().len()));
        if let Some(message) = response
            .body
            .get("message")
            .and_then(JsonValue::string_value)
            && !message.is_empty()
        {
            parts.push(message.to_string());
        }
        parts.join(" | ")
    }

    /// iTop answers code 1 both for a wrong password and for an account without REST rights;
    /// only the message tells them apart. The exact wording should be confirmed on the real server.
    /// Code 1 that's about using REST at all, not about one request.
    pub fn is_rest_profile_message(message: &str) -> bool {
        let m = message.to_lowercase();
        m.contains("rest services") || m.contains("rest service") || m.contains("web services")
    }

    pub fn is_permission_message(message: &str) -> bool {
        let m = message.to_lowercase();
        m.contains("permission") || m.contains("rest services") || m.contains("not allowed")
    }

    /// A transport failure as an `ITopError`.
    pub fn map(error: &TransportError) -> ITopError {
        match error.code {
            url_error::SERVER_CERTIFICATE_UNTRUSTED
            | url_error::SERVER_CERTIFICATE_HAS_BAD_DATE
            | url_error::SERVER_CERTIFICATE_HAS_UNKNOWN_ROOT
            | url_error::SERVER_CERTIFICATE_NOT_YET_VALID => ITopError::UntrustedCertificate,
            // A redirect to another server, refused by the transport (transport.rs).
            url_error::BAD_SERVER_RESPONSE => ITopError::UnexpectedResponse,
            code => ITopError::Transport(code),
        }
    }
}

/// iTop's answer to one operation.
#[derive(Clone, Debug, PartialEq)]
pub struct ITopResponse {
    /// The whole decoded JSON body, for operations with their own fields (`authorized`, `operations`).
    pub body: JsonValue,
}

impl ITopResponse {
    pub fn new(body: JsonValue) -> Self {
        ITopResponse { body }
    }

    /// iTop's own total from a message like "Found: 256", which can be more than the objects
    /// returned when a limit was asked for.
    pub fn found(&self) -> Option<i64> {
        let message = self.body.get("message")?.string_value()?;
        let digits = message.strip_prefix("Found: ")?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }

    /// The returned objects. iTop sends them as a JSON object keyed "Class::id", which has no
    /// order, so they are sorted by id here; callers sort by what they show.
    pub fn objects(&self) -> Vec<ITopObject> {
        let Some(JsonValue::Object(map)) = self.body.get("objects") else {
            return Vec::new();
        };
        let mut objects: Vec<ITopObject> = map.values().filter_map(ITopObject::from_json).collect();
        objects.sort_by(|a, b| {
            let ka = (json::swift_int(&a.key).unwrap_or(0), &a.key);
            let kb = (json::swift_int(&b.key).unwrap_or(0), &b.key);
            ka.cmp(&kb)
        });
        objects
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ITopObject {
    pub class_name: String,
    pub key: String,
    /// The per-object result; writes can fail per object while the call itself succeeds.
    pub code: i64,
    pub message: String,
    pub fields: JsonObject,
}

impl ITopObject {
    /// `ITopObject(json:)`: `None` without a string `class`.
    pub fn from_json(json: &JsonValue) -> Option<ITopObject> {
        let class_name = json.get("class")?.string_value()?.to_string();
        let key_value = json.get("key");
        let key = key_value
            .and_then(JsonValue::string_value)
            .map(str::to_string)
            .or_else(|| {
                key_value
                    .and_then(JsonValue::int_value)
                    .map(|n| n.to_string())
            })
            .unwrap_or_default();
        Some(ITopObject {
            class_name,
            key,
            code: json.get("code").and_then(JsonValue::int_value).unwrap_or(0),
            message: json
                .get("message")
                .and_then(JsonValue::string_value)
                .unwrap_or("")
                .to_string(),
            fields: match json.get("fields") {
                Some(JsonValue::Object(f)) => f.clone(),
                _ => JsonObject::new(),
            },
        })
    }
}
