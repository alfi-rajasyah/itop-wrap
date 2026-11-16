//! New ticket tests, one per model-side item of checklist section 8, plus the JSON shape.

use itop_kit::stub::{RecordedRequest, StubReply};
use serde_json::{Value, json};

use super::*;
use crate::models::cmdb::test_support::*;
use crate::native::fake::Call;

fn handler(r: &RecordedRequest) -> StubReply {
    let class = r.class().unwrap_or_default();
    let op = r.operation().unwrap_or_default();
    let key = oql(r);
    if op == "core/create" {
        if r.json_data()["fields"]["title"] == "refuse" {
            return StubReply::json(
                r#"{"code":100,"message":"Error: servicesubcategory_id: not allowed"}"#,
            );
        }
        if r.json_data()["fields"]["title"] == "nokey" {
            return empty();
        }
        return StubReply::json(&format!(
            r#"{{"code":0,"message":"","objects":{{"{class}::900":{{"class":"{class}","key":"900","code":0,"message":"created","fields":{{"id":"900"}}}}}}}}"#
        ));
    }
    match class.as_str() {
        "Organization" => objects(
            "Organization",
            &[
                ("5", json!({"friendlyname": "Beta"})),
                ("4", json!({"friendlyname": "Acme"})),
            ],
        ),
        "Person" if key.contains("org_id = 4") => {
            objects("Person", &[("31", json!({"friendlyname": "Carl Caller"}))])
        }
        "Person" if key.contains("org_id = 6") => empty(),
        "Person" => objects("Person", &[("41", json!({"friendlyname": "Bea Caller"}))]),
        "Service" if key.contains("cc.org_id = 4") => objects(
            "Service",
            &[
                ("6", json!({"friendlyname": "Mail"})),
                ("3", json!({"friendlyname": "VPN"})),
            ],
        ),
        "Service" if key.contains("cc.org_id") => empty(),
        "Service" => objects("Service", &[("7", json!({"friendlyname": "Generic"}))]),
        "ServiceSubcategory" if key.contains("service_id = 3") => objects(
            "ServiceSubcategory",
            &[("12", json!({"friendlyname": "Drops"}))],
        ),
        "ServiceSubcategory" => empty(),
        "UserRequest" | "Incident" => objects(
            &class,
            &[(
                "900",
                ticket_fields(
                    if class == "Incident" {
                        "I-000900"
                    } else {
                        "R-000900"
                    },
                    "VPN drops",
                    "new",
                    "4",
                    ("0", ""),
                    "0",
                ),
            )],
        ),
        _ => empty(),
    }
}

fn writing() -> Env {
    env_with(handler, true)
}

fn model(e: &Env) -> Arc<NewTickets> {
    NewTickets::new(e.ctx.clone(), e.link_dyn())
}

async fn send(m: &NewTickets, id: &str, intent: Value) -> Value {
    m.dispatch(id, intent).await.unwrap()
}

fn sheet(e: &Env) -> Value {
    e.hub.get(SHEET_ID)
}

/// Opens the sheet plain and picks org 4, caller 31, service 3, subcategory 12, a title and a description.
async fn filled_sheet(m: &NewTickets) {
    send(m, SHEET_ID, json!({"type": "open"})).await;
    settle().await;
    send(m, SHEET_ID, json!({"type": "setOrganization", "id": "4"})).await;
    settle().await;
    send(m, SHEET_ID, json!({"type": "setCaller", "id": "31"})).await;
    send(m, SHEET_ID, json!({"type": "setService", "id": "3"})).await;
    settle().await;
    send(m, SHEET_ID, json!({"type": "setSubcategory", "id": "12"})).await;
    send(
        m,
        SHEET_ID,
        json!({"type": "setTitle", "title": "  VPN drops  "}),
    )
    .await;
    send(
        m,
        SHEET_ID,
        json!({"type": "setDescription", "description": "Since Monday."}),
    )
    .await;
}

