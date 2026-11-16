//! The whole app against a real iTop, off by default: the real `AppRouter` with the real network
//! client, driven only through `dispatch` and the published snapshots, as the UI drives it. It
//! WRITES (creates two tickets and walks them to closed), so point it only at a test server:
//!
//!   ITOP_URL=… ITOP_USER=… ITOP_PASSWORD=… cargo test -p itop-wrap --test live_app -- --ignored --nocapture
//!
//! The test user must be in at least one team. It prints what it did, never the password.

use std::sync::Arc;
use std::time::{Duration, Instant};

use itop_kit::{
    Credentials, ITopClient, JsonObject, JsonValue, MemorySecretStore, ReqwestTransport,
    RestOperation, ServerAddress, SessionService, SessionSettings, Transport, WriteAccess,
};
use itop_wrap_lib::hub::{Hub, RecordingSink};
use itop_wrap_lib::models::format::Fmt;
use itop_wrap_lib::models::prefs::MemoryPrefs;
use itop_wrap_lib::models::{AppConfig, AppRouter};
use itop_wrap_lib::native::fake::FakeNative;
use itop_wrap_lib::router::{NativeEvent, Router};
use serde_json::{Value, json};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

struct App {
    router: AppRouter,
    hub: Hub,
    native: Arc<FakeNative>,
}

impl App {
    async fn send(&self, model: &str, intent: Value) -> Value {
        let kind = intent["type"].as_str().unwrap_or("").to_owned();
        match self.router.dispatch(model, intent).await {
            Ok(v) => v,
            Err(e) => panic!("{model} {kind}: {e:?}"),
        }
    }

    fn get(&self, model: &str) -> Value {
        self.hub.get(model)
    }

