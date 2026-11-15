//! Organizations model tests, one per model-side item of checklist section 12, plus the shape.

use std::sync::atomic::{AtomicBool, Ordering};

use itop_kit::stub::{RecordedRequest, StubReply};
use serde_json::{Value, json};

use super::*;
use crate::models::cmdb::test_support::*;

fn handler(r: &RecordedRequest) -> StubReply {
    let class = r.class().unwrap_or_default();
    let key = oql(r);
    match class.as_str() {
        "Organization" => objects(
            "Organization",
            &[
                ("3", json!({"name": "Acme", "code": "AC"})),
                ("4", json!({"name": "beta labs", "code": ""})),
                ("2", json!({"name": "Zeta", "code": "ZT"})),
            ],
        ),
        "Person" => objects("Person", &[("1", json!({})), ("2", json!({}))]),
        "FunctionalCI" => objects("FunctionalCI", &[("1", json!({}))]),
        "UserRequest" if key.contains("org_id = 3") => objects(
            "UserRequest",
            &[
                (
                    "412",
                    ticket_fields(
                        "R-000412",
                        "VPN down",
                        "assigned",
                        "1",
                        ("5", "Jane Agent"),
                        "1",
                    ),
                ),
                (
                    "413",
                    ticket_fields("R-000413", "Printer", "new", "3", ("0", ""), "1"),
                ),
                (
                    "414",
                    ticket_fields(
                        "R-000414",
                        "Mail",
                        "assigned",
                        "2",
                        ("9", "bob builder"),
                        "1",
                    ),
                ),
            ],
        ),
        "UserRequest" | "Incident" if key.contains("org_id = ") => empty(),
        // The open-ticket counts behind the list: org 3 has two, org 2 one.
        "UserRequest" => objects(
            "UserRequest",
            &[("1", json!({"org_id": "3"})), ("2", json!({"org_id": "3"}))],
        ),
        "Incident" => objects("Incident", &[("1", json!({"org_id": "2"}))]),
        _ => empty(),
    }
}

fn model(e: &Env) -> Arc<OrgsModel> {
    OrgsModel::new(e.ctx.clone(), e.link_dyn())
}

async fn send(m: &OrgsModel, intent: Value) {
    assert_eq!(m.dispatch(MODEL_ID, intent).await.unwrap(), Value::Null);
}

fn snap(e: &Env) -> Value {
    e.hub.get(MODEL_ID)
}

fn org_list_requests(e: &Env) -> usize {
    e.oql()
        .iter()
        .filter(|q| q.as_str() == "SELECT Organization WHERE status = 'active'")
        .count()
}

#[tokio::test(start_paused = true)]
async fn org_01_05_list_sorted_with_open_counts() {
    let e = env(handler);
    let m = model(&e);
    assert_eq!(snap(&e)["list"], json!({"type": "loading"}));
    m.appear();
    settle().await;
    assert_eq!(
        snap(&e)["list"],
        json!({"type": "loaded", "orgs": [
            {"id": "3", "name": "Acme", "code": "AC", "openCount": 2},
            {"id": "4", "name": "beta labs", "code": "", "openCount": 0},
            {"id": "2", "name": "Zeta", "code": "ZT", "openCount": 1},
        ]})
    );
}

#[tokio::test(start_paused = true)]
async fn org_02_loaded_once_per_session() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    m.disappear();
    m.appear();
    settle().await;
    assert_eq!(org_list_requests(&e), 1);
}