fn creates(e: &Env) -> Vec<Value> {
    e.stub
        .requests()
        .iter()
        .filter(|r| r.operation().as_deref() == Some("core/create"))
        .map(|r| r.json_data())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn new_01_closed_until_requested_then_sheet_opens() {
    let e = writing();
    let m = model(&e);
    assert_eq!(sheet(&e), json!({"type": "closed"}));
    assert_eq!(e.hub.get(WINDOW_ID), json!({"type": "closed"}));
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    assert_eq!(sheet(&e)["type"], "open");
    assert_eq!(e.hub.get(WINDOW_ID), json!({"type": "closed"}));
    assert!(e.native.calls().is_empty(), "the sheet needs no window");
}

#[tokio::test(start_paused = true)]
async fn new_02_window_opens_and_keeps_its_form() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    send(&m, WINDOW_ID, json!({"type": "setTitle", "title": "kept"})).await;
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    assert_eq!(e.hub.get(WINDOW_ID)["title"], "kept");
    assert_eq!(
        e.native.calls(),
        vec![Call::ShowNewTicketWindow, Call::ShowNewTicketWindow]
    );
    send(&m, WINDOW_ID, json!({"type": "dismiss"})).await;
    assert_eq!(e.hub.get(WINDOW_ID), json!({"type": "closed"}));
    assert_eq!(e.native.calls().last(), Some(&Call::CloseNewTicketWindow));
    m.clear();
    assert_eq!(e.hub.get(WINDOW_ID), Value::Null);
}

#[tokio::test(start_paused = true)]
async fn new_02_window_closes_itself_with_no_session() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    m.clear();
    assert_eq!(
        e.native.calls(),
        vec![Call::ShowNewTicketWindow, Call::CloseNewTicketWindow]
    );
}

#[tokio::test(start_paused = true)]
async fn new_03_from_an_org() {
    let e = writing();
    let m = model(&e);
    m.request(
        NewTicketInstance::Sheet,
        NewTicketStart::for_org("5", "Beta"),
    )
    .await;
    settle().await;
    let s = sheet(&e);
    assert_eq!(s["subtitle"], Value::Null);
    assert_eq!(s["organization"]["selectedId"], "5");
    assert_eq!(
        s["caller"]["items"],
        json!([{"id": "41", "label": "Bea Caller"}])
    );
    // Org 5 has no contracts: every live service.
    assert_eq!(
        s["service"]["items"],
        json!([{"id": "7", "label": "Generic"}])
    );
    assert_eq!(s["caller"]["selectedId"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn new_03_15_from_a_ci() {
    let e = writing();
    let m = model(&e);
    m.request(
        NewTicketInstance::Sheet,
        NewTicketStart::for_ci("77", "vpn-gw-01", "Acme"),
    )
    .await;
    settle().await;
    assert_eq!(sheet(&e)["subtitle"], "On vpn-gw-01");
    assert_eq!(
        sheet(&e)["organization"]["selectedId"],
        "4",
        "matched by the CI's org name"
    );
    send(&m, SHEET_ID, json!({"type": "setCaller", "id": "31"})).await;
    send(&m, SHEET_ID, json!({"type": "setService", "id": "3"})).await;
    settle().await;
    send(&m, SHEET_ID, json!({"type": "setSubcategory", "id": "12"})).await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "t"})).await;
    send(
        &m,
        SHEET_ID,
        json!({"type": "setDescription", "description": "d"}),
    )
    .await;
    assert_eq!(
        send(&m, SHEET_ID, json!({"type": "create"})).await,
        Value::Null
    );
    assert_eq!(
        creates(&e)[0]["fields"]["functionalcis_list"],
        json!([{"functionalci_id": "77"}])
    );
}

#[tokio::test(start_paused = true)]
async fn new_05_type_defaults_to_user_request() {
    let e = writing();
    let m = model(&e);
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    assert_eq!(sheet(&e)["ticketClass"], "UserRequest");
    send(
        &m,
        SHEET_ID,
        json!({"type": "setClass", "ticketClass": "Incident"}),
    )
    .await;
    assert_eq!(sheet(&e)["ticketClass"], "Incident");
}

