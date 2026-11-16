//! CMDB model tests, one per model-side item of checklist section 11, plus the JSON shape.

use std::sync::atomic::{AtomicBool, Ordering};

use itop_kit::stub::{RecordedRequest, StubReply};
use serde_json::{Value, json};

use super::test_support::*;
use super::*;
use crate::native::fake::Call;

fn ci_row(name: &str, class: &str, org: &str) -> Value {
    json!({"name": name, "finalclass": class, "org_id_friendlyname": org})
}

fn handler(r: &RecordedRequest) -> StubReply {
    let d = r.json_data();
    let op = d["operation"].as_str().unwrap_or("");
    let class = d["class"].as_str().unwrap_or("");
    let key = oql(r);
    if op == "core/get_related" {
        return if d["direction"] == "down" {
            objects(
                "ApplicationSolution",
                &[
                    ("20", json!({"friendlyname": "Mail"})),
                    ("21", json!({"friendlyname": "Intranet"})),
                ],
            )
        } else {
            objects("Server", &[("30", json!({"friendlyname": "srv-01"}))])
        };
    }
    match class {
        "FunctionalCI" if key.contains("'%none%'") => empty(),
        "FunctionalCI" if key.contains("'%fail%'") => StubReply::failure(-1009),
        "FunctionalCI" if key.contains("'%many%'") => {
            let rows: Vec<(String, Value)> = (0..50)
                .map(|i| {
                    (
                        format!("{}", 100 + i),
                        ci_row(&format!("host-{i:02}"), "Server", "Acme"),
                    )
                })
                .collect();
            let refs: Vec<(&str, Value)> =
                rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            objects("FunctionalCI", &refs)
        }
        "FunctionalCI" if key.contains("'%one%'") => {
            objects("FunctionalCI", &[("9", ci_row("one-box", "PC", ""))])
        }
        "FunctionalCI" if key.contains("org_id = 3") => objects(
            "FunctionalCI",
            &[("40", ci_row("acme-fw", "NetworkDevice", "Acme"))],
        ),
        "FunctionalCI" => objects(
            "FunctionalCI",
            &[
                ("7", ci_row("vpn-gw-01", "NetworkDevice", "Acme")),
                ("8", ci_row("vpn-gw-02", "ApplicationSolution", "")),
            ],
        ),
        "UserRequest" => objects(
            "UserRequest",
            &[(
                "412",
                ticket_fields(
                    "R-000412",
                    "VPN down",
                    "escalated_ttr",
                    "1",
                    ("5", "Jane Agent"),
                    "1",
                ),
            )],
        ),
        "Incident" => empty(),
        _ => objects(
            class,
            &[(
                key.as_str(),
                json!({"name": "vpn-gw-01", "status": "production", "business_criticity": "not_applicable",
                       "description": "<p>Edge router</p>", "org_id_friendlyname": "Acme"}),
            )],
        ),
    }
}

fn model(e: &Env) -> Arc<CmdbModel> {
    CmdbModel::new(e.ctx.clone(), e.link_dyn())
}

async fn send(m: &CmdbModel, intent: Value) -> Value {
    m.dispatch(MODEL_ID, intent).await.unwrap()
}

fn snap(e: &Env) -> Value {
    e.hub.get(MODEL_ID)
}