#[tokio::test(start_paused = true)]
async fn org_02_04_failure_and_retry() {
    let fail = Arc::new(AtomicBool::new(true));
    let failing = fail.clone();
    let e = env(move |r: &RecordedRequest| {
        if failing.load(Ordering::SeqCst) {
            StubReply::json_status("{}", 500)
        } else {
            handler(r)
        }
    });
    let m = model(&e);
    m.appear();
    settle().await;
    assert_eq!(
        snap(&e)["list"],
        json!({"type": "failed", "text": "Couldn't load organizations. The server answered with HTTP 500."})
    );
    fail.store(false, Ordering::SeqCst);
    send(&m, json!({"type": "retryList"})).await;
    assert_eq!(snap(&e)["list"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn org_03_04_local_search_and_empty_texts() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "setQuery", "query": " zt "})).await;
    assert_eq!(snap(&e)["list"]["orgs"][0]["name"], "Zeta");
    assert_eq!(snap(&e)["list"]["orgs"].as_array().unwrap().len(), 1);
    send(&m, json!({"type": "setQuery", "query": "BETA"})).await;
    assert_eq!(snap(&e)["list"]["orgs"][0]["id"], "4");
    send(&m, json!({"type": "setQuery", "query": " nope "})).await;
    assert_eq!(
        snap(&e)["list"],
        json!({"type": "empty", "text": "No organization matches \u{201C}nope\u{201D}."})
    );
    assert_eq!(e.stub.requests().len(), 3, "searching is local");

    let none = env(|r: &RecordedRequest| {
        if r.class().as_deref() == Some("Organization") {
            empty()
        } else {
            handler(r)
        }
    });
    let m = model(&none);
    m.appear();
    settle().await;
    assert_eq!(
        snap(&none)["list"],
        json!({"type": "empty", "text": "Your iTop account can't see any organizations."})
    );
}

#[tokio::test(start_paused = true)]
async fn org_06_07_header_and_counts() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "3"})).await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["headerLine"], "Organization · AC");
    assert_eq!(d["peopleLabel"], "People");
    assert_eq!(d["configItemsLabel"], "Config items");
    assert_eq!(
        d["tickets"],
        json!({"type": "loading", "text": "Loading Acme's tickets"})
    );
    settle().await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["peopleLabel"], "People 2");
    assert_eq!(d["configItemsLabel"], "Config items 1");
    send(&m, json!({"type": "select", "id": "4"})).await;
    assert_eq!(snap(&e)["detail"]["headerLine"], "Organization");
    assert_eq!(
        snap(&e)["placeholder"],
        "Select an organization to see its open tickets."
    );
}

#[tokio::test(start_paused = true)]
async fn org_07_counts_failure_keeps_plain_labels() {
    let e = env(|r: &RecordedRequest| {
        if r.class().as_deref() == Some("Person") {
            StubReply::failure(-1009)
        } else {
            handler(r)
        }
    });
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "3"})).await;
    settle().await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["tickets"]["type"], "loaded");
    assert_eq!(d["peopleLabel"], "People");
    assert_eq!(d["configItemsLabel"], "Config items");
}

#[tokio::test(start_paused = true)]
async fn org_08_09_heading_and_reload_on_scope_change() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "4"})).await;
    settle().await;
    assert_eq!(snap(&e)["detail"]["ticketsHeading"], "Open tickets");
    assert_eq!(
        snap(&e)["detail"]["tickets"],
        json!({"type": "empty", "text": "beta labs has no open tickets."})
    );
    e.link.set_scope(TicketScope::new(true, Some(30)));
    m.scope_changed();
    settle().await;
    assert_eq!(snap(&e)["detail"]["ticketsHeading"], "Tickets");
    assert_eq!(
        snap(&e)["detail"]["tickets"],
        json!({"type": "empty", "text": "beta labs has no tickets in this range."})
    );
    let last = e
        .oql()
        .into_iter()
        .rev()
        .find(|q| q.starts_with("SELECT Incident") && q.contains("org_id = 4"))
        .unwrap();
    assert_eq!(
        last,
        "SELECT Incident WHERE start_date >= DATE_SUB(NOW(), INTERVAL 30 DAY) AND org_id = 4"
    );
}