#[tokio::test(start_paused = true)]
async fn new_06_organization_dropdown() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    let first = e
        .sink
        .take()
        .into_iter()
        .find(|(id, v)| id == SHEET_ID && v["type"] == "open")
        .unwrap()
        .1;
    assert_eq!(
        first["organization"],
        json!({"items": [], "selectedId": null, "placeholder": "Loading organizations…", "enabled": false,
               "search": {"prompt": "Search organizations", "noMatch": "No organization matches"}})
    );
    let org = sheet(&e)["organization"].clone();
    assert_eq!(
        org["items"],
        json!([{"id": "4", "label": "Acme"}, {"id": "5", "label": "Beta"}])
    );
    assert_eq!(org["placeholder"], "Choose…");
    assert_eq!(org["enabled"], true);
    assert_eq!(e.oql()[0], "SELECT Organization WHERE status = 'active'");

    let failing = env_with(
        |r: &RecordedRequest| {
            if r.class().as_deref() == Some("Organization") {
                StubReply::failure(-1009)
            } else {
                handler(r)
            }
        },
        true,
    );
    let m = model(&failing);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    assert_eq!(
        sheet(&failing)["organization"]["placeholder"],
        "Couldn't load: Can't reach itop.example.com. Check your connection, then retry."
    );
    assert_eq!(sheet(&failing)["organization"]["enabled"], false);

    let none = env_with(
        |r: &RecordedRequest| {
            if r.class().as_deref() == Some("Organization") {
                empty()
            } else {
                handler(r)
            }
        },
        true,
    );
    let m = model(&none);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    assert_eq!(
        sheet(&none)["organization"]["placeholder"],
        "No organizations found"
    );
}

#[tokio::test(start_paused = true)]
async fn new_07_caller_dropdown() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    let caller = sheet(&e)["caller"].clone();
    assert_eq!(
        caller,
        json!({"items": [], "selectedId": null, "placeholder": "Choose an organization first", "enabled": false,
               "search": {"prompt": "Search people in the organization", "noMatch": "No caller in this organization matches"}})
    );
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "4"})).await;
    assert_eq!(sheet(&e)["caller"]["placeholder"], "Loading people…");
    settle().await;
    let caller = sheet(&e)["caller"].clone();
    assert_eq!(caller["placeholder"], "Choose…");
    assert_eq!(caller["enabled"], true);
    assert_eq!(
        caller["search"],
        json!({"prompt": "Search people in Acme", "noMatch": "No caller in Acme matches"})
    );
    assert!(
        e.oql()
            .contains(&"SELECT Person WHERE org_id = 4 AND status = 'active'".to_owned())
    );
}

#[tokio::test(start_paused = true)]
async fn new_07_nobody_in_org() {
    let e = env_with(
        |r: &RecordedRequest| {
            if r.class().as_deref() == Some("Organization") {
                objects("Organization", &[("6", json!({"friendlyname": "Empty"}))])
            } else {
                handler(r)
            }
        },
        true,
    );
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "6"})).await;
    settle().await;
    assert_eq!(sheet(&e)["caller"]["placeholder"], "Nobody in Empty");
}

#[tokio::test(start_paused = true)]
async fn new_08_service_and_subcategory_placeholders() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    assert_eq!(
        sheet(&e)["service"]["placeholder"],
        "Choose an organization first"
    );
    assert_eq!(sheet(&e)["service"]["search"], Value::Null);
    assert_eq!(
        sheet(&e)["subcategory"]["placeholder"],
        "Choose a service first"
    );
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "4"})).await;
    assert_eq!(sheet(&e)["service"]["placeholder"], "Loading…");
    settle().await;
    assert_eq!(
        sheet(&e)["service"]["items"],
        json!([{"id": "6", "label": "Mail"}, {"id": "3", "label": "VPN"}])
    );
    send(&m, SHEET_ID, json!({"type": "setService", "id": "6"})).await;
    assert_eq!(sheet(&e)["subcategory"]["placeholder"], "Loading…");
    settle().await;
    assert_eq!(
        sheet(&e)["subcategory"]["placeholder"],
        "None for this service"
    );
    assert_eq!(sheet(&e)["subcategory"]["enabled"], true);
}

