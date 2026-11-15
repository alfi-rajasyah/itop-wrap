//! Test helpers: a fake iTop behind `itop_kit::StubServer`, and a workspace wired to it with an
//! in-memory settings store, a recording hub and `FakeNative`. No network, no Tauri.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, TimeZone, Utc};
use itop_kit::stub::{RecordedRequest, StubReply, StubServer};
use itop_kit::{
    Credentials, CurrentUser, ITopClient, ServerAddress, Session, Team, Ticket, TicketClass,
};
use serde_json::{Value, json};

use super::format::Fmt;
use super::prefs::MemoryPrefs;
use super::seams::ModelContext;
use super::workspace::Workspace;
use crate::hub::{Hub, RecordingSink};
use crate::native::fake::FakeNative;

pub fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 16, 15, 0, 0)
        .single()
        .unwrap_or_default()
}

pub fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1".into(),
        contact_id: "7".into(),
        login: "agent".into(),
        display_name: "Agent Seven".into(),
        profiles: vec![],
        teams: vec![Team::new("3", "Service desk")],
    }
}

/// A ticket with the fields the tests care about.
pub fn ticket(key: &str, status: &str, agent: &str) -> Ticket {
    Ticket {
        key: key.into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: format!("R-{:0>6}", key),
        title: format!("Ticket {key}"),
        status: status.into(),
        priority: 4,
        caller_name: "Caller".into(),
        org_name: "Org A".into(),
        org_id: "4".into(),
        team_id: if agent == "0" { "0".into() } else { "3".into() },
        team_name: if agent == "0" {
            String::new()
        } else {
            "Service desk".into()
        },
        agent_id: agent.into(),
        agent_name: if agent == "0" {
            String::new()
        } else if agent == "7" {
            "Agent Seven".into()
        } else {
            format!("Agent {agent}")
        },
        start_date: None,
        last_update: None,
    }
}

fn ticket_object(t: &Ticket) -> (String, Value) {
    let fields = json!({
        "ref": t.r#ref, "title": t.title, "status": t.status, "priority": t.priority,
        "caller_id_friendlyname": t.caller_name, "org_id": t.org_id, "org_id_friendlyname": t.org_name,
        "team_id": t.team_id, "team_id_friendlyname": t.team_name, "agent_id": t.agent_id,
        "agent_id_friendlyname": t.agent_name,
        "start_date": t.start_date.map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default(),
        "last_update": t.last_update.map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default(),
    });
    let cls = t.ticket_class.raw_value();
    (
        format!("{cls}::{}", t.key),
        json!({"code": 0, "message": "", "class": cls, "key": t.key, "fields": fields}),
    )
}

pub fn objects(list: Vec<(String, Value)>) -> Value {
    let n = list.len();
    let map: serde_json::Map<String, Value> = list.into_iter().collect();
    json!({"code": 0, "message": format!("Found: {n}"), "objects": if n == 0 { Value::Null } else { Value::Object(map) }})
}

/// One canned object of a class.
pub fn object(class: &str, key: &str, fields: Value) -> (String, Value) {
    (
        format!("{class}::{key}"),
        json!({"code": 0, "message": "", "class": class, "key": key, "fields": fields}),
    )
}

pub type ListedFn = Box<dyn Fn(&Ticket, bool) -> bool + Send + Sync>;

/// iTop as the models see it.
pub struct FakeITop {
    pub tickets: Mutex<Vec<Ticket>>,
    /// Which tickets a list load returns: (ticket, mine?) → listed. Scope filtering (open only)
    /// is applied on top.
    pub listed: Mutex<ListedFn>,
    /// A stimulus answer: (code, message) for the object, refusing the change.
    pub refuse_stimulus: Mutex<Option<(i64, String)>>,
    /// List loads fail with a transport error.
    pub offline: Mutex<bool>,
    /// Canned `core/get` answers by class (Team, Person, Attachment, Activity, TimeSpent, Service…).
    pub canned: Mutex<HashMap<String, Value>>,
    /// The detail's fields (description, logs, service…).
    pub detail: Mutex<Value>,
    /// `core/create` and `core/update` fail with this iTop error.
    pub refuse_writes: Mutex<Option<(i64, String)>>,
    /// Passwords that are accepted by check_credentials.
    pub password: Mutex<String>,
}