#[tokio::test(start_paused = true)]
async fn org_10_ticket_failure_and_retry() {
    let fail = Arc::new(AtomicBool::new(true));
    let failing = fail.clone();
    let e = env(move |r: &RecordedRequest| {
        if failing.load(Ordering::SeqCst) && oql(r).contains("AND org_id = 3") {
            StubReply::failure(-1009)
        } else {
            handler(r)
        }
    });
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "3"})).await;
    settle().await;
    assert_eq!(
        snap(&e)["detail"]["tickets"],
        json!({"type": "failed", "text": "Couldn't load the tickets. Can't reach itop.example.com. Check your connection, then retry.", "canRetry": true})
    );
    fail.store(false, Ordering::SeqCst);
    send(&m, json!({"type": "retryTickets"})).await;
    settle().await;
    assert_eq!(snap(&e)["detail"]["tickets"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn org_11_ticket_table() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "3"})).await;
    settle().await;
    let t = snap(&e)["detail"]["tickets"]["table"].clone();
    assert_eq!(t["lastColumn"], "assignee");
    // Team order: New before Assigned.
    assert_eq!(t["sections"][0]["group"], "new");
    assert_eq!(t["sections"][0]["title"], "New");
    assert_eq!(t["sections"][1]["total"], 2);
    let rows = &t["sections"][1]["rows"];
    assert_eq!(
        rows[0],
        json!({"ticketId": "UserRequest::412", "ref": "R-000412", "title": "VPN down", "needsYou": false, "p1": true,
               "callerName": "Carl Caller", "assignee": {"type": "person", "initials": "JA", "name": "You"}})
    );
    assert_eq!(
        rows[1]["assignee"],
        json!({"type": "person", "initials": "BB", "name": "bob builder"})
    );
    assert_eq!(rows[1]["p1"], false);
    assert_eq!(
        t["sections"][0]["rows"][0]["assignee"],
        json!({"type": "unassigned"})
    );
    assert_eq!(t["pager"], json!({"type": "count", "text": "3 tickets"}));

    // Paged: 2 per page, page 2, then back to 1 when the page size changes.
    m.set_table_options(TicketSort::Priority, Some(2));
    send(&m, json!({"type": "setTicketPage", "page": 2})).await;
    let t = snap(&e)["detail"]["tickets"]["table"].clone();
    assert_eq!(
        t["pager"],
        json!({"type": "pages", "page": 2, "pageCount": 2, "label": "3–3 of 3", "canPrevious": true, "canNext": false})
    );
    assert_eq!(t["sections"][0]["total"], 2, "the group's whole count");
    m.set_table_options(TicketSort::Oldest, Some(2));
    assert_eq!(snap(&e)["detail"]["tickets"]["table"]["pager"]["page"], 1);
    send(&m, json!({"type": "setTicketPage", "page": 2})).await;
    // Reloading the tickets starts the table on page 1 again.
    send(&m, json!({"type": "retryTickets"})).await;
    settle().await;
    assert_eq!(snap(&e)["detail"]["tickets"]["table"]["pager"]["page"], 1);
}

#[tokio::test(start_paused = true)]
async fn org_shell_10_open_org_from_elsewhere() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "zeta"})).await;
    m.show_org("3");
    settle().await;
    assert!(
        e.stub.requests().is_empty(),
        "nothing loads until the section shows"
    );
    assert_eq!(snap(&e)["query"], "");
    assert_eq!(snap(&e)["selectedId"], "3");
    m.appear();
    settle().await;
    let s = snap(&e);
    assert_eq!(s["detail"]["name"], "Acme");
    assert_eq!(s["detail"]["tickets"]["type"], "loaded");
    // Coming back reloads the shown org's tickets (the detail's task runs on appear), not the list.
    let before = e.stub.requests().len();
    m.disappear();
    m.appear();
    settle().await;
    assert_eq!(org_list_requests(&e), 1);
    assert!(e.stub.requests().len() > before);
}

#[tokio::test(start_paused = true)]
async fn orgs_snapshot_json_shape() {
    let e = env(handler);
    let m = model(&e);
    m.appear();
    settle().await;
    send(&m, json!({"type": "select", "id": "3"})).await;
    settle().await;
    let s = snap(&e);
    let keys = |v: &Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&s),
        ["detail", "list", "placeholder", "query", "selectedId"]
    );
    assert_eq!(
        keys(&s["detail"]),
        [
            "code",
            "configItemsLabel",
            "headerLine",
            "id",
            "name",
            "peopleLabel",
            "tickets",
            "ticketsHeading"
        ]
    );
    assert_eq!(
        keys(&s["detail"]["tickets"]["table"]),
        ["lastColumn", "pager", "sections"]
    );
    assert_eq!(
        keys(&s["detail"]["tickets"]["table"]["sections"][0]),
        ["group", "rows", "title", "total"]
    );
}