#[tokio::test(start_paused = true)]
async fn new_09_cascade_clears_children_and_ignores_late_answers() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    assert_eq!(sheet(&e)["subcategory"]["selectedId"], "12");
    send(&m, SHEET_ID, json!({"type": "setService", "id": "6"})).await;
    assert_eq!(sheet(&e)["subcategory"]["selectedId"], Value::Null);
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "5"})).await;
    let s = sheet(&e);
    assert_eq!(s["caller"]["selectedId"], Value::Null);
    assert_eq!(s["service"]["selectedId"], Value::Null);
    // Switching straight back: the answers for org 5 arrive after org 4 was chosen and are dropped.
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "4"})).await;
    settle().await;
    assert_eq!(
        sheet(&e)["caller"]["items"],
        json!([{"id": "31", "label": "Carl Caller"}])
    );
    // An id the dropdown doesn't list is refused.
    assert!(matches!(
        m.dispatch(SHEET_ID, json!({"type": "setCaller", "id": "999"}))
            .await,
        Err(AppError::InvalidIntent { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn new_10_impact_and_urgency() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    let s = sheet(&e);
    assert_eq!(
        s["impact"],
        json!({"items": [{"id": "1", "label": "A department"}, {"id": "2", "label": "A service"}, {"id": "3", "label": "A person"}],
               "selectedId": "1", "placeholder": "Choose…", "enabled": true, "search": null})
    );
    assert_eq!(s["urgency"]["selectedId"], "4");
    assert_eq!(
        s["urgency"]["items"][0],
        json!({"id": "1", "label": "Critical"})
    );
    assert_eq!(s["required"]["organization"], json!({"missing": false}));
}

#[tokio::test(start_paused = true)]
async fn new_12_assign_to_me_is_remembered_and_shared() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    assert_eq!(sheet(&e)["assignToMe"], true);
    send(
        &m,
        WINDOW_ID,
        json!({"type": "setAssignToMe", "assignToMe": false}),
    )
    .await;
    assert!(!*e.native.assign_to_me.lock().unwrap());
    assert_eq!(sheet(&e)["assignToMe"], false);
    assert_eq!(e.hub.get(WINDOW_ID)["assignToMe"], false);
}

#[tokio::test(start_paused = true)]
async fn new_13_creation_switched_off() {
    let e = env(handler);
    let m = model(&e);
    filled_sheet(&m).await;
    let s = sheet(&e);
    assert_eq!(
        s["disabledText"],
        "Creating tickets is switched off in this build, so nothing is sent to iTop."
    );
    assert_eq!(s["confirmEnabled"], false);
    assert_eq!(
        send(&m, SHEET_ID, json!({"type": "create"})).await,
        Value::Null
    );
    assert!(creates(&e).is_empty());
    assert_eq!(sheet(&e)["type"], "open");
}

#[tokio::test(start_paused = true)]
async fn new_14_first_missing_field_named() {
    let e = writing();
    let m = model(&e);
    m.request(NewTicketInstance::Sheet, NewTicketStart::default())
        .await;
    assert_eq!(sheet(&e)["missingText"], Value::Null);
    assert_eq!(
        send(&m, SHEET_ID, json!({"type": "create"})).await,
        Value::Null
    );
    let s = sheet(&e);
    assert_eq!(s["missingText"], "Fill in the organization first.");
    for f in [
        "organization",
        "caller",
        "service",
        "subcategory",
        "title",
        "description",
    ] {
        assert_eq!(s["required"][f]["missing"], true, "{f}");
    }
    send(&m, SHEET_ID, json!({"type": "setOrganization", "id": "4"})).await;
    settle().await;
    assert_eq!(sheet(&e)["missingText"], "Fill in the caller first.");
    assert_eq!(sheet(&e)["required"]["organization"]["missing"], false);
    send(&m, SHEET_ID, json!({"type": "setCaller", "id": "31"})).await;
    send(&m, SHEET_ID, json!({"type": "setService", "id": "3"})).await;
    settle().await;
    send(&m, SHEET_ID, json!({"type": "setSubcategory", "id": "12"})).await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": " \n "})).await;
    assert_eq!(sheet(&e)["missingText"], "Fill in the title first.");
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "x"})).await;
    assert_eq!(sheet(&e)["missingText"], "Fill in the description first.");
    assert!(creates(&e).is_empty());
}

