//! People model tests, one per model-side item of checklist section 13, plus the JSON shape.

use itop_kit::stub::{RecordedRequest, StubReply};
use serde_json::{Value, json};

use super::*;
use crate::models::cmdb::test_support::*;

fn agent() -> Value {
    json!({"name": "Agent", "first_name": "Jo", "org_id": "3", "org_id_friendlyname": "Acme", "email": "jo@acme.test",
           "phone": "+1 555", "function": "IT support",
           "team_list": [{"team_id": "1", "team_name": "Network"}, {"team_id": "2", "team_name": "Service desk"}]})
}

fn caller() -> Value {
    json!({"name": "Caller", "first_name": "Ann", "org_id": "3", "org_id_friendlyname": "Acme", "email": "", "phone": "",
           "function": "", "team_list": []})
}

fn handler(r: &RecordedRequest) -> StubReply {
    let class = r.class().unwrap_or_default();
    let key = oql(r);
    match class.as_str() {
        "Person" if key.contains("org_id = 3") => {
            objects_found("Person", &[("11", agent()), ("12", caller())], Some(140))
        }
        "Person" if key.contains("'%none%'") => empty(),
        "Person" if key.contains("'%fail%'") => StubReply::failure(-1009),
        "Person" if key.contains("'%many%'") => {
            objects_found("Person", &[("11", agent()), ("12", caller())], Some(240))
        }
        "Person" => objects_found("Person", &[("12", caller()), ("11", agent())], Some(2)),
        "UserRequest" if key.contains("agent_id = 11") => objects(
            "UserRequest",
            &[
                (
                    "412",
                    ticket_fields(
                        "R-000412",
                        "VPN down",
                        "assigned",
                        "2",
                        ("11", "Jo Agent"),
                        "1",
                    ),
                ),
                (
                    "413",
                    ticket_fields("R-000413", "Mail", "pending", "3", ("11", "Jo Agent"), "1"),
                ),
            ],
        ),
        _ => empty(),
    }
}

fn model(e: &Env) -> Arc<PeopleModel> {
    PeopleModel::new(e.ctx.clone(), e.link_dyn())
}

async fn send(m: &PeopleModel, intent: Value) {
    assert_eq!(m.dispatch(MODEL_ID, intent).await.unwrap(), Value::Null);
}

fn snap(e: &Env) -> Value {
    e.hub.get(MODEL_ID)
}

async fn search(m: &PeopleModel, q: &str) {
    send(m, json!({"type": "setQuery", "query": q})).await;
    advance(310).await;
}