impl Default for FakeITop {
    fn default() -> Self {
        let mine_listed: ListedFn = Box::new(|t: &Ticket, mine: bool| {
            if mine {
                t.agent_id == "7"
                    || (t.status == "escalated_tto" && t.agent_id == "0" && t.team_id == "3")
            } else {
                t.team_id == "3"
            }
        });
        FakeITop {
            tickets: Mutex::default(),
            listed: Mutex::new(mine_listed),
            refuse_stimulus: Mutex::default(),
            offline: Mutex::default(),
            canned: Mutex::default(),
            detail: Mutex::new(json!({
                "description": "<p>Printer jams</p>",
                "public_log": {"entries": []},
                "private_log": {"entries": []},
                "functionalcis_list": [],
                "service_id": "0", "service_id_friendlyname": "",
                "servicesubcategory_id": "0", "servicesubcategory_id_friendlyname": "",
            })),
            refuse_writes: Mutex::default(),
            password: Mutex::new("secret".into()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl FakeITop {
    pub fn with(tickets: Vec<Ticket>) -> Self {
        let f = FakeITop::default();
        *lock(&f.tickets) = tickets;
        f
    }

    pub fn set(&self, key: &str, change: impl FnOnce(&mut Ticket)) {
        if let Some(t) = lock(&self.tickets).iter_mut().find(|t| t.key == key) {
            change(t);
        }
    }

    pub fn get(&self, key: &str) -> Option<Ticket> {
        lock(&self.tickets).iter().find(|t| t.key == key).cloned()
    }

    pub fn can(&self, class: &str, answer: Value) {
        lock(&self.canned).insert(class.to_owned(), answer);
    }

    fn answer(&self, r: &RecordedRequest) -> StubReply {
        let data = r.json_data();
        let op = data["operation"].as_str().unwrap_or("");
        let class = data["class"].as_str().unwrap_or("");
        let key = data["key"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| data["key"].to_string());
        let reply = |v: Value| StubReply::json(&v.to_string());
        match op {
            "core/check_credentials" => {
                let ok = data["password"].as_str() == Some(lock(&self.password).as_str());
                reply(json!({"code": 0, "message": "", "authorized": ok}))
            }
            "core/get" if class == "User" => reply(objects(vec![object(
                "User",
                "1",
                json!({"login": "agent", "contactid": 7, "first_name": "Agent", "last_name": "Seven", "profile_list": []}),
            )])),
            "core/get" if class == "Person" && key.starts_with("SELECT Person WHERE id =") => {
                reply(objects(vec![object(
                    "Person",
                    "7",
                    json!({"name": "Seven", "first_name": "Agent", "team_list": [{"team_id": 3, "team_name": "Service desk"}]}),
                )]))
            }
            "core/get" if class == "UserRequest" || class == "Incident" => {
                let tickets = lock(&self.tickets).clone();
                let of_class: Vec<&Ticket> = tickets
                    .iter()
                    .filter(|t| t.ticket_class.raw_value() == class)
                    .collect();
                if let Some(rest) = key.split("ref = '").nth(1) {
                    let r = rest.trim_end_matches('\'');
                    return reply(objects(
                        of_class
                            .iter()
                            .filter(|t| t.r#ref == r)
                            .map(|t| ticket_object(t))
                            .collect(),
                    ));
                }
                if let Some(id) = key.split("WHERE id = ").nth(1) {
                    return reply(objects(
                        of_class
                            .iter()
                            .filter(|t| t.key == id.trim())
                            .map(|t| ticket_object(t))
                            .collect(),
                    ));
                }
                if !key.starts_with("SELECT") {
                    // The detail, by key.
                    let detail = lock(&self.detail).clone();
                    return reply(objects(
                        of_class
                            .iter()
                            .filter(|t| t.key == key)
                            .map(|t| object(class, &t.key, detail.clone()))
                            .collect(),
                    ));
                }
                if *lock(&self.offline) {
                    return StubReply::failure(-1009);
                }
                let mine = key.contains("agent_id =");
                let open_only = key.contains("NOT IN ('resolved','closed')");
                let listed = lock(&self.listed);
                reply(objects(
                    of_class
                        .iter()
                        .filter(|t| listed(t, mine))
                        .filter(|t| !open_only || (t.status != "resolved" && t.status != "closed"))
                        .map(|t| ticket_object(t))
                        .collect(),
                ))
            }
            "core/get" => reply(
                lock(&self.canned)
                    .get(class)
                    .cloned()
                    .unwrap_or_else(|| objects(vec![])),
            ),
            "core/apply_stimulus" => {
                if let Some((code, message)) = lock(&self.refuse_stimulus).clone() {
                    return reply(json!({"code": 0, "message": "", "objects": {
                        format!("{class}::{key}"): {"code": code, "message": message, "class": class, "key": key, "fields": {"id": key}}
                    }}));
                }
                let stimulus = data["stimulus"].as_str().unwrap_or("");
                let fields = data["fields"].clone();
                self.set(&key, |t| match stimulus {
                    "ev_assign" | "ev_reassign" => {
                        t.status = "assigned".into();
                        t.agent_id = fields["agent_id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| fields["agent_id"].to_string());
                        t.team_id = fields["team_id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| fields["team_id"].to_string());
                        t.agent_name = if t.agent_id == "7" {
                            "Agent Seven".into()
                        } else {
                            format!("Agent {}", t.agent_id)
                        };
                    }
                    "ev_pending" => t.status = "pending".into(),
                    "ev_resolve" => t.status = "resolved".into(),
                    "ev_reopen" => t.status = "assigned".into(),
                    "ev_close" => t.status = "closed".into(),
                    _ => {}
                });
                reply(json!({"code": 0, "message": "", "objects": {
                    format!("{class}::{key}"): {"code": 0, "message": "", "class": class, "key": key, "fields": {"id": key}}
                }}))
            }
            "core/update" | "core/create" => {
                if let Some((code, message)) = lock(&self.refuse_writes).clone() {
                    return reply(json!({"code": code, "message": message}));
                }
                reply(json!({"code": 0, "message": "", "objects": {
                    format!("{class}::99"): {"code": 0, "message": "", "class": class, "key": "99", "fields": {"id": 99, "secret": "s"}}
                }}))
            }
            _ => reply(json!({"code": 0, "message": ""})),
        }
    }

    /// A stub server answering as this iTop.
    pub fn serve(self: &Arc<Self>) -> StubServer {
        let me = self.clone();
        StubServer::with_handler(move |r| me.answer(r))
    }
}

pub fn client(stub: &StubServer, writes: bool) -> ITopClient {
    let server = ServerAddress::new("https://itop.example.com").expect("a valid test address");
    ITopClient::with_writes(
        server,
        Credentials::new("agent", "secret"),
        stub.transport(),
        writes,
        None,
        writes,
    )
}

pub struct Harness {
    pub itop: Arc<FakeITop>,
    pub stub: StubServer,
    pub ws: Workspace,
    pub native: Arc<FakeNative>,
    pub sink: Arc<RecordingSink>,
    pub hub: Hub,
    pub prefs: Arc<MemoryPrefs>,
}

pub struct Setup {
    pub writes: bool,
    pub prefs: MemoryPrefs,
    pub user: CurrentUser,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            writes: true,
            prefs: MemoryPrefs::new(),
            user: me(),
        }
    }
}

impl Harness {
    /// A workspace whose loops aren't started, like the Swift tests' bare models.
    pub fn new(itop: FakeITop) -> Harness {
        Self::with(itop, Setup::default())
    }

    pub fn with(itop: FakeITop, setup: Setup) -> Harness {
        let itop = Arc::new(itop);
        let stub = itop.serve();
        let native = Arc::new(FakeNative::new(now()));
        let sink = Arc::new(RecordingSink::default());
        let hub = Hub::new(sink.clone());
        let prefs = Arc::new(setup.prefs);
        let session = Arc::new(Session {
            client: client(&stub, setup.writes),
            user: setup.user,
        });
        let ctx = ModelContext {
            session,
            hub: hub.clone(),
            native: native.clone(),
        };
        let ws = Workspace::new(ctx, Fmt::utc(), prefs.clone(), false);
        Harness {
            itop,
            stub,
            ws,
            native,
            sink,
            hub,
            prefs,
        }
    }

    /// Lets spawned tasks run (the stub answers at once).
    pub async fn settle(&self) {
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
    }

    /// The latest published snapshot of a model.
    pub fn snap(&self, id: &str) -> Value {
        self.ws.publish();
        self.hub.get(id)
    }

    /// How many REST calls with this operation (and class, when given) were made.
    pub fn calls(&self, op: &str, class: Option<&str>) -> usize {
        self.stub
            .requests()
            .iter()
            .filter(|r| {
                r.operation().as_deref() == Some(op)
                    && class.is_none_or(|c| r.class().as_deref() == Some(c))
            })
            .count()
    }

    /// Loads My tickets (and Team tickets) like the views would.
    pub async fn load(&self) {
        self.ws.list_refresh(super::records::ListScope::Mine).await;
        self.settle().await;
    }
}