#[tokio::test(start_paused = true)]
async fn new_15_16_18_19_20_create_reads_back_and_opens_it() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    send(&m, SHEET_ID, json!({"type": "setImpact", "id": "3"})).await;
    send(&m, SHEET_ID, json!({"type": "setUrgency", "id": "2"})).await;
    e.sink.take();
    assert_eq!(
        send(&m, SHEET_ID, json!({"type": "create"})).await,
        Value::Null
    );
    let c = &creates(&e)[0];
    assert_eq!(c["class"], "UserRequest");
    assert_eq!(
        c["fields"],
        json!({"org_id": "4", "caller_id": "31", "service_id": "3", "servicesubcategory_id": "12", "impact": "3", "urgency": "2",
               "title": "VPN drops", "description": "<p>Since Monday.</p>"})
    );
    assert!(
        e.oql()
            .contains(&"SELECT UserRequest WHERE id = 900".to_owned())
    );
    // The spinner state was published while creating.
    let creating: Vec<Value> = e
        .sink
        .take()
        .into_iter()
        .filter(|(id, v)| id == SHEET_ID && v["creating"] == true)
        .map(|(_, v)| v)
        .collect();
    assert_eq!(creating[0]["confirmEnabled"], false);
    assert_eq!(sheet(&e), json!({"type": "closed"}));
    assert_eq!(
        e.link.calls(),
        vec![
            LinkCall::Section("myTickets".into()),
            LinkCall::Created("R-000900".into(), true)
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn new_04_19_create_from_window_closes_it_and_honours_assign_off() {
    let e = writing();
    let m = model(&e);
    *e.native.assign_to_me.lock().unwrap() = false;
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    settle().await;
    for (t, v) in [
        ("setClass", json!({"ticketClass": "Incident"})),
        ("setOrganization", json!({"id": "4"})),
    ] {
        let mut intent = v;
        intent["type"] = json!(t);
        send(&m, WINDOW_ID, intent).await;
    }
    settle().await;
    send(&m, WINDOW_ID, json!({"type": "setCaller", "id": "31"})).await;
    send(&m, WINDOW_ID, json!({"type": "setService", "id": "3"})).await;
    settle().await;
    send(&m, WINDOW_ID, json!({"type": "setSubcategory", "id": "12"})).await;
    send(&m, WINDOW_ID, json!({"type": "setTitle", "title": "t"})).await;
    send(
        &m,
        WINDOW_ID,
        json!({"type": "setDescription", "description": "d"}),
    )
    .await;
    send(&m, WINDOW_ID, json!({"type": "create"})).await;
    assert_eq!(creates(&e)[0]["class"], "Incident");
    assert_eq!(e.native.calls().last(), Some(&Call::CloseNewTicketWindow));
    assert_eq!(e.hub.get(WINDOW_ID), json!({"type": "closed"}));
    assert_eq!(
        e.link.calls()[1],
        LinkCall::Created("I-000900".into(), false)
    );
}

#[tokio::test(start_paused = true)]
async fn new_16_17_errors_keep_the_form() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "refuse"})).await;
    let err = send(&m, SHEET_ID, json!({"type": "create"})).await;
    assert_eq!(
        err,
        "iTop didn't create the ticket. iTop returned error 100: Error: servicesubcategory_id: not allowed."
    );
    let s = sheet(&e);
    assert_eq!(s["error"], err);
    assert_eq!(s["creating"], false);
    assert_eq!(s["title"], "refuse");
    assert_eq!(s["caller"]["selectedId"], "31");
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "nokey"})).await;
    let err = send(&m, SHEET_ID, json!({"type": "create"})).await;
    assert_eq!(
        err,
        "iTop didn't create the ticket. iTop returned error 0: iTop didn't return the new ticket."
    );
    assert!(e.link.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn new_21_cancel_keeps_a_draft_dismiss_does_not() {
    let e = writing();
    let m = model(&e);
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "half"})).await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    assert_eq!(sheet(&e), json!({"type": "closed"}));
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    assert_eq!(sheet(&e)["title"], "half");
    assert_eq!(sheet(&e)["showClearForm"], true);

    // Org, title and description all empty (spaces only): no draft. A newline counts as text.
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "  "})).await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    assert_eq!(sheet(&e)["showClearForm"], false);
    send(
        &m,
        SHEET_ID,
        json!({"type": "setDescription", "description": "\n"}),
    )
    .await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    assert_eq!(sheet(&e)["showClearForm"], true);

    // The window's close button drops what was typed there and leaves the draft as it was.
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    send(
        &m,
        WINDOW_ID,
        json!({"type": "setTitle", "title": "not kept"}),
    )
    .await;
    send(&m, WINDOW_ID, json!({"type": "dismiss"})).await;
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    assert_eq!(e.hub.get(WINDOW_ID)["description"], "\n");
    assert_eq!(e.hub.get(WINDOW_ID)["title"], "");
}