#[tokio::test(start_paused = true)]
async fn ppl_01_idle_text_and_placeholder() {
    let e = env(handler);
    let _m = model(&e);
    assert_eq!(
        snap(&e)["results"],
        json!({"type": "idle", "text": "Search by name, for example a caller or an agent."})
    );
    assert_eq!(
        snap(&e)["placeholder"],
        "Select a person to see their details and tickets."
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_02_two_characters_debounce_and_sort() {
    let e = env(handler);
    let m = model(&e);
    send(&m, json!({"type": "setQuery", "query": "j"})).await;
    advance(400).await;
    assert!(e.stub.requests().is_empty());
    send(&m, json!({"type": "setQuery", "query": "jo"})).await;
    advance(299).await;
    assert_eq!(snap(&e)["results"]["type"], "searching");
    assert!(e.stub.requests().is_empty());
    advance(2).await;
    assert_eq!(e.oql().len(), 1);
    assert!(e.oql()[0].contains("'%jo%'"), "{:?}", e.oql());
    let people = &snap(&e)["results"]["people"];
    assert_eq!(people[0]["displayName"], "Ann Caller");
    assert_eq!(people[1]["displayName"], "Jo Agent");
    assert_eq!(snap(&e)["results"]["banner"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn ppl_02_typing_restores_previous_results() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    let loaded = snap(&e)["results"].clone();
    send(&m, json!({"type": "setQuery", "query": "jon"})).await;
    advance(50).await;
    send(&m, json!({"type": "setQuery", "query": "jo"})).await;
    settle().await;
    assert_eq!(snap(&e)["results"], loaded);
    advance(400).await;
    assert_eq!(e.stub.requests().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn ppl_03_banner_when_more_found() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "many").await;
    assert_eq!(
        snap(&e)["results"]["banner"],
        "Showing 2 of 240 people. Type more of the name to narrow it down."
    );
    m.show_org("3", "Acme");
    m.appear();
    settle().await;
    assert_eq!(
        snap(&e)["results"]["banner"],
        "Showing 2 of 140 people. Search by name to find someone."
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_04_states() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "fail").await;
    assert_eq!(
        snap(&e)["results"],
        json!({"type": "failed", "text": "Couldn't search iTop. Can't reach itop.example.com. Check your connection, then retry."})
    );
    search(&m, "none ").await;
    assert_eq!(
        snap(&e)["results"],
        json!({"type": "empty", "text": "Nobody matches \u{201C}none\u{201D}."})
    );
    let scoped = env(|r: &RecordedRequest| {
        if r.class().as_deref() == Some("Person") {
            empty()
        } else {
            handler(r)
        }
    });
    let m = model(&scoped);
    m.show_org("9", "Empty Org");
    m.appear();
    settle().await;
    assert_eq!(
        snap(&scoped)["results"],
        json!({"type": "empty", "text": "Nobody in this organization."})
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_05_org_scope() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    settle().await;
    m.disappear();
    m.show_org("3", "Acme");
    let s = snap(&e);
    assert_eq!(s["scope"], json!({"orgId": "3", "orgName": "Acme"}));
    assert_eq!(s["selectedId"], Value::Null);
    assert_eq!(s["query"], "");
    m.appear();
    settle().await;
    assert_eq!(e.oql().last().unwrap(), "SELECT Person WHERE org_id = 3");
    assert_eq!(snap(&e)["results"]["people"].as_array().unwrap().len(), 2);
    // Typing clears the scope.
    send(&m, json!({"type": "setQuery", "query": "an"})).await;
    assert_eq!(snap(&e)["scope"], Value::Null);
    // The × clears back to idle.
    m.show_org("3", "Acme");
    send(&m, json!({"type": "clearScope"})).await;
    assert_eq!(snap(&e)["scope"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn ppl_06_07_rows() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    assert_eq!(
        snap(&e)["results"]["people"],
        json!([
            {"id": "12", "displayName": "Ann Caller", "initials": "AC", "subtitle": "Caller · Acme"},
            {"id": "11", "displayName": "Jo Agent", "initials": "JA", "subtitle": "Agent · Network, Service desk"},
        ])
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_08_09_10_agent_detail() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["kindLine"], "Agent · IT support");
    assert_eq!(d["organization"], json!({"orgId": "3", "orgName": "Acme"}));
    assert_eq!(d["email"], "jo@acme.test");
    assert_eq!(d["phone"], "+1 555");
    assert_eq!(d["teams"], "Network, Service desk");
    assert_eq!(d["tab"], "assigned");
    assert_eq!(d["assignedLabel"], "Assigned to them");
    assert_eq!(
        d["tickets"],
        json!({"type": "loading", "text": "Loading tickets"})
    );
    settle().await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["assignedLabel"], "Assigned to them 2");
    assert_eq!(d["reportedLabel"], "Reported by them 0");
    assert_eq!(d["tickets"]["table"]["lastColumn"], "caller");
}

#[tokio::test(start_paused = true)]
async fn ppl_08_09_10_caller_detail_starts_on_reported() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "12"})).await;
    settle().await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["kindLine"], "Caller");
    assert_eq!(d["email"], Value::Null);
    assert_eq!(d["phone"], Value::Null);
    assert_eq!(d["teams"], Value::Null);
    assert_eq!(d["tab"], "reported");
    assert_eq!(
        d["tickets"],
        json!({"type": "empty", "text": "No open tickets reported by Ann Caller."})
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_10_11_13_range_change_keeps_tab() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    settle().await;
    send(&m, json!({"type": "setTab", "tab": "reported"})).await;
    assert_eq!(
        snap(&e)["detail"]["tickets"]["text"],
        "No open tickets reported by Jo Agent."
    );
    e.link.set_scope(TicketScope::new(false, Some(7)));
    m.scope_changed();
    settle().await;
    let d = snap(&e)["detail"].clone();
    assert_eq!(d["tab"], "reported");
    assert_eq!(
        d["tickets"]["text"],
        "No tickets in this range reported by Jo Agent."
    );
    let q: Vec<String> = e.oql().into_iter().filter(|q| q.contains("= 11")).collect();
    assert!(
        q.iter()
            .any(|q| q.contains("agent_id = 11") && q.contains("INTERVAL 7 DAY")),
        "{q:?}"
    );
    assert!(
        q.iter()
            .any(|q| q.contains("caller_id = 11") && q.contains("INTERVAL 7 DAY")),
        "{q:?}"
    );
    send(&m, json!({"type": "setTab", "tab": "assigned"})).await;
    assert_eq!(
        snap(&e)["detail"]["tickets"]["table"]["sections"][0]["group"],
        "assigned"
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_13_failure_has_no_retry() {
    let e = env(|r: &RecordedRequest| {
        if r.class().as_deref() == Some("UserRequest") {
            StubReply::failure(-1009)
        } else {
            handler(r)
        }
    });
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    settle().await;
    assert_eq!(
        snap(&e)["detail"]["tickets"],
        json!({"type": "failed", "text": "Couldn't load the tickets. Can't reach itop.example.com. Check your connection, then retry.", "canRetry": false})
    );
}

#[tokio::test(start_paused = true)]
async fn ppl_14_table_columns_per_tab_and_paging() {
    let e = env(handler);
    let m = model(&e);
    m.set_table_options(TicketSort::Priority, Some(1));
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    settle().await;
    send(&m, json!({"type": "setTicketPage", "page": 2})).await;
    let t = snap(&e)["detail"]["tickets"]["table"].clone();
    assert_eq!(t["pager"]["label"], "2–2 of 2");
    assert_eq!(
        t["sections"][0]["rows"][0]["assignee"],
        json!({"type": "person", "initials": "JA", "name": "Jo Agent"})
    );
    // Another tab shows other tickets: back to page 1 there.
    send(&m, json!({"type": "setTab", "tab": "reported"})).await;
    send(&m, json!({"type": "setTab", "tab": "assigned"})).await;
    assert_eq!(snap(&e)["detail"]["tickets"]["table"]["pager"]["page"], 1);
}

#[tokio::test(start_paused = true)]
async fn ppl_detail_hides_while_searching_and_reloads_when_back() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
    settle().await;
    send(&m, json!({"type": "setTab", "tab": "reported"})).await;
    send(&m, json!({"type": "setQuery", "query": "jo "})).await;
    // Same trimmed query, already loaded: no new search, the detail stays.
    assert_eq!(snap(&e)["detail"]["tab"], "reported");
    search(&m, "agent").await;
    // A new search for the same person shows them again, back on their default tab.
    assert_eq!(snap(&e)["detail"]["tab"], "assigned");
}

#[tokio::test(start_paused = true)]
async fn people_snapshot_json_shape() {
    let e = env(handler);
    let m = model(&e);
    search(&m, "jo").await;
    send(&m, json!({"type": "select", "id": "11"})).await;
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
            "detail",
            "placeholder",
            "query",
            "results",
            "scope",
            "selectedId"
        ]
    );
    assert_eq!(keys(&s["results"]), ["banner", "people", "type"]);
    assert_eq!(
        keys(&s["detail"]),
        [
            "assignedLabel",
            "displayName",
            "email",
            "id",
            "initials",
            "kindLine",
            "organization",
            "phone",
            "reportedLabel",
            "tab",
            "teams",
            "tickets"
        ]
    );
}