#[tokio::test(start_paused = true)]
async fn cmdb_01_idle_text_and_placeholder() {
    let e = env(handler);
    let _m = model(&e);
    let s = snap(&e);
    assert_eq!(
        s["results"],
        json!({"type": "idle", "text": "Search by name, for example a server or application."})
    );
    assert_eq!(s["placeholder"], "Select a config item to see its details.");
    assert_eq!(s["query"], "");
    assert_eq!(s["scope"], Value::Null);
    assert_eq!(s["current"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn cmdb_02_search_needs_two_characters() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": " v "})).await;
    advance(400).await;
    assert!(e.stub.requests().is_empty());
    assert_eq!(snap(&e)["results"]["type"], "idle");
    send(&m, json!({"type": "setQuery", "query": "vp"})).await;
    settle().await;
    assert_eq!(snap(&e)["results"]["type"], "searching");
    assert!(e.stub.requests().is_empty());
}

#[tokio::test(start_paused = true)]
async fn cmdb_02_debounce_300ms() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(299).await;
    assert!(e.stub.requests().is_empty());
    advance(2).await;
    assert_eq!(e.stub.requests().len(), 1);
    assert_eq!(snap(&e)["results"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn cmdb_02_typing_cancels_and_restores_previous_results() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    let loaded = snap(&e)["results"].clone();
    assert_eq!(loaded["type"], "loaded");
    // Typing more starts a new debounce; going back to "vpn" within it restores the results
    // and doesn't search again (same query, already loaded).
    send(&m, json!({"type": "setQuery", "query": "vpn-"})).await;
    advance(100).await;
    assert_eq!(snap(&e)["results"]["type"], "searching");
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    settle().await;
    assert_eq!(snap(&e)["results"], loaded);
    advance(400).await;
    assert_eq!(e.stub.requests().len(), 1);
    // Fewer than two characters: idle, and the searched query is forgotten.
    send(&m, json!({"type": "setQuery", "query": "v"})).await;
    settle().await;
    assert_eq!(snap(&e)["results"]["type"], "idle");
}

#[tokio::test(start_paused = true)]
async fn cmdb_02_coming_back_with_same_query_does_not_search_again() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    m.disappear();
    m.appear();
    advance(400).await;
    assert_eq!(e.stub.requests().len(), 1);
    assert_eq!(snap(&e)["results"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn cmdb_02_leaving_during_debounce_restores_previous() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(100).await;
    m.disappear();
    assert_eq!(snap(&e)["results"]["type"], "idle");
    advance(400).await;
    assert!(e.stub.requests().is_empty());
}

#[tokio::test(start_paused = true)]
async fn cmdb_03_query_and_real_class() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    assert!(
        e.oql()[0].starts_with("SELECT FunctionalCI WHERE name LIKE '%vpn%'"),
        "{:?}",
        e.oql()
    );
    let items = &snap(&e)["results"]["items"];
    assert_eq!(items[0]["className"], "NetworkDevice");
    assert_eq!(items[0]["id"], "NetworkDevice::7");
}

#[tokio::test(start_paused = true)]
async fn cmdb_04_failure_and_no_results_texts() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "fail"})).await;
    advance(310).await;
    assert_eq!(
        snap(&e)["results"],
        json!({"type": "failed", "text": "Couldn't search iTop. Can't reach itop.example.com. Check your connection, then retry."})
    );
    send(&m, json!({"type": "setQuery", "query": " none "})).await;
    advance(310).await;
    assert_eq!(
        snap(&e)["results"],
        json!({"type": "empty", "text": "No config items match \u{201C}none\u{201D}."})
    );
}

#[tokio::test(start_paused = true)]
async fn cmdb_05_header_and_rows() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    let r = snap(&e)["results"].clone();
    assert_eq!(r["header"], "2 results");
    assert_eq!(r["items"][0]["subtitle"], "Network device · Acme");
    assert_eq!(r["items"][1]["subtitle"], "Application solution");
    send(&m, json!({"type": "setQuery", "query": "one"})).await;
    advance(310).await;
    assert_eq!(snap(&e)["results"]["header"], "1 result");
    assert_eq!(snap(&e)["results"]["items"][0]["subtitle"], "PC");
    send(&m, json!({"type": "setQuery", "query": "many"})).await;
    advance(310).await;
    assert_eq!(snap(&e)["results"]["header"], "First 50 results");
}

#[tokio::test(start_paused = true)]
async fn cmdb_07_org_scope() {
    let e = env(handler);
    let m = model(&e);
    m.show_org("3", "Acme");
    m.appear();
    settle().await;
    let s = snap(&e);
    assert_eq!(s["scope"], json!({"orgId": "3", "orgName": "Acme"}));
    assert_eq!(s["results"]["items"][0]["name"], "acme-fw");
    assert_eq!(
        e.oql(),
        vec!["SELECT FunctionalCI WHERE org_id = 3".to_owned()]
    );
    // Coming back doesn't load the org again.
    m.disappear();
    m.appear();
    settle().await;
    assert_eq!(e.stub.requests().len(), 1);
    // The × clears it back to idle.
    send(&m, json!({"type": "clearScope"})).await;
    settle().await;
    assert_eq!(snap(&e)["scope"], Value::Null);
    assert_eq!(snap(&e)["results"]["type"], "idle");
    // Typing a search clears the scope.
    m.show_org("3", "Acme");
    settle().await;
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    settle().await;
    assert_eq!(snap(&e)["scope"], Value::Null);
    advance(310).await;
    assert_eq!(snap(&e)["results"]["header"], "2 results");
}