#[tokio::test(start_paused = true)]
async fn new_22_plain_start_restores_the_whole_draft() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    send(
        &m,
        SHEET_ID,
        json!({"type": "setClass", "ticketClass": "Incident"}),
    )
    .await;
    send(&m, SHEET_ID, json!({"type": "setImpact", "id": "3"})).await;
    send(&m, SHEET_ID, json!({"type": "setUrgency", "id": "2"})).await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    // Shared with the menu bar's window.
    m.request(NewTicketInstance::Window, NewTicketStart::default())
        .await;
    settle().await;
    let s = e.hub.get(WINDOW_ID);
    assert_eq!(s["ticketClass"], "Incident");
    assert_eq!(s["organization"]["selectedId"], "4");
    assert_eq!(s["caller"]["selectedId"], "31");
    assert_eq!(s["service"]["selectedId"], "3");
    assert_eq!(s["subcategory"]["selectedId"], "12");
    assert_eq!(s["impact"]["selectedId"], "3");
    assert_eq!(s["urgency"]["selectedId"], "2");
    assert_eq!(s["title"], "  VPN drops  ");
    assert_eq!(s["description"], "Since Monday.");
}

#[tokio::test(start_paused = true)]
async fn new_22_org_or_ci_start_begins_fresh_and_keeps_the_draft() {
    let e = writing();
    let m = model(&e);
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "draft"})).await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    m.request(
        NewTicketInstance::Sheet,
        NewTicketStart::for_org("4", "Acme"),
    )
    .await;
    settle().await;
    assert_eq!(sheet(&e)["title"], "");
    assert_eq!(sheet(&e)["showClearForm"], true);
    send(&m, SHEET_ID, json!({"type": "dismiss"})).await;
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    assert_eq!(sheet(&e)["title"], "draft");
}

#[tokio::test(start_paused = true)]
async fn new_23_clear_form() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    send(
        &m,
        SHEET_ID,
        json!({"type": "setClass", "ticketClass": "Incident"}),
    )
    .await;
    send(&m, SHEET_ID, json!({"type": "setUrgency", "id": "1"})).await;
    send(&m, SHEET_ID, json!({"type": "cancel"})).await;
    send(&m, SHEET_ID, json!({"type": "open"})).await;
    settle().await;
    send(&m, SHEET_ID, json!({"type": "setTitle", "title": "refuse"})).await;
    send(&m, SHEET_ID, json!({"type": "create"})).await;
    assert!(sheet(&e)["error"].is_string());
    send(&m, SHEET_ID, json!({"type": "clearForm"})).await;
    settle().await;
    let s = sheet(&e);
    assert_eq!(s["showClearForm"], false);
    assert_eq!(s["ticketClass"], "Incident", "the chosen type stays");
    assert_eq!(s["organization"]["selectedId"], Value::Null);
    assert_eq!(s["caller"]["placeholder"], "Choose an organization first");
    assert_eq!(s["subcategory"]["selectedId"], Value::Null);
    assert_eq!(s["impact"]["selectedId"], "1");
    assert_eq!(s["urgency"]["selectedId"], "4");
    assert_eq!(s["title"], "");
    assert_eq!(s["error"], Value::Null);
    assert_eq!(s["missingText"], Value::Null);
    assert_eq!(s["required"]["title"]["missing"], false);
}

#[tokio::test(start_paused = true)]
async fn new_ticket_snapshot_json_shape() {
    let e = writing();
    let m = model(&e);
    filled_sheet(&m).await;
    let s = sheet(&e);
    let keys = |v: &Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&s),
        [
            "assignToMe",
            "caller",
            "confirmEnabled",
            "creating",
            "description",
            "disabledText",
            "edit",
            "error",
            "impact",
            "missingText",
            "organization",
            "required",
            "service",
            "showClearForm",
            "subcategory",
            "subtitle",
            "ticketClass",
            "title",
            "type",
            "urgency"
        ]
    );
    assert_eq!(
        keys(&s["organization"]),
        ["enabled", "items", "placeholder", "search", "selectedId"]
    );
    assert_eq!(
        keys(&s["required"]),
        [
            "caller",
            "description",
            "organization",
            "service",
            "subcategory",
            "title"
        ]
    );
    assert_eq!(s["type"], "open");
    assert_eq!(s["confirmEnabled"], true);
    assert_eq!(s["disabledText"], Value::Null);
    assert!(matches!(
        m.dispatch("newTicket:other", json!({"type": "open"})).await,
        Err(AppError::UnknownModel { .. })
    ));
}