    /// Waits (up to 60 s) until the model's snapshot satisfies `ok`, and returns it.
    async fn wait(&self, model: &str, what: &str, ok: impl Fn(&Value) -> bool) -> Value {
        let start = Instant::now();
        loop {
            let s = self.get(model);
            if ok(&s) {
                return s;
            }
            if start.elapsed() > Duration::from_secs(60) {
                panic!("timed out waiting for {model}: {what}\nlast snapshot: {s:#}");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

fn items(dropdown: &Value) -> Vec<(String, String)> {
    dropdown["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| {
                    (
                        i["id"].as_str().unwrap_or("").to_owned(),
                        i["label"].as_str().unwrap_or("").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn loaded_dropdown(d: &Value) -> bool {
    let p = d["placeholder"].as_str().unwrap_or("");
    !p.starts_with("Loading") && !p.starts_with("Choose an") && !p.starts_with("Choose a ")
}

/// New ticket in the main window's sheet, filled with the first org that has callers and services.
async fn create_ticket(app: &App, title: &str, assign: bool) -> (String, String) {
    app.send("shell", json!({"type": "newTicket"})).await;
    let sheet = app
        .wait("newTicket:sheet", "orgs loaded", |s| {
            s["type"] == "open" && loaded_dropdown(&s["organization"])
        })
        .await;
    let orgs = items(&sheet["organization"]);
    assert!(!orgs.is_empty(), "no organizations");
    let mut chosen = None;
    for (org_id, org_name) in orgs.iter().take(15) {
        app.send(
            "newTicket:sheet",
            json!({"type": "setOrganization", "id": org_id}),
        )
        .await;
        let s = app
            .wait("newTicket:sheet", "callers and services", |s| {
                loaded_dropdown(&s["caller"]) && loaded_dropdown(&s["service"])
            })
            .await;
        let (callers, services) = (items(&s["caller"]), items(&s["service"]));
        if callers.is_empty() || services.is_empty() {
            continue;
        }
        app.send(
            "newTicket:sheet",
            json!({"type": "setCaller", "id": callers[0].0}),
        )
        .await;
        for (service_id, _) in &services {
            app.send(
                "newTicket:sheet",
                json!({"type": "setService", "id": service_id}),
            )
            .await;
            let s = app
                .wait("newTicket:sheet", "subcategories", |s| {
                    loaded_dropdown(&s["subcategory"])
                })
                .await;
            if let Some((sub, _)) = items(&s["subcategory"]).first() {
                app.send(
                    "newTicket:sheet",
                    json!({"type": "setSubcategory", "id": sub}),
                )
                .await;
                chosen = Some(org_name.clone());
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let org = chosen.expect("an organization with a caller and a service with a subcategory");
    app.send(
        "newTicket:sheet",
        json!({"type": "setTitle", "title": title}),
    )
    .await;
    app.send("newTicket:sheet", json!({"type": "setDescription", "description": "Created by the iTop Wrap live test.\nSafe to ignore."})).await;
    app.send(
        "newTicket:sheet",
        json!({"type": "setAssignToMe", "assignToMe": assign}),
    )
    .await;
    let r = app.send("newTicket:sheet", json!({"type": "create"})).await;
    assert_eq!(r, Value::Null, "create: {r}");
    assert_eq!(app.get("newTicket:sheet")["type"], "closed");
    let list = app
        .wait("list:mine", "the new ticket selected", |l| {
            l["selection"].is_string()
        })
        .await;
    let id = list["selection"].as_str().unwrap_or("").to_owned();
    let r#ref = list["query"].as_str().unwrap_or("").to_owned();
    println!("created {ref} ({id}) for {org}");
    (id, r#ref)
}

/// Resolves the ticket shown in `detail:<scope>` with its prefilled service, then closes it.
async fn resolve_and_close(app: &App, scope: &str, id: &str, r#ref: &str) {
    let resolve = format!("resolve:{scope}");
    app.send(&resolve, json!({"type": "open", "ticketId": id}))
        .await;
    let s = app
        .wait(&resolve, "services", |s| loaded_dropdown(&s["service"]))
        .await;
    if s["service"]["selectedId"].is_null() {
        let first = items(&s["service"]).first().cloned().expect("a service");
        app.send(
            &resolve,
            json!({"type": "setService", "serviceId": first.0}),
        )
        .await;
    }
    let s = app
        .wait(&resolve, "subcategories", |s| {
            loaded_dropdown(&s["subcategory"])
        })
        .await;
    if s["subcategory"]["selectedId"].is_null() {
        let first = items(&s["subcategory"])
            .first()
            .cloned()
            .expect("a subcategory");
        app.send(
            &resolve,
            json!({"type": "setSubcategory", "subcategoryId": first.0}),
        )
        .await;
    }
    app.send(&resolve, json!({"type": "setSolution", "solution": "Closed by the live test.\n\nNothing was wrong."})).await;
    let r = app.send(&resolve, json!({"type": "confirm"})).await;
    assert_eq!(r, Value::Null, "resolve: {r}");
    assert_eq!(app.get(&resolve)["type"], "closed");
    let list = app.get(&format!("list:{scope}"));
    // The notice line belongs to the list view; an emptied list shows its message instead (LIST-69).
    if list["content"]["type"] == "list" {
        assert_eq!(
            list["content"]["body"]["notice"]["text"],
            format!("{ref} is resolved.")
        );
    }
    println!(
        "resolved {ref}; extra section: {}",
        list["content"]["body"]["extraSection"]["title"]
    );
    let detail = app
        .wait(&format!("detail:{scope}"), "Close offered", |d| {
            d["primary"]["kind"] == "confirmClose"
        })
        .await;
    assert_eq!(detail["ticket"]["status"], "resolved");
    app.send(&format!("detail:{scope}"), json!({"type": "close"}))
        .await;
    let list = app.get(&format!("list:{scope}"));
    if list["content"]["type"] == "list" {
        assert_eq!(
            list["content"]["body"]["notice"],
            json!({"text": format!("{ref} is closed."), "isError": false})
        );
    }
    assert_eq!(
        app.get(&format!("detail:{scope}"))["ticket"]["status"],
        "closed"
    );
    println!("closed {ref}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a real iTop test server; it writes"]
async fn the_app_against_a_real_itop() {
    let transport: Arc<dyn Transport> = Arc::new(ReqwestTransport::new().expect("transport"));
    let prefs = Arc::new(MemoryPrefs::new());
    let settings: Arc<dyn itop_kit::SettingsStore> = prefs.clone();
    let t = transport.clone();
    // Release builds' write access (the test server allows writes).
    let service = SessionService::with_make_client(
        Arc::new(MemorySecretStore::new()),
        SessionSettings::new(settings),
        WriteAccess::All,
        move |server, creds| ITopClient::new(server, creds, t.clone()),
    );
    let native = Arc::new(FakeNative::new(chrono::Utc::now()));
    let hub = Hub::new(Arc::new(RecordingSink::default()));
    let router = AppRouter::new(AppConfig {
        service,
        prefs,
        hub: hub.clone(),
        native: native.clone(),
        fmt: Fmt::local(),
        trace: false,
    });
    let app = App {
        router,
        hub,
        native,
    };

    // Launch with nothing saved: the sign-in form.
    app.router.native_event(NativeEvent::Launched).await;
    assert_eq!(app.get("session")["type"], "signedOut");

    // Sign in.
    app.send(
        "session",
        json!({"type": "setServer", "server": env("ITOP_URL")}),
    )
    .await;
    app.send(
        "session",
        json!({"type": "setUser", "user": env("ITOP_USER")}),
    )
    .await;
    let r = app
        .send(
            "session",
            json!({"type": "signIn", "password": env("ITOP_PASSWORD")}),
        )
        .await;
    assert_eq!(r, json!({"ok": true, "focus": null}), "sign-in");
    let session = app.get("session");
    println!(
        "signed in as {} on {}",
        session["displayName"], session["host"]
    );

    // My tickets.
    app.wait("list:mine", "loaded", |l| l["content"]["type"] != "loading")
        .await;
    app.send("list:mine", json!({"type": "refresh"})).await;
    let mine = app.get("list:mine");
    assert_ne!(mine["content"]["type"], "failed", "{mine:#}");
    println!("my tickets: {} ({})", mine["pager"], mine["updatedText"]);
    println!(
        "tray: {:?} / {}",
        app.get("menubar")["trayTitle"],
        app.get("menubar")["trayAccessibilityLabel"]
    );

    // Tickets an interrupted earlier run left open: resolve and close them first.
    let leftovers: Vec<(String, String)> = mine["content"]["body"]["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default())
        .filter(|r| {
            r["ticket"]["title"]
                .as_str()
                .is_some_and(|t| t.starts_with("iTop Wrap live test"))
        })
        .filter(|r| r["ticket"]["status"] == "assigned")
        .map(|r| {
            (
                r["ticket"]["id"].as_str().unwrap_or("").to_owned(),
                r["ticket"]["ref"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
    for (id, r#ref) in leftovers {
        println!("cleaning up {ref}");
        app.send("list:mine", json!({"type": "select", "ticketId": id}))
            .await;
        app.wait("detail:mine", "loaded", |d| d["body"]["type"] == "loaded")
            .await;
        resolve_and_close(&app, "mine", &id, &r#ref).await;
    }

    // Every section.
    for section in [
        "teamTickets",
        "configItems",
        "organizations",
        "people",
        "myTickets",
    ] {
        app.send("shell", json!({"type": "select", "item": section}))
            .await;
        assert_eq!(app.get("shell")["selection"], section);
    }
    let team = app
        .wait("list:team", "loaded", |l| l["content"]["type"] != "loading")
        .await;
    assert_ne!(team["content"]["type"], "failed", "{team:#}");
    println!("team tickets: {}", team["content"]["type"]);

    // Searches.
    app.send("shell", json!({"type": "select", "item": "configItems"}))
        .await;
    app.send("cmdb", json!({"type": "setQuery", "query": "se"}))
        .await;
    let cmdb = app
        .wait("cmdb", "CI search done", |c| {
            c["results"]["type"] != "searching" && c["results"]["type"] != "idle"
        })
        .await;
    assert_ne!(cmdb["results"]["type"], "failed", "{cmdb:#}");
    println!(
        "cmdb 'se': {} {}",
        cmdb["results"]["type"], cmdb["results"]["header"]
    );
    app.send("shell", json!({"type": "select", "item": "people"}))
        .await;
    app.send("people", json!({"type": "setQuery", "query": "an"}))
        .await;
    let people = app
        .wait("people", "people search done", |p| {
            p["results"]["type"] != "searching" && p["results"]["type"] != "idle"
        })
        .await;
    assert_ne!(people["results"]["type"], "failed", "{people:#}");
    println!("people 'an': {}", people["results"]["type"]);
    app.send("shell", json!({"type": "select", "item": "organizations"}))
        .await;
    let orgs = app
        .wait("orgs", "org list", |o| o["list"]["type"] != "loading")
        .await;
    assert_eq!(orgs["list"]["type"], "loaded", "{orgs:#}");
    let first_org = orgs["list"]["orgs"][0]["id"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    app.send("orgs", json!({"type": "select", "id": first_org}))
        .await;
    let org = app
        .wait("orgs", "org tickets", |o| {
            o["detail"]["tickets"]["type"] != "loading"
        })
        .await;
    println!(
        "org {}: {}",
        org["detail"]["name"], org["detail"]["tickets"]["type"]
    );

    // A new ticket, assigned to me: shown in My tickets as the search result.
    let stamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let (id, r#ref) = create_ticket(&app, &format!("iTop Wrap live test {stamp}"), true).await;
    assert_eq!(app.get("shell")["selection"], "myTickets");
    let list = app
        .wait("list:mine", "assigned notice", |l| {
            l["content"]["body"]["notice"]["text"]
                .as_str()
                .is_some_and(|t| t.contains("is created and") || t.contains("is created, but"))
        })
        .await;
    assert_eq!(
        list["content"]["body"]["notice"],
        json!({"text": format!("{ref} is created and assigned to you."), "isError": false})
    );

    // The detail loads; a public and a private entry.
    let detail = app
        .wait("detail:mine", "loaded", |d| d["body"]["type"] == "loaded")
        .await;
    assert_eq!(detail["ticket"]["ref"], r#ref);
    assert_eq!(detail["ticket"]["isMine"], true);
    app.send(
        "detail:mine",
        json!({"type": "setDraft", "text": "Public entry from the live test."}),
    )
    .await;
    assert_eq!(
        app.send("detail:mine", json!({"type": "send"})).await,
        Value::Bool(true),
        "{:#}",
        app.get("detail:mine")["composer"]
    );
    app.send("detail:mine", json!({"type": "setTab", "tab": "private"}))
        .await;
    app.send(
        "detail:mine",
        json!({"type": "setDraft", "text": "Private entry from the live test."}),
    )
    .await;
    assert_eq!(
        app.send("detail:mine", json!({"type": "send"})).await,
        Value::Bool(true),
        "{:#}",
        app.get("detail:mine")["composer"]
    );
    let detail = app.get("detail:mine");
    assert_eq!(
        detail["body"]["entries"].as_array().map(Vec::len),
        Some(1),
        "the private entry"
    );
    println!("entries added: {}", detail["body"]["entries"][0]["header"]);

    // The timer: iTop without Time Tracking. Under a minute saves nothing; a longer run fails to
    // save with iTop's message and offers Retry, like the Swift app.
    app.send("detail:mine", json!({"type": "toggleTimer"}))
        .await;
    assert_eq!(app.get("timer")["running"]["ticketId"], id);
    app.native.advance(chrono::Duration::minutes(2));
    app.send("timer", json!({"type": "stop"})).await;
    let notice = app.get("timer")["notice"].clone();
    println!("timer notice: {}", notice["text"]);
    assert_eq!(notice["canRetry"], true);
    assert!(
        notice["text"]
            .as_str()
            .unwrap_or("")
            .starts_with(&format!("Couldn't save 0:02 to {ref}. "))
    );
    app.send("timer", json!({"type": "clearNotice"})).await;

    // Reassign to a team and an agent (me).
    app.send("assign:mine", json!({"type": "open", "ticketId": id}))
        .await;
    let sheet = app
        .wait("assign:mine", "teams", |s| s["team"]["enabled"] == true)
        .await;
    if sheet["team"]["selectedId"].is_null() {
        let team = items(&sheet["team"]).first().cloned().expect("a team");
        app.send("assign:mine", json!({"type": "setTeam", "teamId": team.0}))
            .await;
    }
    let sheet = app
        .wait("assign:mine", "agents", |s| s["agent"]["enabled"] == true)
        .await;
    let me = items(&sheet["agent"])
        .into_iter()
        .find(|(_, l)| l.ends_with("(you)"))
        .expect("me among the agents");
    app.send("assign:mine", json!({"type": "setAgent", "agentId": me.0}))
        .await;
    let r = app.send("assign:mine", json!({"type": "confirm"})).await;
    assert_eq!(r, Value::Null, "assign: {r}");
    println!(
        "{}",
        app.get("list:mine")["content"]["body"]["notice"]["text"]
    );

    // Resolve, then close.
    resolve_and_close(&app, "mine", &id, &r#ref).await;

    // A second ticket, not assigned, put in my team: Team tickets offers "Assign to me".
    let (id2, ref2) =
        create_ticket(&app, &format!("iTop Wrap live test (team) {stamp}"), false).await;
    let team_id = app.get("assign:mine");
    drop(team_id);
    let my_team = {
        app.send("assign:mine", json!({"type": "open", "ticketId": id2}))
            .await;
        let s = app
            .wait("assign:mine", "teams", |s| s["team"]["enabled"] == true)
            .await;
        app.send("assign:mine", json!({"type": "close"})).await;
        // The test user's team: the one the agent list has them in.
        let mut found = None;
        for (team, _) in items(&s["team"]) {
            app.send("assign:mine", json!({"type": "open", "ticketId": id2}))
                .await;
            app.wait("assign:mine", "teams", |s| s["team"]["enabled"] == true)
                .await;
            app.send("assign:mine", json!({"type": "setTeam", "teamId": team}))
                .await;
            let s = app
                .wait("assign:mine", "agents", |s| {
                    s["agent"]["enabled"] == true
                        || s["agent"]["placeholder"] == "Nobody in this team"
                })
                .await;
            app.send("assign:mine", json!({"type": "close"})).await;
            if items(&s["agent"]).iter().any(|(_, l)| l.ends_with("(you)")) {
                found = Some(team);
                break;
            }
        }
        found.expect("the test user is in a team")
    };
    // Set the team directly (the app has no team-only change, like the Swift app).
    let key = id2.split("::").nth(1).unwrap_or("").to_owned();
    let class = id2.split("::").next().unwrap_or("").to_owned();
    let client = ITopClient::with_writes(
        ServerAddress::new(&env("ITOP_URL")).expect("server"),
        Credentials::new(env("ITOP_USER"), env("ITOP_PASSWORD")),
        transport.clone(),
        true,
        None,
        false,
    );
    let mut fields = JsonObject::new();
    fields.insert("team_id".into(), JsonValue::String(my_team.clone()));
    client
        .send(RestOperation::Update {
            class_name: class,
            key,
            fields,
            comment: "iTop Wrap live test".into(),
        })
        .await
        .expect("team set");
    app.send("shell", json!({"type": "select", "item": "teamTickets"}))
        .await;
    app.send("list:team", json!({"type": "refresh"})).await;
    let team = app.get("list:team");
    let row = team["content"]["body"]["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default())
        .find(|r| r["ticket"]["id"] == id2.as_str())
        .unwrap_or_else(|| panic!("{ref2} in Team tickets: {team:#}"));
    assert_eq!(row["agent"], json!({"type": "assignToMe"}));
    println!("{ref2} is in Team tickets with the Assign to me pill");
    app.send(
        "list:team",
        json!({"type": "rowAssignToMe", "ticketId": id2}),
    )
    .await;
    let team = app.get("list:team");
    assert_eq!(
        team["content"]["body"]["notice"],
        json!({"text": format!("{ref2} is assigned to you."), "isError": false})
    );
    assert_eq!(team["selection"], id2.as_str());
    app.wait("detail:team", "loaded", |d| {
        d["body"]["type"] == "loaded" && d["ticket"]["isMine"] == true
    })
    .await;
    resolve_and_close(&app, "team", &id2, &ref2).await;

    // Sign out: the form, no workspace.
    app.send("session", json!({"type": "signOut"})).await;
    assert_eq!(app.get("session")["type"], "signedOut");
    assert_eq!(app.get("list:mine"), Value::Null);
    println!("signed out");
}