#[tokio::test(start_paused = true)]
async fn cmdb_08_select_and_open_from_chip() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    send(&m, json!({"type": "select", "id": "NetworkDevice::7"})).await;
    settle().await;
    let s = snap(&e);
    assert_eq!(s["selectedId"], "NetworkDevice::7");
    assert_eq!(s["current"]["name"], "vpn-gw-01");
    assert_eq!(s["current"]["orgName"], "Acme");
    assert_eq!(s["current"]["body"]["type"], "loaded");
    // A chip opens a CI that isn't in the results.
    send(
        &m,
        json!({"type": "openCI", "key": "30", "className": "Server", "name": "srv-01"}),
    )
    .await;
    settle().await;
    let s = snap(&e);
    assert_eq!(s["selectedId"], "Server::30");
    assert_eq!(s["current"]["id"], "Server::30");
    assert_eq!(s["current"]["orgName"], "");
    // Deselecting keeps the detail (ShellView shows `current`).
    send(&m, json!({"type": "select", "id": null})).await;
    assert_eq!(snap(&e)["current"]["id"], "Server::30");
}

#[tokio::test(start_paused = true)]
async fn cmdb_08_open_from_ticket_loads_when_section_appears() {
    let e = env(handler);
    let m = model(&e);
    m.open(ConfigItem::new("7", "NetworkDevice", "vpn-gw-01", ""));
    settle().await;
    assert!(e.stub.requests().is_empty());
    assert_eq!(
        snap(&e)["current"]["body"]["text"],
        "Loading vpn-gw-01 from iTop"
    );
    m.appear();
    settle().await;
    assert_eq!(snap(&e)["current"]["body"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn cmdb_09_detail_loads_fields_relationships_and_tickets() {
    let e = env(handler);
    let m = model(&e);
    send(
        &m,
        json!({"type": "openCI", "key": "7", "className": "NetworkDevice", "name": "vpn-gw-01"}),
    )
    .await;
    settle().await;
    let ops: Vec<String> = e
        .stub
        .requests()
        .iter()
        .map(|r| r.operation().unwrap_or_default())
        .collect();
    assert_eq!(ops.iter().filter(|o| *o == "core/get_related").count(), 2);
    let body = &snap(&e)["current"]["body"];
    assert_eq!(
        body["fields"][0],
        json!({"label": "Status", "value": "Production"})
    );
    assert_eq!(
        body["fields"][1],
        json!({"label": "Organization", "value": "Acme"})
    );
    assert_eq!(
        body["fields"][2],
        json!({"label": "Business criticality", "value": "Not applicable"})
    );
}

#[tokio::test(start_paused = true)]
async fn cmdb_10_loading_failure_and_retry() {
    let fail = Arc::new(AtomicBool::new(true));
    let failing = fail.clone();
    let e = env(move |r: &RecordedRequest| {
        if failing.load(Ordering::SeqCst) && r.class().as_deref() == Some("Rack") {
            StubReply::failure(-1009)
        } else {
            handler(r)
        }
    });
    let m = model(&e);
    send(
        &m,
        json!({"type": "openCI", "key": "99", "className": "Rack", "name": "rack-9"}),
    )
    .await;
    let first = e
        .sink
        .take()
        .into_iter()
        .filter(|(id, _)| id == MODEL_ID)
        .map(|(_, v)| v)
        .collect::<Vec<_>>();
    assert_eq!(
        first.last().unwrap()["current"]["body"],
        json!({"type": "loading", "text": "Loading rack-9 from iTop"})
    );
    settle().await;
    fail.store(false, Ordering::SeqCst);
    assert_eq!(
        snap(&e)["current"]["body"],
        json!({"type": "failed", "text": "Couldn't load this config item. Can't reach itop.example.com. Check your connection, then retry."})
    );
    send(&m, json!({"type": "retryDetail"})).await;
    assert_eq!(snap(&e)["current"]["body"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn cmdb_11_header_copy_and_web_url() {
    let e = env(handler);
    let m = model(&e);
    send(
        &m,
        json!({"type": "openCI", "key": "7", "className": "NetworkDevice", "name": "vpn-gw-01"}),
    )
    .await;
    assert_eq!(snap(&e)["current"]["headerLine"], "Network device");
    settle().await;
    let c = snap(&e)["current"].clone();
    assert_eq!(c["headerLine"], "Network device · Production");
    assert_eq!(
        c["webUrl"],
        "https://itop.example.com/pages/UI.php?operation=details&class=NetworkDevice&id=7"
    );
    send(&m, json!({"type": "copyName"})).await;
    assert_eq!(e.native.calls(), vec![Call::CopyText("vpn-gw-01".into())]);
    assert_eq!(snap(&e)["current"]["copied"], true);
    advance(1990).await;
    assert_eq!(snap(&e)["current"]["copied"], true);
    advance(20).await;
    assert_eq!(snap(&e)["current"]["copied"], false);
}

#[tokio::test(start_paused = true)]
async fn cmdb_13_14_15_description_relationships_and_tickets() {
    let e = env(handler);
    let m = model(&e);
    send(
        &m,
        json!({"type": "openCI", "key": "7", "className": "NetworkDevice", "name": "vpn-gw-01"}),
    )
    .await;
    settle().await;
    let b = snap(&e)["current"]["body"].clone();
    assert_eq!(b["description"], "Edge router");
    assert_eq!(
        b["dependsOn"],
        json!([{"key": "30", "className": "Server", "name": "srv-01", "help": "Server: open srv-01"}])
    );
    assert_eq!(b["usedBy"][0]["name"], "Intranet");
    assert_eq!(
        b["usedBy"][0]["help"],
        "Application solution: open Intranet"
    );
    assert_eq!(
        b["usedBySentence"],
        "If vpn-gw-01 fails, Intranet and Mail are affected."
    );
    assert_eq!(
        b["openTickets"],
        json!([{"ticketId": "UserRequest::412", "ref": "R-000412", "title": "VPN down", "needsYou": true,
                "statusLine": "Escalated TTR · You"}])
    );
    assert_eq!(b["openTicketsEmptyText"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn cmdb_14_15_empty_relationships_and_no_tickets() {
    let e = env(|r: &RecordedRequest| {
        let d = r.json_data();
        match (
            d["operation"].as_str().unwrap_or(""),
            d["class"].as_str().unwrap_or(""),
        ) {
            ("core/get_related", _) => empty(),
            (_, "UserRequest" | "Incident") => empty(),
            (_, c) => objects(c, &[("7", json!({"name": "lonely", "description": ""}))]),
        }
    });
    let m = model(&e);
    send(
        &m,
        json!({"type": "openCI", "key": "7", "className": "Server", "name": "lonely"}),
    )
    .await;
    settle().await;
    let b = snap(&e)["current"]["body"].clone();
    assert_eq!(b["description"], Value::Null);
    assert_eq!(b["dependsOn"], json!([]));
    assert_eq!(
        b["usedBySentence"],
        "Nothing in the CMDB depends on lonely."
    );
    assert_eq!(b["openTicketsEmptyText"], "No open tickets on lonely.");
}

#[tokio::test(start_paused = true)]
async fn cmdb_snapshot_json_shape() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "vpn"})).await;
    advance(310).await;
    send(&m, json!({"type": "select", "id": "NetworkDevice::7"})).await;
    settle().await;
    let s = snap(&e);
    let keys = |v: &Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&s),
        [
            "current",
            "placeholder",
            "query",
            "results",
            "scope",
            "selectedId"
        ]
    );
    assert_eq!(keys(&s["results"]), ["header", "items", "type"]);
    assert_eq!(
        s["results"]["items"][0],
        json!({"id": "NetworkDevice::7", "key": "7", "className": "NetworkDevice", "name": "vpn-gw-01", "subtitle": "Network device · Acme"})
    );
    assert_eq!(
        keys(&s["current"]),
        [
            "body",
            "className",
            "copied",
            "headerLine",
            "id",
            "key",
            "name",
            "orgName",
            "webUrl"
        ]
    );
    assert_eq!(
        keys(&s["current"]["body"]),
        [
            "dependsOn",
            "description",
            "fields",
            "openTickets",
            "openTicketsEmptyText",
            "type",
            "usedBy",
            "usedBySentence"
        ]
    );
    assert_eq!(s["current"]["body"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn cmdb_bad_intent_and_model_id() {
    let e = env(handler);
    let m = model(&e);
    assert!(matches!(
        m.dispatch(MODEL_ID, json!({"type": "nope"})).await,
        Err(AppError::InvalidIntent { .. })
    ));
    assert!(matches!(
        m.dispatch("orgs", json!({"type": "clearScope"})).await,
        Err(AppError::UnknownModel { .. })
    ));
}
