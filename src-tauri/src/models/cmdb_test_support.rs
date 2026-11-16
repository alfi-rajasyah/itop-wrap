//! Shared by the tests of the CMDB, Organizations, People and New ticket models: a stub iTop, a
//! fake Mac, a recording hub and a `WorkspaceLink` that records what the models asked for.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use itop_kit::stub::{RecordedRequest, StubReply, StubServer};
use itop_kit::{
    Credentials, CurrentUser, ITopClient, ServerAddress, Session, Team, Ticket, TicketScope,
};
use serde_json::{Value, json};

use crate::hub::{Hub, RecordingSink};
use crate::models::seams::{ModelContext, WorkspaceLink};
use crate::native::fake::FakeNative;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkCall {
    OpenTicket(String),
    Created(String, bool),
    Section(String),
}

pub struct FakeLink {
    pub scope: Mutex<TicketScope>,
    pub calls: Mutex<Vec<LinkCall>>,
}

impl FakeLink {
    pub fn calls(&self) -> Vec<LinkCall> {
        self.calls.lock().unwrap().clone()
    }
    pub fn set_scope(&self, scope: TicketScope) {
        *self.scope.lock().unwrap() = scope;
    }
}

#[async_trait::async_trait]
impl WorkspaceLink for FakeLink {
    fn scope(&self) -> TicketScope {
        *self.scope.lock().unwrap()
    }
    async fn open_ticket(&self, ticket: Ticket) {
        self.calls
            .lock()
            .unwrap()
            .push(LinkCall::OpenTicket(ticket.id()));
    }
    async fn ticket_created(&self, ticket: Ticket, assign: bool) {
        self.calls
            .lock()
            .unwrap()
            .push(LinkCall::Created(ticket.r#ref.clone(), assign));
    }
    async fn select_section(&self, section: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(LinkCall::Section(section.to_owned()));
    }
}

pub struct Env {
    pub stub: StubServer,
    pub native: Arc<FakeNative>,
    pub sink: Arc<RecordingSink>,
    pub hub: Hub,
    pub link: Arc<FakeLink>,
    pub ctx: ModelContext,
}

impl Env {
    /// The requests' OQL (`json_data.key`) in order.
    pub fn oql(&self) -> Vec<String> {
        self.stub.requests().iter().map(oql).collect()
    }
    pub fn link_dyn(&self) -> Arc<dyn WorkspaceLink> {
        self.link.clone()
    }
}

pub fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1".into(),
        contact_id: "5".into(),
        login: "jane".into(),
        display_name: "Jane Agent".into(),
        profiles: vec![],
        teams: vec![Team::new("1", "Network")],
    }
}

pub fn env(handler: impl Fn(&RecordedRequest) -> StubReply + Send + Sync + 'static) -> Env {
    env_with(handler, false)
}

/// `writes`: the client may create tickets.
pub fn env_with(
    handler: impl Fn(&RecordedRequest) -> StubReply + Send + Sync + 'static,
    writes: bool,
) -> Env {
    let stub = StubServer::with_handler(handler);
    let client = ITopClient::with_writes(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("jane", "pw"),
        stub.transport(),
        writes,
        None,
        false,
    );
    let native = Arc::new(FakeNative::new(
        Utc.with_ymd_and_hms(2026, 11, 16, 9, 0, 0).unwrap(),
    ));
    let sink = Arc::new(RecordingSink::default());
    let hub = Hub::new(sink.clone());
    let link = Arc::new(FakeLink {
        scope: Mutex::new(TicketScope::OPEN),
        calls: Mutex::default(),
    });
    let ctx = ModelContext {
        session: Arc::new(Session { client, user: me() }),
        hub: hub.clone(),
        native: native.clone(),
    };
    Env {
        stub,
        native,
        sink,
        hub,
        link,
        ctx,
    }
}

pub fn oql(r: &RecordedRequest) -> String {
    r.json_data()
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// An iTop answer with these objects: `(key, fields)`.
pub fn objects(cls: &str, rows: &[(&str, Value)]) -> StubReply {
    objects_found(cls, rows, None)
}

/// As `objects`, with iTop's "Found: <n>" message.
pub fn objects_found(cls: &str, rows: &[(&str, Value)], found: Option<usize>) -> StubReply {
    let mut map = serde_json::Map::new();
    for (key, fields) in rows {
        map.insert(
            format!("{cls}::{key}"),
            json!({"class": cls, "key": key, "code": 0, "message": "", "fields": fields}),
        );
    }
    let message = found.map_or_else(String::new, |n| format!("Found: {n}"));
    StubReply::json(&json!({"code": 0, "message": message, "objects": map}).to_string())
}

pub fn empty() -> StubReply {
    StubReply::json(r#"{"code":0,"message":"","objects":{}}"#)
}

/// A ticket row's fields.
pub fn ticket_fields(
    r: &str,
    title: &str,
    status: &str,
    priority: &str,
    agent: (&str, &str),
    team: &str,
) -> Value {
    json!({
        "ref": r, "title": title, "status": status, "priority": priority,
        "caller_id_friendlyname": "Carl Caller", "org_id": "3", "org_id_friendlyname": "Acme",
        "team_id": team, "team_id_friendlyname": "Network", "agent_id": agent.0, "agent_id_friendlyname": agent.1,
        "start_date": "2026-11-10 09:00:00", "last_update": "2026-11-15 10:00:00",
    })
}

/// Lets every runnable task finish without moving the paused clock past a debounce.
pub async fn settle() {
    tokio::time::sleep(Duration::from_millis(1)).await;
}

/// Advances the paused clock by `ms`, running everything due.
pub async fn advance(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
