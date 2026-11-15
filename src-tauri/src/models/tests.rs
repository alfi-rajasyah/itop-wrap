//! Model tests: the Swift `ITopWrapTests` ported one to one, then one test per model-side
//! checklist item (named after it), then the JSON shapes against contract.ts.

use std::sync::Arc;
use std::time::Duration;

use itop_kit::{
    MemorySecretStore, Resolution, SecretStore, SessionService, SessionSettings, TicketChange,
    WriteAccess,
};
use serde_json::{Value, json};

use super::app_router::{AppConfig, AppRouter, route_workspace};
use super::format::Fmt;
use super::prefs::{MemoryPrefs, Prefs};
use super::records::{ListScope, SidebarItem};
use super::seams::ModelContext;
use super::testkit::{FakeITop, Harness, Setup, client, me, now, object, objects, ticket};
use crate::error::AppError;
use crate::hub::{Hub, RecordingSink};
use crate::native::fake::{Call, FakeNative};
use crate::router::{NativeEvent, Router};

const MINE: ListScope = ListScope::Mine;
const TEAM: ListScope = ListScope::Team;

async fn send(h: &Harness, model: &str, intent: Value) -> Result<Value, AppError> {
    let r = route_workspace(&h.ws, model, intent).await;
    h.settle().await;
    r
}

fn selected(h: &Harness, scope: ListScope) -> Option<itop_kit::Ticket> {
    let st = h.ws.lock();
    let l = &st.lists[scope.index()];
    l.ticket(l.selection.as_deref()).cloned()
}

/// List loads sent (both classes count).
fn loads(h: &Harness) -> usize {
    h.stub
        .requests()
        .iter()
        .filter(|r| {
            r.json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("agent_id =") || k.contains("team_id IN"))
        })
        .count()
}

fn detail_gets(h: &Harness) -> usize {
    h.stub
        .requests()
        .iter()
        .filter(|r| {
            r.json_data()["key"]
                .as_str()
                .is_some_and(|k| !k.starts_with("SELECT"))
                && r.operation().as_deref() == Some("core/get")
        })
        .count()
}

fn select(h: &Harness, scope: ListScope, id: Option<&str>) {
    h.ws.lock().lists[scope.index()].selection = id.map(str::to_owned);
    h.ws.publish();
}

// =============================================================================================
// Tests/ITopWrapTests/LookupRefreshTests.swift
// =============================================================================================

/// FakeITop of LookupRefreshTests: R-000601, new and unassigned until assigned; in My tickets
/// only once assigned and only if `listed`.
fn lookup_itop(listed: bool) -> FakeITop {
    let itop = FakeITop::with(vec![ticket("601", "new", "0")]);
    *itop.listed.lock().unwrap_or_else(|e| e.into_inner()) =
        Box::new(move |t: &itop_kit::Ticket, _| listed && t.agent_id == "7");
    itop
}

#[tokio::test(start_paused = true)]
async fn lookup_refresh_assigned_joins_list() {
    let h = Harness::new(lookup_itop(true));
    let current = h.itop.get("601").expect("ticket");
    h.ws.list_show(MINE, &current, Some("R-000601 is created.".into()));
    h.settle().await;
    h.ws.list_assign_to_me(MINE, &current).await;
    let shown = selected(&h, MINE).expect("shown");
    assert_eq!(shown.agent_id, "7");
    assert_eq!(shown.status, "assigned");
}

#[tokio::test(start_paused = true)]
async fn lookup_refresh_stays_outside() {
    let h = Harness::new(lookup_itop(false));
    let current = h.itop.get("601").expect("ticket");
    h.ws.list_show(MINE, &current, None);
    h.settle().await;
    h.ws.list_assign_to_me(MINE, &current).await;
    assert_eq!(
        selected(&h, MINE).expect("shown").agent_id,
        "7",
        "the open ticket is read again"
    );
}

#[tokio::test(start_paused = true)]
async fn assign_new_assigned() {
    let h = Harness::new(lookup_itop(true));
    let current = h.itop.get("601").expect("ticket");
    h.ws.list_show(MINE, &current, Some("R-000601 is created.".into()));
    h.ws.list_assign_new(MINE, &current).await;
    let st = h.ws.lock();
    assert_eq!(
        st.lists[0].notice,
        Some(super::records::Notice::ok(
            "R-000601 is created and assigned to you."
        ))
    );
    drop(st);
    assert_eq!(selected(&h, MINE).expect("shown").agent_id, "7");
}

#[tokio::test(start_paused = true)]
async fn assign_new_refused() {
    let h = Harness::new(lookup_itop(true));
    *h.itop
        .refuse_stimulus
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((100, "Not allowed".into()));
    let current = h.itop.get("601").expect("ticket");
    h.ws.list_show(MINE, &current, Some("R-000601 is created.".into()));
    h.ws.list_assign_new(MINE, &current).await;
    let st = h.ws.lock();
    let notice = st.lists[0].notice.clone().expect("notice");
    assert!(notice.is_error);
    assert!(
        notice
            .text
            .starts_with("R-000601 is created, but not assigned to you. "),
        "{}",
        notice.text
    );
    assert!(notice.text.contains("Not allowed"));
    assert_eq!(
        st.lists[0].selection.as_deref(),
        Some("UserRequest::601"),
        "the new ticket stays open"
    );
}

// =============================================================================================
// Tests/ITopWrapTests/ResolveKeepsOpenTests.swift
// =============================================================================================

fn resolution() -> TicketChange {
    TicketChange::Resolve(Resolution::new("1", "2", "fixed", "Done"))
}

async fn resolve_harness() -> Harness {
    // A second ticket keeps the list non-empty: an empty list shows its message instead (LIST-44).
    let h = Harness::new(FakeITop::with(vec![
        ticket("602", "assigned", "7"),
        ticket("609", "assigned", "7"),
    ]));
    h.load().await;
    h
}

#[tokio::test(start_paused = true)]
async fn list_30_resolved_stays_on_screen() {
    let h = resolve_harness().await;
    let t = h.itop.get("602").expect("ticket");
    select(&h, MINE, Some(&t.id()));
    assert_eq!(h.ws.list_apply(MINE, &resolution(), &t).await, None);
    let st = h.ws.lock();
    assert_eq!(st.lists[0].selection.as_deref(), Some("UserRequest::602"));
    assert_eq!(
        st.lists[0]
            .ticket(st.lists[0].selection.as_deref())
            .map(|t| t.status.clone())
            .as_deref(),
        Some("resolved")
    );
    assert_eq!(
        st.lists[0].just_changed.as_deref(),
        Some("UserRequest::602")
    );
    drop(st);
    let snap = h.snap("list:mine");
    assert_eq!(
        snap["content"]["body"]["extraSection"]["title"],
        "Resolved just now"
    );
}

#[tokio::test(start_paused = true)]
async fn list_31_close_from_there_stays_open() {
    let h = resolve_harness().await;
    let t = h.itop.get("602").expect("ticket");
    select(&h, MINE, Some(&t.id()));
    h.ws.list_apply(MINE, &resolution(), &t).await;
    let resolved = h.itop.get("602").expect("ticket");
    assert_eq!(
        h.ws.list_apply(MINE, &TicketChange::Close, &resolved).await,
        None
    );
    assert_eq!(
        selected(&h, MINE).map(|t| t.status).as_deref(),
        Some("closed")
    );
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["extraSection"]["title"],
        "Closed just now"
    );
}

#[tokio::test(start_paused = true)]
async fn list_32_resolving_an_unselected_ticket_changes_nothing() {
    let h = resolve_harness().await;
    let t = h.itop.get("602").expect("ticket");
    select(&h, MINE, None);
    h.ws.list_apply(MINE, &resolution(), &t).await;
    let st = h.ws.lock();
    assert_eq!(st.lists[0].selection, None);
    assert_eq!(st.lists[0].just_changed, None);
}

#[tokio::test(start_paused = true)]
async fn list_32_just_now_goes_at_the_first_reload_after_selecting_another() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("602", "assigned", "7"),
        ticket("603", "assigned", "7"),
    ]));
    h.load().await;
    let t = h.itop.get("602").expect("ticket");
    select(&h, MINE, Some(&t.id()));
    h.ws.list_apply(MINE, &resolution(), &t).await;
    select(&h, MINE, Some("UserRequest::603"));
    h.ws.list_refresh(MINE).await;
    assert!(h.snap("list:mine")["content"]["body"]["extraSection"].is_null());
}

// =============================================================================================
// Sign in and session (AUTH-*)
// =============================================================================================

struct App {
    router: AppRouter,
    hub: Hub,
    native: Arc<FakeNative>,
    secrets: Arc<MemorySecretStore>,
    prefs: Arc<MemoryPrefs>,
    itop: Arc<FakeITop>,
}

fn app_with(prefs: MemoryPrefs, secrets: MemorySecretStore) -> App {
    let itop = Arc::new(FakeITop::with(vec![ticket("601", "assigned", "7")]));
    let stub = itop.serve();
    let prefs = Arc::new(prefs);
    let secrets = Arc::new(secrets);
    let settings: Arc<dyn itop_kit::SettingsStore> = prefs.clone();
    let transport = stub.transport();
    let service = SessionService::with_make_client(
        secrets.clone(),
        SessionSettings::new(settings),
        WriteAccess::All,
        move |server, creds| itop_kit::ITopClient::new(server, creds, transport.clone()),
    );
    let native = Arc::new(FakeNative::new(now()));
    let hub = Hub::new(Arc::new(RecordingSink::default()));
    let router = AppRouter::new(AppConfig {
        service,
        prefs: prefs.clone(),
        hub: hub.clone(),
        native: native.clone(),
        fmt: Fmt::utc(),
        trace: false,
    });
    App {
        router,
        hub,
        native,
        secrets,
        prefs,
        itop,
    }
}

fn app() -> App {
    app_with(MemoryPrefs::new(), MemorySecretStore::new())
}

fn saved_app() -> App {
    let prefs = MemoryPrefs::new();
    use itop_kit::SettingsStore;
    prefs.set_string("server", Some("https://itop.example.com"));
    prefs.set_string("user", Some("agent"));
    let secrets = MemorySecretStore::new();
    let _ = secrets.set_password("secret", "agent@https://itop.example.com");
    app_with(prefs, secrets)
}

async fn settle() {
    for _ in 0..200 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn auth_1_starts_restoring() {
    let a = app();
    assert_eq!(a.hub.get("session"), json!({"type": "restoring"}));
}

#[tokio::test(start_paused = true)]
async fn auth_2_nothing_saved_shows_the_form_without_notice() {
    let a = app();
    a.router.native_event(NativeEvent::Launched).await;
    let s = a.hub.get("session");
    assert_eq!(s["type"], "signedOut");
    assert_eq!(s["form"]["message"], Value::Null);
    assert_eq!(s["form"]["server"], "https://");
    assert_eq!(s["form"]["initialFocus"], "server", "AUTH-12");
}

#[tokio::test(start_paused = true)]
async fn auth_3_rejected_password_notice() {
    let a = saved_app();
    *a.itop.password.lock().unwrap_or_else(|e| e.into_inner()) = "changed".into();
    a.router.native_event(NativeEvent::Launched).await;
    let s = a.hub.get("session");
    assert_eq!(s["type"], "signedOut");
    assert_eq!(
        s["form"]["message"],
        json!({"type": "notice", "text": "Your password was rejected. Sign in again."})
    );
    assert_eq!(s["form"]["initialFocus"], "password");
}

#[tokio::test(start_paused = true)]
async fn auth_4_other_restore_errors_show_unreachable() {
    let prefs = MemoryPrefs::new();
    use itop_kit::SettingsStore;
    prefs.set_string("server", Some("https://itop.example.com"));
    prefs.set_string("user", Some("agent"));
    let secrets = MemorySecretStore::new();
    let _ = secrets.set_password("secret", "agent@https://itop.example.com");
    let stub = itop_kit::stub::StubServer::new(itop_kit::stub::StubReply::failure(-1009));
    let settings: Arc<dyn itop_kit::SettingsStore> = Arc::new(prefs);
    let transport = stub.transport();
    let service = SessionService::with_make_client(
        Arc::new(secrets),
        SessionSettings::new(settings.clone()),
        WriteAccess::All,
        move |server, creds| itop_kit::ITopClient::new(server, creds, transport.clone()),
    );
    let hub = Hub::new(Arc::new(RecordingSink::default()));
    let router = AppRouter::new(AppConfig {
        service,
        prefs: Arc::new(MemoryPrefs::new()),
        hub: hub.clone(),
        native: Arc::new(FakeNative::new(now())),
        fmt: Fmt::utc(),
        trace: false,
    });
    router.native_event(NativeEvent::Launched).await;
    assert_eq!(
        hub.get("session"),
        json!({"type": "unreachable", "message": "Can't reach the server. Check your connection and the server address."})
    );
}

#[tokio::test(start_paused = true)]
async fn auth_5_another_account_keeps_the_keychain_item() {
    let a = saved_app();
    a.router
        .dispatch("session", json!({"type": "signInWithAnotherAccount"}))
        .await
        .expect("ok");
    assert_eq!(a.hub.get("session")["type"], "signedOut");
    assert_eq!(a.secrets.count(), 1);
}

#[tokio::test(start_paused = true)]
async fn auth_13_http_warning() {
    let a = app();
    a.router.native_event(NativeEvent::Launched).await;
    a.router
        .dispatch(
            "session",
            json!({"type": "setServer", "server": " HTTP://itop.lan"}),
        )
        .await
        .expect("ok");
    assert_eq!(a.hub.get("session")["form"]["httpWarning"], true);
    a.router
        .dispatch(
            "session",
            json!({"type": "setServer", "server": "https://itop.lan"}),
        )
        .await
        .expect("ok");
    assert_eq!(a.hub.get("session")["form"]["httpWarning"], false);
}

#[tokio::test(start_paused = true)]
async fn auth_15_missing_fields_send_nothing() {
    let a = app();
    a.router.native_event(NativeEvent::Launched).await;
    let r = a
        .router
        .dispatch("session", json!({"type": "signIn", "password": "x"}))
        .await
        .expect("ok");
    assert_eq!(r, json!({"ok": false, "focus": "user"}));
    assert_eq!(
        a.hub.get("session")["form"]["message"],
        json!({"type": "missingFields", "text": "Enter your username and password."})
    );
    a.router
        .dispatch("session", json!({"type": "setUser", "user": "agent"}))
        .await
        .expect("ok");
    let r = a
        .router
        .dispatch("session", json!({"type": "signIn", "password": ""}))
        .await
        .expect("ok");
    assert_eq!(r, json!({"ok": false, "focus": "password"}));
}

#[tokio::test(start_paused = true)]
async fn auth_18_invalid_credentials_focus_password() {
    let a = app();
    a.router.native_event(NativeEvent::Launched).await;
    a.router
        .dispatch(
            "session",
            json!({"type": "setServer", "server": "itop.example.com"}),
        )
        .await
        .expect("ok");
    a.router
        .dispatch("session", json!({"type": "setUser", "user": "agent"}))
        .await
        .expect("ok");
    let r = a
        .router
        .dispatch("session", json!({"type": "signIn", "password": "wrong"}))
        .await
        .expect("ok");
    assert_eq!(r, json!({"ok": false, "focus": "password"}));
    assert_eq!(
        a.hub.get("session")["form"]["message"],
        json!({"type": "error", "text": "Invalid credentials. Check your username and password."})
    );
    assert_eq!(a.secrets.count(), 0, "AUTH-20: nothing saved on failure");
}

#[test]
fn auth_17_rest_refused_box() {
    let mut s = super::session::SessionModelState::new(None, Some("jane".into()));
    s.form.error = Some(itop_kit::ITopError::RestNotAllowed);
    assert_eq!(
        serde_json::to_value(s.message(None)).ok(),
        Some(
            json!({"type": "restRefused", "bold": "This account can't use the iTop REST API.",
            "rest": " Ask your iTop admin to add the \"REST Services User\" profile to jane, then sign in again."})
        )
    );
    s.form.missing_fields = true;
    assert_eq!(
        s.message(None)
            .map(|m| matches!(m, super::session::LoginMessage::MissingFields { .. })),
        Some(true),
        "AUTH-19"
    );
}

#[test]
fn auth_12_initial_focus() {
    use super::session::{LoginField, initial_focus};
    assert_eq!(initial_focus("https://", ""), LoginField::Server);
    assert_eq!(initial_focus("https://x.org", ""), LoginField::User);
    assert_eq!(initial_focus("https://x.org", "me"), LoginField::Password);
}

#[tokio::test(start_paused = true)]
async fn auth_20_success_saves_and_shows_the_shell() {
    let a = app();
    a.router.native_event(NativeEvent::Launched).await;
    a.router
        .dispatch(
            "session",
            json!({"type": "setServer", "server": "itop.example.com/pages/UI.php"}),
        )
        .await
        .expect("ok");
    a.router
        .dispatch("session", json!({"type": "setUser", "user": " agent "}))
        .await
        .expect("ok");
    let r = a
        .router
        .dispatch("session", json!({"type": "signIn", "password": "secret"}))
        .await
        .expect("ok");
    settle().await;
    assert_eq!(r, json!({"ok": true, "focus": null}));
    assert_eq!(
        a.hub.get("session"),
        json!({"type": "signedIn", "displayName": "Agent Seven", "host": "itop.example.com"})
    );
    use itop_kit::SettingsStore;
    assert_eq!(
        a.prefs.string("server").as_deref(),
        Some("https://itop.example.com")
    );
    assert_eq!(a.prefs.string("user").as_deref(), Some("agent"));
    assert_eq!(a.secrets.count(), 1);
    assert_eq!(a.hub.get("shell")["selection"], "myTickets");
    assert!(a.native.calls().contains(&Call::SessionChanged(true)));
}

#[tokio::test(start_paused = true)]
async fn auth_30_31_sign_out_keeps_server_user_and_timer() {
    let a = saved_app();
    use itop_kit::SettingsStore;
    a.prefs.set_string("timer.running", Some(r#"{"ticketClass":"UserRequest","ticketKey":"601","ref":"R-000601","title":"T","orgID":"4","start":785000000}"#));
    a.router.native_event(NativeEvent::Launched).await;
    settle().await;
    assert_eq!(a.hub.get("session")["type"], "signedIn");
    a.router
        .dispatch("session", json!({"type": "signOut"}))
        .await
        .expect("ok");
    assert_eq!(a.hub.get("session")["type"], "signedOut");
    assert_eq!(a.hub.get("session")["form"]["message"], Value::Null);
    assert_eq!(a.secrets.count(), 0);
    assert_eq!(
        a.prefs.string("server").as_deref(),
        Some("https://itop.example.com")
    );
    assert!(
        a.prefs.string("timer.running").is_some(),
        "AUTH-31: the timer stays saved"
    );
    assert_eq!(
        a.hub.get("list:mine"),
        Value::Null,
        "workspace snapshots go"
    );
    assert!(a.native.calls().contains(&Call::SessionChanged(false)));
}

#[tokio::test(start_paused = true)]
async fn contract_7_errors() {
    let a = app();
    let e = a
        .router
        .dispatch("list:mine", json!({"type": "refresh"}))
        .await;
    assert_eq!(e, Err(AppError::not_signed_in()));
    let e = a.router.dispatch("nope", json!({})).await;
    assert!(matches!(e, Err(AppError::UnknownModel { .. })));
    let e = a.router.dispatch("session", json!({"type": "bogus"})).await;
    assert!(matches!(e, Err(AppError::InvalidIntent { .. })));
}

// =============================================================================================
// Shell (SHELL-*)
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn shell_3_4_sidebar_titles_and_need_count() {
    let mut esc = ticket("610", "escalated_ttr", "7");
    esc.priority = 1;
    let h = Harness::new(FakeITop::with(vec![esc, ticket("611", "assigned", "7")]));
    h.load().await;
    let s = h.snap("shell");
    assert_eq!(
        s["sidebar"],
        json!([
            {"item": "myTickets", "title": "My tickets", "count": 1},
            {"item": "teamTickets", "title": "Team tickets", "count": null},
            {"item": "configItems", "title": "Config items", "count": null},
            {"item": "organizations", "title": "Organizations", "count": null},
            {"item": "people", "title": "People", "count": null},
        ])
    );
    assert_eq!(s["needCount"], 1);
    assert_eq!(s["contentTitle"], "My tickets", "SHELL-5");
    assert_eq!(s["selection"], "myTickets", "SHELL-6");
    assert_eq!(
        s["account"],
        json!({"initials": "AS", "displayName": "Agent Seven", "host": "itop.example.com"}),
        "AUTH-27"
    );
    assert_eq!(
        s["signOutConfirm"],
        json!({"title": "Sign out of itop.example.com?", "message": "Your password is removed from the Keychain. Your username stays filled in."}),
        "AUTH-29"
    );
}

#[tokio::test(start_paused = true)]
async fn shell_7_detail_placeholder() {
    let h = Harness::new(FakeITop::default());
    assert_eq!(
        h.snap("detail:mine"),
        json!({"type": "none", "placeholder": "Select a ticket to see its details."})
    );
}

#[tokio::test(start_paused = true)]
async fn shell_9_open_ticket_in_the_list_that_has_it() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("601", "assigned", "7"),
        ticket("700", "assigned", "9"),
    ]));
    h.load().await;
    h.ws.list_refresh(TEAM).await;
    h.ws.set_query(MINE, "zzz");
    send(&h, "shell", json!({"type": "openTicket", "ticket": {"ticketId": "UserRequest::601", "ref": "R-000601"}})).await.expect("ok");
    let l = h.snap("list:mine");
    assert_eq!(l["query"], "");
    assert_eq!(l["selection"], "UserRequest::601");
    assert_eq!(h.snap("shell")["selection"], "myTickets");
    send(&h, "shell", json!({"type": "openTicket", "ticket": {"ticketId": "UserRequest::700", "ref": "R-000700"}})).await.expect("ok");
    assert_eq!(h.snap("shell")["selection"], "teamTickets");
    assert_eq!(h.snap("list:team")["selection"], "UserRequest::700");
}

#[tokio::test(start_paused = true)]
async fn shell_9_unknown_ticket_becomes_a_search_without_selecting() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("601", "assigned", "7"),
        ticket("512", "new", "9"),
    ]));
    h.load().await;
    send(&h, "shell", json!({"type": "select", "item": "people"}))
        .await
        .expect("ok");
    send(&h, "shell", json!({"type": "openTicket", "ticket": {"ticketId": "UserRequest::512", "ref": "R-000512"}})).await.expect("ok");
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.settle().await;
    let l = h.snap("list:mine");
    assert_eq!(l["query"], "R-000512");
    assert_eq!(l["selection"], Value::Null);
    assert_eq!(
        l["content"]["body"]["extraSection"]["title"],
        "Search result: not in your queue"
    );
    assert_eq!(h.snap("shell")["selection"], "myTickets");
}

#[tokio::test(start_paused = true)]
async fn shell_10_to_14_routing_into_the_standalone_models() {
    let h = Harness::new(FakeITop::default());
    send(
        &h,
        "shell",
        json!({"type": "openOrg", "orgId": "4", "orgName": "Org A"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("shell")["selection"], "organizations");
    assert_eq!(h.hub.get("orgs")["selectedId"], "4", "SHELL-10");
    send(
        &h,
        "shell",
        json!({"type": "openCI", "ci": {"key": "9", "className": "Server", "name": "srv"}}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("shell")["selection"], "configItems");
    assert_eq!(h.hub.get("cmdb")["current"]["name"], "srv", "SHELL-11");
    send(
        &h,
        "shell",
        json!({"type": "openOrgPeople", "orgId": "4", "orgName": "Org A"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("shell")["selection"], "people");
    assert_eq!(
        h.hub.get("people")["scope"],
        json!({"orgId": "4", "orgName": "Org A"}),
        "SHELL-12"
    );
    send(
        &h,
        "shell",
        json!({"type": "openOrgConfigItems", "orgId": "4", "orgName": "Org A"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.hub.get("cmdb")["scope"],
        json!({"orgId": "4", "orgName": "Org A"})
    );
    send(
        &h,
        "shell",
        json!({"type": "newTicketForOrg", "orgId": "4", "orgName": "Org A"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.hub.get("newTicket:sheet")["type"], "open", "SHELL-14");
    assert_eq!(h.hub.get("newTicket:window")["type"], "closed");
    send(&h, "menubar", json!({"type": "newTicket"}))
        .await
        .expect("ok");
    assert_eq!(h.hub.get("newTicket:window")["type"], "open", "NEW-2");
    assert!(h.native.calls().contains(&Call::ShowNewTicketWindow));
}

#[tokio::test(start_paused = true)]
async fn new_19_ticket_created_shows_in_my_tickets() {
    let h = Harness::new(FakeITop::with(vec![ticket("650", "new", "0")]));
    h.load().await;
    send(&h, "shell", json!({"type": "select", "item": "people"}))
        .await
        .expect("ok");
    h.ws.created(h.itop.get("650").expect("t"), false).await;
    h.settle().await;
    assert_eq!(h.snap("shell")["selection"], "myTickets");
    let list = h.snap("list:mine");
    assert_eq!(list["query"], "R-000650");
    assert_eq!(list["selection"], "UserRequest::650");
    assert_eq!(
        list["content"]["body"]["notice"],
        json!({"text": "R-000650 is created.", "isError": false})
    );
}

// =============================================================================================
// Ticket lists (LIST-*)
// =============================================================================================

fn org_ticket(key: &str, org: &str, org_id: &str) -> itop_kit::Ticket {
    let mut t = ticket(key, "assigned", "7");
    t.org_name = org.into();
    t.org_id = org_id.into();
    t.title = format!("VPN {key}");
    t
}

#[tokio::test(start_paused = true)]
async fn list_10_need_count_ignores_org_filter_and_search() {
    let mut a = ticket("1", "escalated_ttr", "7");
    a.org_name = "A".into();
    let mut b = ticket("2", "escalated_ttr", "7");
    b.org_name = "B".into();
    let h = Harness::new(FakeITop::with(vec![a, b, ticket("3", "resolved", "7")]));
    h.load().await;
    send(&h, "list:mine", json!({"type": "toggleOrg", "name": "A"}))
        .await
        .expect("ok");
    send(&h, "list:mine", json!({"type": "setQuery", "query": "zzz"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("list:mine")["needCount"], 2);
}

#[tokio::test(start_paused = true)]
async fn list_17_33_to_37_org_filter() {
    let h = Harness::new(FakeITop::with(vec![
        org_ticket("1", "Beta", "5"),
        org_ticket("2", "Alpha", "4"),
        org_ticket("3", "Alpha", "0"),
    ]));
    h.load().await;
    let f = &h.snap("list:mine")["orgFilter"];
    assert_eq!(f["visible"], true);
    assert_eq!(f["label"], "All organizations");
    assert_eq!(f["help"], "Filter by organization");
    assert_eq!(f["accessibilityLabel"], "Filter by organization: all");
    assert_eq!(
        f["choices"],
        json!([{"name": "Alpha", "selected": false}, {"name": "Beta", "selected": false}])
    );
    send(
        &h,
        "list:mine",
        json!({"type": "toggleOrg", "name": "Alpha"}),
    )
    .await
    .expect("ok");
    let s = h.snap("list:mine");
    assert_eq!(s["orgFilter"]["label"], "Alpha");
    assert_eq!(s["orgFilter"]["help"], "Showing tickets for Alpha");
    assert_eq!(
        s["content"]["body"]["sections"][0]["rows"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    send(
        &h,
        "list:mine",
        json!({"type": "toggleOrg", "name": "Alpha"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("list:mine")["orgFilter"]["selected"],
        Value::Null,
        "LIST-35: picking it again turns it off"
    );
    assert_eq!(
        h.snap("list:team")["orgFilter"]["visible"],
        false,
        "Team tickets never has it"
    );
}

#[tokio::test(start_paused = true)]
async fn list_37_none_for_the_org_with_see_all() {
    let h = Harness::new(FakeITop::with(vec![
        org_ticket("1", "Beta", "5"),
        org_ticket("2", "Alpha", "4"),
    ]));
    h.load().await;
    h.ws.lock().lists[0].org_filter = Some("Gamma".into());
    let body = &h.snap("list:mine")["content"]["body"];
    assert_eq!(
        body["orgNoMatch"],
        json!({"text": "None of your tickets are for Gamma.", "seeAll": null})
    );
    h.ws.lock().lists[0].query = "nomatch".into();
    assert!(
        h.snap("list:mine")["content"]["body"]["orgNoMatch"].is_null(),
        "not while searching"
    );
}

#[tokio::test(start_paused = true)]
async fn list_18_no_match_text() {
    let h = Harness::new(FakeITop::with(vec![org_ticket("1", "A", "4")]));
    h.load().await;
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": " printer "}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["noMatchText"],
        "No tickets in your list match “printer”."
    );
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "vpn 1"}),
    )
    .await
    .expect("ok");
    let body = &h.snap("list:mine")["content"]["body"];
    assert!(body["noMatchText"].is_null());
    assert_eq!(
        body["sections"][0]["rows"][0]["ticket"]["ref"], "R-000001",
        "LIST-16"
    );
}

#[tokio::test(start_paused = true)]
async fn list_19_44_empty_messages() {
    let h = Harness::new(FakeITop::default());
    h.load().await;
    assert_eq!(
        h.snap("list:mine")["content"],
        json!({"type": "message", "text": "Nothing waiting on you. New assignments show up here."})
    );
    h.ws.list_refresh(TEAM).await;
    assert_eq!(
        h.snap("list:team")["content"],
        json!({"type": "message", "text": "Your teams have no open tickets."})
    );
    send(&h, "list:mine", json!({"type": "setQuery", "query": "x"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("list:mine")["content"]["type"],
        "list",
        "LIST-19: searching never shows the empty message"
    );
    h.ws.lock().options.range = 7;
    assert_eq!(
        h.snap("list:team")["content"],
        json!({"type": "message", "text": "No tickets in this range. Try a longer range in the View menu."})
    );
}

#[tokio::test(start_paused = true)]
async fn list_20_partial_refs_and_digits_are_never_looked_up() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    for q in ["412", "R-0004", "000512"] {
        send(&h, "list:mine", json!({"type": "setQuery", "query": q}))
            .await
            .expect("ok");
        tokio::time::sleep(Duration::from_millis(300)).await;
        h.settle().await;
        assert!(
            h.snap("list:mine")["content"]["body"]["lookupBar"].is_null(),
            "{q}"
        );
    }
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "r-000001"}),
    )
    .await
    .expect("ok");
    assert!(
        h.snap("list:mine")["content"]["body"]["lookupBar"].is_null(),
        "a loaded ref filters locally"
    );
    assert_eq!(loads(&h), 2, "the first load only");
    assert_eq!(
        h.stub
            .requests()
            .iter()
            .filter(|r| r.json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("ref =")))
            .count(),
        0,
        "no lookup"
    );
}

#[tokio::test(start_paused = true)]
async fn list_22_to_27_full_ref_lookup() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "assigned", "7"),
        ticket("512", "assigned", "9"),
    ]));
    h.load().await;
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": " r-000512 "}),
    )
    .await
    .expect("ok");
    let body = &h.snap("list:mine")["content"]["body"];
    assert_eq!(
        body["lookupBar"],
        json!({"type": "searching", "text": "Looking up R-000512 in iTop…"}),
        "LIST-24"
    );
    assert_eq!(body["showNotices"], false, "LIST-69");
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.settle().await;
    let body = &h.snap("list:mine")["content"]["body"];
    assert!(body["lookupBar"].is_null());
    assert_eq!(
        body["extraSection"]["title"],
        "Search result: not in your queue"
    );
    assert_eq!(
        body["extraSection"]["row"]["agent"],
        json!({"type": "name", "name": "Agent 9"}),
        "LIST-27: agent always shown"
    );
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "R-000999"}),
    )
    .await
    .expect("ok");
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.settle().await;
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["lookupBar"],
        json!({"type": "missing", "text": "R-000999 doesn't exist, or your iTop account can't see it."}),
        "LIST-25"
    );
    send(&h, "list:mine", json!({"type": "setQuery", "query": "vpn"}))
        .await
        .expect("ok");
    assert!(
        h.snap("list:mine")["content"]["body"]["lookupBar"].is_null(),
        "LIST-28"
    );
}

#[tokio::test(start_paused = true)]
async fn list_22_a_new_keystroke_cancels_the_wait() {
    let h = Harness::new(FakeITop::with(vec![ticket("512", "assigned", "9")]));
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| false);
    h.load().await;
    let before = h.calls("core/get", Some("UserRequest"));
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "R-000512"}),
    )
    .await
    .expect("ok");
    tokio::time::sleep(Duration::from_millis(100)).await;
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "R-00051"}),
    )
    .await
    .expect("ok");
    tokio::time::sleep(Duration::from_millis(400)).await;
    h.settle().await;
    assert_eq!(
        h.calls("core/get", Some("UserRequest")),
        before,
        "no lookup went out"
    );
}

#[tokio::test(start_paused = true)]
async fn list_26_lookup_failure() {
    let h = Harness::new(FakeITop::default());
    h.load().await;
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = true;
    h.ws.lock().lists[0]
        .tickets
        .push(ticket("1", "assigned", "7"));
    // The lookup query isn't a list query, so make the stub fail every UserRequest read.
    let stub = itop_kit::stub::StubServer::new(itop_kit::stub::StubReply::failure(-1009));
    let ws = super::workspace::Workspace::new(
        ModelContext {
            session: Arc::new(itop_kit::Session {
                client: client(&stub, true),
                user: me(),
            }),
            hub: h.hub.clone(),
            native: h.native.clone(),
        },
        Fmt::utc(),
        h.prefs.clone(),
        false,
    );
    ws.lock().lists[0].phase = super::list::Phase::Loaded;
    ws.lock().lists[0]
        .tickets
        .push(ticket("1", "assigned", "7"));
    route_workspace(
        &ws,
        "list:mine",
        json!({"type": "setQuery", "query": "R-000777"}),
    )
    .await
    .expect("ok");
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.settle().await;
    ws.publish();
    assert_eq!(
        h.hub.get("list:mine")["content"]["body"]["lookupBar"],
        json!({"type": "failed", "text": "Couldn't look that up: Can't reach itop.example.com. Check your connection, then retry."})
    );
}

#[tokio::test(start_paused = true)]
async fn list_29_found_ticket_is_dropped_once_the_list_has_it() {
    let h = Harness::new(FakeITop::with(vec![ticket("512", "new", "0")]));
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|t, _| t.agent_id == "7");
    h.load().await;
    send(
        &h,
        "list:mine",
        json!({"type": "setQuery", "query": "R-000512"}),
    )
    .await
    .expect("ok");
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.settle().await;
    h.itop.set("512", |t| t.title = "Renamed".into());
    h.ws.list_refresh(MINE).await;
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["extraSection"]["row"]["ticket"]["title"],
        "Renamed",
        "read again"
    );
    h.itop.set("512", |t| t.agent_id = "7".into());
    h.ws.list_refresh(MINE).await;
    assert!(
        h.snap("list:mine")["content"]["body"]["extraSection"].is_null(),
        "the list's copy wins"
    );
}

#[tokio::test(start_paused = true)]
async fn list_45_controls_and_63_no_teams() {
    let h = Harness::new(FakeITop::default());
    assert_eq!(h.snap("list:mine")["showControls"], false);
    assert_eq!(
        h.snap("list:mine")["content"],
        json!({"type": "loading"}),
        "LIST-60"
    );
    h.load().await;
    assert_eq!(h.snap("list:mine")["showControls"], true, "even when empty");
    let mut user = me();
    user.teams.clear();
    let h = Harness::with(
        FakeITop::default(),
        Setup {
            user,
            ..Setup::default()
        },
    );
    h.ws.list_refresh(TEAM).await;
    let s = h.snap("list:team");
    assert_eq!(s["showControls"], false);
    assert_eq!(
        s["content"],
        json!({"type": "message", "text": "You're not in any iTop team, so there are no team tickets to show."})
    );
}

#[tokio::test(start_paused = true)]
async fn list_46_range_change_reloads_the_shown_list_and_goes_to_page_1() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    h.ws.lock().lists[0].page = 3;
    let before = h.calls("core/get", Some("UserRequest"));
    send(&h, "options", json!({"type": "setRange", "range": 30}))
        .await
        .expect("ok");
    assert_eq!(h.ws.lock().lists[0].page, 1);
    assert!(
        h.calls("core/get", Some("UserRequest")) > before,
        "reloaded"
    );
    assert_eq!(h.prefs.int("list.range"), Some(30), "PERS-9");
    h.ws.lock().lists[0].page = 2;
    let before = h.calls("core/get", Some("UserRequest"));
    send(&h, "options", json!({"type": "setSort", "sort": "newest"}))
        .await
        .expect("ok");
    assert_eq!(h.ws.lock().lists[0].page, 1);
    assert_eq!(
        h.calls("core/get", Some("UserRequest")),
        before,
        "sort doesn't reload"
    );
    send(
        &h,
        "options",
        json!({"type": "setPageSize", "pageSize": null}),
    )
    .await
    .expect("ok");
    assert_eq!(h.prefs.int("list.pageSize"), Some(0));
    send(
        &h,
        "options",
        json!({"type": "setIncludeClosed", "includeClosed": true}),
    )
    .await
    .expect("ok");
    assert_eq!(h.prefs.bool("list.includeClosed"), Some(true));
    send(&h, "options", json!({"type": "reset"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("options")["isDefault"], true, "LIST-41");
    assert!(matches!(
        send(&h, "options", json!({"type": "setRange", "range": 5})).await,
        Err(AppError::InvalidIntent { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn list_46_range_change_elsewhere_reloads_nothing() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    send(
        &h,
        "shell",
        json!({"type": "select", "item": "configItems"}),
    )
    .await
    .expect("ok");
    let before = h.calls("core/get", Some("UserRequest"));
    send(&h, "options", json!({"type": "setRange", "range": 7}))
        .await
        .expect("ok");
    assert_eq!(h.calls("core/get", Some("UserRequest")), before);
}

#[tokio::test(start_paused = true)]
async fn list_49_pager() {
    let tickets: Vec<_> = (1..=25)
        .map(|k| ticket(&k.to_string(), "assigned", "7"))
        .collect();
    let h = Harness::new(FakeITop::with(tickets));
    h.load().await;
    let s = h.snap("list:mine");
    assert_eq!(
        s["pager"],
        json!({"type": "pages", "page": 1, "pageCount": 2, "label": "1–20 of 25", "canPrevious": false, "canNext": true})
    );
    assert_eq!(
        s["content"]["body"]["sections"][0]["total"], 25,
        "LIST-13: the whole group's count"
    );
    send(&h, "list:mine", json!({"type": "setPage", "page": 9}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("list:mine")["pager"]["label"],
        "21–25 of 25",
        "LIST-50"
    );
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    assert_eq!(
        h.snap("list:mine")["pager"],
        json!({"type": "count", "text": "1 ticket"})
    );
}

#[tokio::test(start_paused = true)]
async fn list_52_to_58_rows() {
    let mut p1 = ticket("1", "new", "0");
    p1.team_id = "3".into();
    p1.priority = 1;
    p1.start_date = Some(now() - chrono::Duration::hours(2));
    let mut p2 = ticket("2", "assigned", "9");
    p2.priority = 2;
    let h = Harness::new(FakeITop::with(vec![p1, p2]));
    h.ws.list_refresh(TEAM).await;
    let rows = &h.snap("list:team")["content"]["body"]["sections"];
    let new_row = &rows[0]["rows"][0];
    assert_eq!(new_row["age"], "2h");
    assert_eq!(
        new_row["openedTooltip"],
        "Opened Nov 16, 2026 at 1:00\u{202F}PM"
    );
    assert_eq!(new_row["priorityMark"], "p1");
    assert_eq!(new_row["agent"], json!({"type": "assignToMe"}));
    let assigned_row = &rows[1]["rows"][0];
    assert_eq!(assigned_row["priorityMark"], "p2");
    assert_eq!(
        assigned_row["agent"],
        json!({"type": "name", "name": "Agent 9"})
    );
    assert_eq!(assigned_row["age"], Value::Null);
    let mut t = ticket("1", "new", "0");
    t.team_id = "3".into();
    let h = Harness::with(
        FakeITop::with(vec![t]),
        Setup {
            writes: false,
            ..Setup::default()
        },
    );
    h.ws.list_refresh(TEAM).await;
    assert_eq!(
        h.snap("list:team")["content"]["body"]["sections"][0]["rows"][0]["agent"],
        json!({"type": "unassigned"}),
        "DBG-5: no pill with writes off"
    );
}

#[tokio::test(start_paused = true)]
async fn list_58_row_pill_selects_and_assigns() {
    let mut t = ticket("1", "new", "0");
    t.team_id = "3".into();
    let h = Harness::new(FakeITop::with(vec![t]));
    h.ws.list_refresh(TEAM).await;
    send(
        &h,
        "list:team",
        json!({"type": "rowAssignToMe", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("list:team");
    assert_eq!(s["selection"], "UserRequest::1");
    assert_eq!(
        s["content"]["body"]["notice"],
        json!({"text": "R-000001 is assigned to you.", "isError": false}),
        "ASG-4"
    );
    assert_eq!(h.itop.get("1").map(|t| t.agent_id).as_deref(), Some("7"));
}

#[tokio::test(start_paused = true)]
async fn list_61_62_failures() {
    let h = Harness::new(FakeITop::default());
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = true;
    h.load().await;
    assert_eq!(
        h.snap("list:mine")["content"],
        json!({"type": "failed", "message": "Can't reach itop.example.com. Check your connection, then retry."})
    );
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = false;
    h.itop
        .tickets
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(ticket("1", "assigned", "7"));
    h.load().await;
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = true;
    h.load().await;
    let s = h.snap("list:mine");
    assert_eq!(
        s["content"]["body"]["refreshError"],
        "Couldn't refresh: Can't reach itop.example.com. Check your connection, then retry."
    );
    assert_eq!(
        s["content"]["body"]["sections"][0]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the list stays"
    );
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = false;
    h.load().await;
    assert!(h.snap("list:mine")["content"]["body"]["refreshError"].is_null());
}

#[tokio::test(start_paused = true)]
async fn list_65_a_reload_asked_during_one_runs_once_after() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.ws.lock().lists[0].refreshing = true;
    h.ws.list_refresh(MINE).await;
    h.ws.list_refresh(MINE).await;
    assert!(h.ws.lock().lists[0].refresh_again);
    h.ws.lock().lists[0].refreshing = false;
    let before = loads(&h);
    h.ws.list_refresh(MINE).await;
    h.settle().await;
    assert_eq!(
        loads(&h) - before,
        4,
        "one reload plus exactly one more (two classes each)"
    );
}

#[tokio::test(start_paused = true)]
async fn list_66_stale_selection_is_cleared() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.itop.set("1", |t| t.agent_id = "9".into());
    h.load().await;
    assert_eq!(h.snap("list:mine")["selection"], Value::Null);
}

#[test]
fn list_68_updated_text() {
    let mut l = super::list::ListState::new(MINE);
    assert_eq!(super::list::updated_text(&l, now()), "Loading");
    l.phase = super::list::Phase::Failed("x".into());
    assert_eq!(super::list::updated_text(&l, now()), "Not updated yet");
    l.updated_at = Some(now() - chrono::Duration::seconds(59));
    assert_eq!(super::list::updated_text(&l, now()), "Updated just now");
    l.updated_at = Some(now() - chrono::Duration::minutes(3));
    assert_eq!(super::list::updated_text(&l, now()), "Updated 3 min ago");
}

#[tokio::test(start_paused = true)]
async fn list_68_footer_spinner_only_after_the_first_load() {
    let h = Harness::new(FakeITop::default());
    h.ws.lock().lists[0].refreshing = true;
    assert_eq!(h.snap("list:mine")["footerSpinner"], false);
    h.ws.lock().lists[0].phase = super::list::Phase::Loaded;
    let s = h.snap("list:mine");
    assert_eq!(s["footerSpinner"], true);
    assert_eq!(s["refreshing"], true, "LIST-64: Refresh disabled");
}

// =============================================================================================
// Ticket detail (DET-*) and composer (LOG-*)
// =============================================================================================

async fn detail_harness(t: itop_kit::Ticket, writes: bool) -> Harness {
    let id = t.id();
    let h = Harness::with(
        FakeITop::with(vec![t]),
        Setup {
            writes,
            ..Setup::default()
        },
    );
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
    h.ws.lock().options.include_closed = true;
    h.load().await;
    select(&h, MINE, Some(&id));
    h.settle().await;
    h
}

#[tokio::test(start_paused = true)]
async fn det_1_to_6_loads_and_shows() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    let d = h.snap("detail:mine");
    assert_eq!(d["type"], "ticket");
    assert_eq!(d["classLabel"], "User request");
    assert_eq!(d["body"]["type"], "loaded");
    assert_eq!(
        d["body"]["description"]["blocks"][0]["runs"][0]["text"],
        "Printer jams"
    );
    assert_eq!(d["body"]["serviceLine"], Value::Null);
    assert_eq!(
        d["body"]["emptyLogText"], "No public entries yet.",
        "DET-36"
    );
    assert_eq!(
        d["attachments"],
        json!({"type": "loaded", "items": [], "downloadingId": null, "saved": null, "error": null})
    );
    assert!(d["composer"].is_object(), "LOG-1");
}

#[tokio::test(start_paused = true)]
async fn det_2_reloads_when_the_list_copy_changes() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    let gets = detail_gets(&h);
    h.load().await;
    assert_eq!(
        detail_gets(&h),
        gets,
        "a reload with no change: no detail read"
    );
    h.itop.set("1", |t| t.status = "pending".into());
    h.load().await;
    assert_eq!(detail_gets(&h), gets + 1, "the detail reloads");
}

#[tokio::test(start_paused = true)]
async fn det_5_first_load_failure_then_retry() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    // Make the detail read return nothing.
    h.itop
        .tickets
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter_mut()
        .for_each(|t| t.ticket_class = itop_kit::TicketClass::Incident);
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    let d = h.snap("detail:mine");
    assert_eq!(
        d["body"]["text"],
        "Couldn't load this ticket. iTop returned error 0: R-000001 wasn't returned. It may have been deleted, or your account can't see it any more."
    );
    assert!(d["composer"].is_null());
    h.itop
        .tickets
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter_mut()
        .for_each(|t| t.ticket_class = itop_kit::TicketClass::UserRequest);
    send(&h, "detail:mine", json!({"type": "retry"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("detail:mine")["body"]["type"], "loaded");
}

#[tokio::test(start_paused = true)]
async fn det_7_10_copy_ref_for_two_seconds() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    send(&h, "detail:mine", json!({"type": "copyRef"}))
        .await
        .expect("ok");
    assert!(
        h.native
            .calls()
            .contains(&Call::CopyText("R-000001".into()))
    );
    assert_eq!(h.snap("detail:mine")["copied"], true);
    tokio::time::sleep(Duration::from_millis(2100)).await;
    h.settle().await;
    assert_eq!(h.snap("detail:mine")["copied"], false);
}

#[tokio::test(start_paused = true)]
async fn det_12_to_17_primary_action_by_status() {
    let cases = [
        (
            "new",
            "0",
            json!({"type": "action", "kind": "assignToMe", "label": "Assign to me", "help": "Assign R-000001 to you", "teal": false, "enabled": true, "busy": false}),
        ),
        (
            "new",
            "9",
            json!({"type": "action", "kind": "openAssign", "label": "Assign ticket", "help": "Choose a team and agent", "teal": false, "enabled": true, "busy": false}),
        ),
        (
            "escalated_ttr",
            "7",
            json!({"type": "action", "kind": "openResolve", "label": "Resolve ticket", "help": "Resolve R-000001", "teal": true, "enabled": true, "busy": false}),
        ),
        (
            "resolved",
            "7",
            json!({"type": "action", "kind": "confirmClose", "label": "Close ticket", "help": "Close R-000001", "teal": false, "enabled": true, "busy": false}),
        ),
        ("closed", "7", json!({"type": "none"})),
        ("waiting_for_approval", "7", json!({"type": "none"})),
    ];
    for (status, agent, want) in cases {
        let h = Harness::new(FakeITop::with(vec![ticket("1", status, agent)]));
        *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
        h.ws.lock().options.include_closed = true;
        h.load().await;
        select(&h, MINE, Some("UserRequest::1"));
        h.settle().await;
        assert_eq!(h.snap("detail:mine")["primary"], want, "{status}");
    }
}

#[tokio::test(start_paused = true)]
async fn det_16_close_confirm_texts() {
    let h = detail_harness(ticket("1", "resolved", "7"), true).await;
    assert_eq!(
        h.snap("detail:mine")["closeConfirm"],
        json!({"title": "Close R-000001?", "message": "A closed ticket can't be reopened from the app."})
    );
}

#[tokio::test(start_paused = true)]
async fn det_18_dbg_5_writes_off() {
    let h = detail_harness(ticket("1", "assigned", "7"), false).await;
    let d = h.snap("detail:mine");
    assert_eq!(d["primary"]["enabled"], false);
    assert_eq!(
        d["primary"]["help"],
        "Changes to iTop are switched off in this build"
    );
    assert_eq!(d["more"]["enabled"], false);
    let c = &d["composer"];
    assert_eq!(c["editable"], false, "LOG-10");
    assert_eq!(
        c["sendHelp"],
        "Changes to iTop are switched off in this build"
    );
    assert_eq!(c["paperclipHelp"], "Adding images is switched off");
    assert_eq!(
        c["status"],
        json!({"text": "Adding entries is switched off in this build, so nothing is sent to iTop.", "isError": false})
    );
    assert_eq!(
        d["timerButton"],
        json!({"type": "start", "enabled": false, "help": "Changes to iTop are switched off in this build"})
    );
}

#[tokio::test(start_paused = true)]
async fn det_20_to_24_more_menu() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    assert_eq!(
        h.snap("detail:mine")["more"],
        json!({"assign": {"label": "Reassign…"}, "pending": true, "reopen": false, "enabled": true, "divider": true,
            "webUrl": "https://itop.example.com/pages/UI.php?operation=details&class=UserRequest&id=1"})
    );
    let h = detail_harness(ticket("1", "new", "9"), true).await;
    assert!(
        h.snap("detail:mine")["more"]["assign"].is_null(),
        "primary is Assign and it has an agent"
    );
    let h = detail_harness(ticket("1", "new", "0"), true).await;
    assert_eq!(
        h.snap("detail:mine")["more"]["assign"]["label"],
        "Assign to someone else…"
    );
}

#[tokio::test(start_paused = true)]
async fn det_25_to_28_strip() {
    let mut t = ticket("1", "assigned", "9");
    t.caller_name = String::new();
    t.team_name = String::new();
    let h = detail_harness(t, true).await;
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.ws.lock()
        .timer
        .totals
        .insert("UserRequest::1".into(), 4800);
    h.itop.detail.lock().unwrap_or_else(|e| e.into_inner())["functionalcis_list"] = json!([{"functionalci_id": 12, "functionalci_id_friendlyname": "vpn-gw-01", "functionalci_id_finalclass_recall": "NetworkDevice"}]);
    send(&h, "detail:mine", json!({"type": "retry"}))
        .await
        .expect("ok");
    let strip = &h.snap("detail:mine")["strip"];
    assert_eq!(
        strip,
        &json!({"caller": "None", "organization": "Org A", "team": "No team",
            "agent": {"type": "person", "initials": "A9", "name": "Agent 9"}, "spent": "1h 20m spent",
            "linkedCIs": [{"key": "12", "className": "NetworkDevice", "name": "vpn-gw-01"}]})
    );
}

#[tokio::test(start_paused = true)]
async fn det_30_31_service_line_and_no_description() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    *h.itop.detail.lock().unwrap_or_else(|e| e.into_inner()) = json!({
        "description": "", "service_id": "5", "service_id_friendlyname": "Network access",
        "servicesubcategory_id": "6", "servicesubcategory_id_friendlyname": "VPN",
    });
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    let body = &h.snap("detail:mine")["body"];
    assert_eq!(body["serviceLine"], "Network access › VPN");
    assert_eq!(body["description"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn det_33_34_attachments_download() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can("Attachment", objects(vec![object("Attachment", "3", json!({"friendlyname": "vpn.log", "contents": {"data": "aGVsbG8=", "filename": "vpn.log", "mimetype": "text/plain"}}))]));
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    assert_eq!(
        h.snap("detail:mine")["attachments"]["items"],
        json!([{"id": "3", "name": "vpn.log"}])
    );
    send(
        &h,
        "detail:mine",
        json!({"type": "downloadAttachment", "attachmentId": "3"}),
    )
    .await
    .expect("ok");
    assert!(
        h.native
            .calls()
            .contains(&Call::SavePanel("vpn.log".into()))
    );
    assert_eq!(
        h.snap("detail:mine")["attachments"]["saved"],
        Value::Null,
        "cancelled: nothing"
    );
    let dir = std::env::temp_dir().join(format!("itopwrap-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("vpn.log");
    *h.native.save_to.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
    send(
        &h,
        "detail:mine",
        json!({"type": "downloadAttachment", "attachmentId": "3"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("detail:mine")["attachments"]["saved"],
        "Saved vpn.log (5 bytes)."
    );
    assert_eq!(std::fs::read(&path).ok(), Some(b"hello".to_vec()));
    let _ = std::fs::remove_dir_all(&dir);
    *h.native.save_to.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(std::path::PathBuf::from("/nonexistent-dir/x/y"));
    send(
        &h,
        "detail:mine",
        json!({"type": "downloadAttachment", "attachmentId": "3"}),
    )
    .await
    .expect("ok");
    let err = h.snap("detail:mine")["attachments"]["error"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(err.starts_with("Couldn't download vpn.log. "), "{err}");
    assert!(matches!(
        send(
            &h,
            "detail:mine",
            json!({"type": "downloadAttachment", "attachmentId": "99"})
        )
        .await,
        Err(AppError::InvalidIntent { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn det_35_tabs_reset_per_ticket() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "assigned", "7"),
        ticket("2", "assigned", "7"),
    ]));
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    send(
        &h,
        "detail:mine",
        json!({"type": "setTab", "tab": "private"}),
    )
    .await
    .expect("ok");
    let d = h.snap("detail:mine");
    assert_eq!(d["body"]["tab"], "private");
    assert_eq!(
        d["body"]["emptyLogText"],
        "No private entries yet. Only agents can see the private log."
    );
    assert_eq!(
        d["composer"]["placeholder"], "Write a private entry, only agents see it",
        "LOG-2"
    );
    assert_eq!(d["composer"]["sendLabel"], "Add to private log");
    select(&h, MINE, Some("UserRequest::2"));
    h.settle().await;
    assert_eq!(h.snap("detail:mine")["body"]["tab"], "public");
}

#[tokio::test(start_paused = true)]
async fn det_37_39_log_entries() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.detail.lock().unwrap_or_else(|e| e.into_inner())["public_log"] = json!({"entries": [
        {"date": "2026-11-16 09:12:00", "user_login": "jane", "message": "Hello", "message_html": "<p>Hello</p>"}
    ]});
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    let e = &h.snap("detail:mine")["body"]["entries"][0];
    let header = e["header"].as_str().unwrap_or_default();
    assert!(header.starts_with("jane · "), "{header}");
    assert_eq!(e["isPrivate"], false);
    assert_eq!(e["content"]["blocks"][0]["runs"][0]["text"], "Hello");
}

#[tokio::test(start_paused = true)]
async fn log_4_to_7_drafts_and_send() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    send(&h, "detail:mine", json!({"type": "setDraft", "text": "  "}))
        .await
        .expect("ok");
    assert_eq!(h.snap("detail:mine")["composer"]["canSend"], false, "blank");
    send(
        &h,
        "detail:mine",
        json!({"type": "setDraft", "text": "Rebooted it"}),
    )
    .await
    .expect("ok");
    send(
        &h,
        "detail:mine",
        json!({"type": "setTab", "tab": "private"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("detail:mine")["composer"]["draft"],
        "",
        "LOG-4: per log"
    );
    send(
        &h,
        "detail:mine",
        json!({"type": "setTab", "tab": "public"}),
    )
    .await
    .expect("ok");
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(c["draft"], "Rebooted it");
    assert_eq!(c["canSend"], true);
    assert_eq!(c["sendHelp"], "Add the entry (⌘↩)");
    let r = send(&h, "detail:mine", json!({"type": "send"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Bool(true));
    assert_eq!(h.snap("detail:mine")["composer"]["draft"], "");
    let update = h
        .stub
        .requests()
        .into_iter()
        .find(|r| r.operation().as_deref() == Some("core/update"))
        .expect("update");
    assert_eq!(
        update.json_data()["fields"]["public_log"],
        json!({"add_item": {"message": "Rebooted it", "format": "text"}}),
        "LOG-6"
    );
}

#[tokio::test(start_paused = true)]
async fn log_8_failure_keeps_the_draft() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    *h.itop
        .refuse_writes
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((100, "Locked".into()));
    send(&h, "detail:mine", json!({"type": "setDraft", "text": "Hi"}))
        .await
        .expect("ok");
    let r = send(&h, "detail:mine", json!({"type": "send"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Bool(false));
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(c["draft"], "Hi");
    assert_eq!(
        c["status"],
        json!({"text": "iTop didn't save the entry. iTop returned error 100: Locked. Your draft is kept.", "isError": true})
    );
}

const PNG: [u8; 33] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
    2, 0, 0, 0, 3, 8, 6, 0, 0, 0, 0, 0, 0, 0,
];

fn b64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[tokio::test(start_paused = true)]
async fn log_13_16_to_20_image_intake() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    let png = b64(&PNG);
    let items: Vec<Value> = (0..6)
        .map(|i| json!({"filename": format!("shot{i}.png"), "dataBase64": png, "size": 33}))
        .collect();
    send(
        &h,
        "detail:mine",
        json!({"type": "addImages", "items": items}),
    )
    .await
    .expect("ok");
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(c["images"].as_array().map(Vec::len), Some(5));
    assert_eq!(
        c["status"]["text"],
        "An entry can have up to 5 images, so 1 wasn't added."
    );
    assert_eq!(c["imagesAccessibilityLabel"], "5 images to add");
    assert_eq!(c["canAddImages"], false);
    assert_eq!(c["paperclipHelp"], "An entry can have up to 5 images");
    assert_eq!(c["images"][0]["sizeText"], "33 bytes");
    let id = c["images"][0]["id"].as_str().unwrap_or_default().to_owned();
    assert_eq!(
        c["images"][0]["thumbnailSrc"],
        format!("itop://localhost/draft-image/{id}")
    );
    send(
        &h,
        "detail:mine",
        json!({"type": "removeImage", "imageId": id}),
    )
    .await
    .expect("ok");
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(c["images"].as_array().map(Vec::len), Some(4));
    assert!(c["status"].is_null(), "removing clears the error");
    send(
        &h,
        "detail:mine",
        json!({"type": "addImages", "items": [
            {"filename": "huge.png", "dataBase64": null, "size": 50_000_000},
            {"filename": "notes.txt", "dataBase64": b64(b"hello"), "size": 5},
        ]}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("detail:mine")["composer"]["status"]["text"],
        "huge.png is larger than 4 MB. notes.txt isn't an image iTop Wrap can add. Use PNG, JPEG or GIF."
    );
}

#[tokio::test(start_paused = true)]
async fn log_18_conversion_names() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    *h.native.converted.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(crate::native::ConvertedImage {
            data: PNG.to_vec(),
            ext: "png",
        });
    send(&h, "detail:mine", json!({"type": "addImages", "items": [{"filename": "", "dataBase64": b64(&[0x49, 0x49, 0x2A, 0x00, 1]), "size": 5}]})).await.expect("ok");
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(
        c["images"][0]["filename"], "Pasted image.png",
        "LOG-14 and LOG-18"
    );
    assert!(c["status"].is_null());
}

#[test]
fn log_14_pasted_data_is_named_by_type() {
    let tiff = super::detail::intake_item(super::records::ImageIntakeItem {
        filename: String::new(),
        data_base64: Some(b64(&[0x4D, 0x4D, 0x00, 0x2A])),
        size: 4,
    });
    assert_eq!(tiff.filename, "Pasted image.tiff");
    let png = super::detail::intake_item(super::records::ImageIntakeItem {
        filename: String::new(),
        data_base64: Some(b64(&PNG)),
        size: 33,
    });
    assert_eq!(png.filename, "Pasted image.png");
    let big = super::detail::intake_item(super::records::ImageIntakeItem {
        filename: "a.heic".into(),
        data_base64: None,
        size: 41 * 1_048_576,
    });
    assert!(big.oversize);
}

#[tokio::test(start_paused = true)]
async fn det_44_assets() {
    let a = saved_app();
    a.router.native_event(NativeEvent::Launched).await;
    settle().await;
    a.itop.can(
        "InlineImage",
        objects(vec![object(
            "InlineImage",
            "5",
            json!({"contents": {"data": b64(&PNG), "mimetype": "image/png"}}),
        )]),
    );
    let got = a.router.asset("inline-image", "5").await;
    assert_eq!(got, Some((PNG.to_vec(), "image/png".to_owned())));
    assert_eq!(a.router.asset("draft-image", "nope").await, None);
    assert_eq!(a.router.asset("other", "5").await, None);
}

// =============================================================================================
// Assign sheet (ASG-*)
// =============================================================================================

fn teams_and_agents(itop: &FakeITop) {
    itop.can(
        "Team",
        objects(vec![
            object("Team", "3", json!({"friendlyname": "Service desk"})),
            object("Team", "8", json!({"friendlyname": "Network"})),
        ]),
    );
    itop.can(
        "Person",
        objects(vec![
            object("Person", "7", json!({"friendlyname": "Agent Seven"})),
            object("Person", "9", json!({"friendlyname": "Agent 9"})),
        ]),
    );
}

#[tokio::test(start_paused = true)]
async fn asg_6_to_12_sheet() {
    let h = detail_harness(ticket("1", "assigned", "9"), true).await;
    teams_and_agents(&h.itop);
    send(
        &h,
        "assign:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("assign:mine");
    assert_eq!(s["type"], "open");
    assert_eq!(s["title"], "Reassign R-000001");
    assert_eq!(s["subtitle"], "Ticket 1");
    assert_eq!(
        s["assignToMe"],
        json!({"enabled": true, "help": "Assign R-000001 to you"})
    );
    assert_eq!(
        s["team"]["selectedId"], "3",
        "ASG-9: the ticket's team preselected"
    );
    assert_eq!(s["team"]["placeholder"], "Choose…");
    assert_eq!(
        s["agent"]["items"][1],
        json!({"id": "7", "label": "Agent Seven (you)"}),
        "ASG-10"
    );
    assert_eq!(s["confirmLabel"], "Reassign ticket");
    assert_eq!(s["confirmEnabled"], false, "ASG-12: no agent yet");
    send(
        &h,
        "assign:mine",
        json!({"type": "setAgent", "agentId": "9"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("assign:mine")["confirmEnabled"], true);
    send(&h, "assign:mine", json!({"type": "setTeam", "teamId": "8"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("assign:mine")["agent"]["selectedId"],
        Value::Null,
        "ASG-11: the agent clears"
    );
    send(
        &h,
        "assign:mine",
        json!({"type": "setTeam", "teamId": null}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("assign:mine")["agent"]["placeholder"],
        "Choose a team first"
    );
    assert_eq!(h.snap("assign:mine")["agent"]["enabled"], false);
    send(&h, "assign:mine", json!({"type": "close"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("assign:mine"), json!({"type": "closed"}));
}

#[tokio::test(start_paused = true)]
async fn asg_9_placeholders_while_loading() {
    let h = detail_harness(ticket("1", "new", "0"), true).await;
    route_workspace(
        &h.ws,
        "assign:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("assign:mine");
    assert_eq!(s["title"], "Assign R-000001");
    assert_eq!(s["team"]["placeholder"], "Loading teams…");
    assert_eq!(s["team"]["enabled"], false);
    h.settle().await;
    assert_eq!(
        h.snap("assign:mine")["team"]["placeholder"],
        "No teams found"
    );
}

#[tokio::test(start_paused = true)]
async fn asg_13_confirm_success_and_notice() {
    let h = detail_harness(ticket("1", "assigned", "9"), true).await;
    teams_and_agents(&h.itop);
    send(
        &h,
        "assign:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    send(
        &h,
        "assign:mine",
        json!({"type": "setAgent", "agentId": "7"}),
    )
    .await
    .expect("ok");
    let r = send(&h, "assign:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Null);
    assert_eq!(h.snap("assign:mine"), json!({"type": "closed"}));
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["notice"],
        json!({"text": "R-000001 is assigned to you in Service desk.", "isError": false})
    );
}

#[tokio::test(start_paused = true)]
async fn asg_13_failure_stays_open() {
    let h = detail_harness(ticket("1", "assigned", "9"), true).await;
    teams_and_agents(&h.itop);
    *h.itop
        .refuse_stimulus
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((100, "No".into()));
    send(
        &h,
        "assign:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    send(
        &h,
        "assign:mine",
        json!({"type": "setAgent", "agentId": "9"}),
    )
    .await
    .expect("ok");
    let r = send(&h, "assign:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(
        r,
        json!("iTop didn't assign R-000001. iTop returned error 100: No.")
    );
    let s = h.snap("assign:mine");
    assert_eq!(s["type"], "open");
    assert_eq!(
        s["error"],
        "iTop didn't assign R-000001. iTop returned error 100: No."
    );
}

#[tokio::test(start_paused = true)]
async fn asg_2_already_taken_reloads() {
    let h = detail_harness(ticket("1", "new", "0"), true).await;
    let copy = h.itop.get("1").expect("t");
    h.itop.set("1", |t| {
        t.agent_id = "9".into();
        t.agent_name = "Bea".into();
    });
    h.ws.list_assign_to_me(MINE, &copy).await;
    assert_eq!(
        h.ws.lock().lists[0].notice,
        Some(super::records::Notice::error(
            "Bea took R-000001 a moment ago, so it stays with them."
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn asg_5_no_team() {
    let mut user = me();
    user.teams.clear();
    let h = Harness::with(
        FakeITop::with(vec![ticket("1", "new", "0")]),
        Setup {
            user,
            ..Setup::default()
        },
    );
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
    h.load().await;
    let t = h.itop.get("1").expect("t");
    h.ws.list_assign_to_me(MINE, &t).await;
    assert_eq!(
        h.ws.lock().lists[0]
            .notice
            .as_ref()
            .map(|n| n.text.clone())
            .as_deref(),
        Some(
            "You aren't in any team in iTop, so Assign to me can't pick one. Choose a team and agent instead."
        )
    );
}

#[tokio::test(start_paused = true)]
async fn asg_7_assign_to_me_in_the_sheet() {
    let h = detail_harness(ticket("1", "new", "0"), true).await;
    send(
        &h,
        "assign:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let r = send(&h, "assign:mine", json!({"type": "assignToMe"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Null);
    assert_eq!(h.snap("assign:mine"), json!({"type": "closed"}));
}

// =============================================================================================
// Pending, resolve, reopen, close (LC-*)
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn lc_3_one_change_at_a_time() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    h.ws.lock().lists[0].working = Some("UserRequest::9".into());
    let t = h.itop.get("1").expect("t");
    assert_eq!(
        h.ws.list_apply(MINE, &TicketChange::Close, &t)
            .await
            .as_deref(),
        Some("Another change is still running.")
    );
    let team = itop_kit::Team::new("3", "S");
    let agent = itop_kit::Agent::new("7", "A");
    assert_eq!(
        h.ws.list_assign(MINE, &t, &team, &agent).await.as_deref(),
        Some("Another assignment is still running.")
    );
}

#[tokio::test(start_paused = true)]
async fn lc_8_to_12_pending_sheet() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    send(
        &h,
        "pending:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("pending:mine");
    assert_eq!(s["title"], "Set R-000001 to pending");
    assert_eq!(s["reasonRequired"], json!({"missing": false}));
    let r = send(&h, "pending:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Null);
    let s = h.snap("pending:mine");
    assert_eq!(s["reasonRequired"], json!({"missing": true}));
    assert_eq!(s["reasonError"], "Enter why the ticket is waiting.");
    assert_eq!(h.calls("core/apply_stimulus", None), 0, "nothing sent");
    send(
        &h,
        "pending:mine",
        json!({"type": "setReason", "reason": " The caller's reply "}),
    )
    .await
    .expect("ok");
    send(&h, "pending:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("pending:mine"), json!({"type": "closed"}));
    let sent = h
        .stub
        .requests()
        .into_iter()
        .find(|r| r.operation().as_deref() == Some("core/apply_stimulus"))
        .expect("stimulus");
    assert_eq!(
        sent.json_data()["fields"]["pending_reason"],
        "The caller's reply"
    );
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["notice"],
        json!({"text": "R-000001 is set to pending.", "isError": false}),
        "LC-5"
    );
}

#[tokio::test(start_paused = true)]
async fn lc_4_11_pending_stops_and_saves_the_timer_first() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    h.itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    let t = h.itop.get("1").expect("t");
    h.ws.timer_start(&t).await;
    h.native.advance(chrono::Duration::minutes(42));
    send(
        &h,
        "pending:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("pending:mine")["timerLine"],
        "Your timer stops and saves 0:42 to this ticket."
    );
    send(
        &h,
        "pending:mine",
        json!({"type": "setReason", "reason": "Part"}),
    )
    .await
    .expect("ok");
    send(&h, "pending:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    let timer = h.snap("timer");
    assert_eq!(timer["running"], Value::Null);
    assert_eq!(
        timer["notice"]["text"],
        "Setting R-000001 to pending stopped its timer. Saved 0:42 to R-000001."
    );
}

#[tokio::test(start_paused = true)]
async fn lc_6_23_reopen_failure_goes_to_the_notice() {
    let h = detail_harness(ticket("1", "resolved", "7"), true).await;
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
    h.ws.lock().options.include_closed = true;
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    *h.itop
        .refuse_stimulus
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((100, "Closed already".into()));
    send(&h, "detail:mine", json!({"type": "reopen"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["notice"],
        json!({"text": "iTop didn't reopen R-000001. iTop returned error 100: Closed already.", "isError": true})
    );
}

#[tokio::test(start_paused = true)]
async fn lc_13_to_21_resolve_sheet() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "assigned", "7"),
        ticket("2", "assigned", "7"),
    ]));
    *h.itop.detail.lock().unwrap_or_else(|e| e.into_inner()) = json!({
        "description": "", "service_id": "5", "service_id_friendlyname": "",
        "servicesubcategory_id": "6", "servicesubcategory_id_friendlyname": "VPN",
    });
    h.itop.can(
        "Service",
        objects(vec![object(
            "Service",
            "2",
            json!({"friendlyname": "Email"}),
        )]),
    );
    h.itop.can(
        "ServiceSubcategory",
        objects(vec![object(
            "ServiceSubcategory",
            "7",
            json!({"friendlyname": "Other"}),
        )]),
    );
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    send(
        &h,
        "resolve:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("resolve:mine");
    assert_eq!(s["title"], "Resolve R-000001");
    assert_eq!(
        s["service"]["items"][0],
        json!({"id": "5", "label": "Service 5"}),
        "LC-15: inserted at the top"
    );
    assert_eq!(s["service"]["selectedId"], "5");
    assert_eq!(
        s["subcategory"]["items"][0],
        json!({"id": "6", "label": "VPN"}),
        "kept, inserted at the top"
    );
    assert_eq!(s["subcategory"]["selectedId"], "6");
    assert_eq!(s["code"]["selectedId"], "assistance", "LC-18");
    assert_eq!(
        s["code"]["items"][1],
        json!({"id": "bug fixed", "label": "Bug fixed"})
    );
    send(
        &h,
        "resolve:mine",
        json!({"type": "setService", "serviceId": "2"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("resolve:mine")["subcategory"]["selectedId"],
        Value::Null,
        "LC-16"
    );
    let r = send(&h, "resolve:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(r, Value::Null);
    let s = h.snap("resolve:mine");
    assert_eq!(s["subcategoryRequired"], json!({"missing": true}));
    assert_eq!(s["solutionRequired"], json!({"missing": true}));
    assert_eq!(
        s["solutionError"],
        "Describe what fixed it. The caller sees this."
    );
    send(
        &h,
        "resolve:mine",
        json!({"type": "setSubcategory", "subcategoryId": "7"}),
    )
    .await
    .expect("ok");
    send(
        &h,
        "resolve:mine",
        json!({"type": "setSolution", "solution": "Restarted"}),
    )
    .await
    .expect("ok");
    send(&h, "resolve:mine", json!({"type": "confirm"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("resolve:mine"), json!({"type": "closed"}));
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["extraSection"]["title"],
        "Resolved just now",
        "LC-22"
    );
}

#[tokio::test(start_paused = true)]
async fn lc_17_resolve_placeholders() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    route_workspace(
        &h.ws,
        "resolve:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let s = h.snap("resolve:mine");
    assert_eq!(s["service"]["placeholder"], "Loading services…");
    assert_eq!(s["subcategory"]["placeholder"], "Choose a service first");
    h.settle().await;
    assert_eq!(
        h.snap("resolve:mine")["service"]["placeholder"],
        "No services found"
    );
}

// =============================================================================================
// Timer (TMR-*)
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn tmr_2_start_only_on_assigned_tickets_of_yours() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "escalated_ttr", "7"),
        ticket("2", "assigned", "9"),
        ticket("3", "assigned", "7"),
    ]));
    *h.itop.listed.lock().unwrap_or_else(|e| e.into_inner()) = Box::new(|_, _| true);
    h.load().await;
    for id in ["UserRequest::1", "UserRequest::2"] {
        send(&h, "timer", json!({"type": "start", "ticketId": id}))
            .await
            .expect("ok");
        assert_eq!(h.snap("timer")["running"], Value::Null, "{id}");
    }
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::3"}),
    )
    .await
    .expect("ok");
    let r = &h.snap("timer")["running"];
    assert_eq!(r["ticketId"], "UserRequest::3");
    assert_eq!(r["counter"], "00:00:00");
    assert!(
        h.prefs.string_value("timer.running").is_some(),
        "TMR-6: saved"
    );
}

trait PrefsExt {
    fn string_value(&self, key: &str) -> Option<String>;
}
impl PrefsExt for MemoryPrefs {
    fn string_value(&self, key: &str) -> Option<String> {
        itop_kit::SettingsStore::string(self, key)
    }
}

#[tokio::test(start_paused = true)]
async fn tmr_3_writes_off_notice() {
    let h = detail_harness(ticket("1", "assigned", "7"), false).await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("timer")["notice"],
        json!({"text": "Saving time to iTop is switched off in this build, so the timer can't start.", "isError": true, "canRetry": false, "dismissLabel": "Dismiss"})
    );
}

fn stopwatch(itop: &FakeITop) {
    itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    itop.can(
        "TimeSpent",
        objects(vec![object(
            "TimeSpent",
            "50",
            json!({"start_date": "2026-11-16 10:42:00", "end_date": "", "duration": "0"}),
        )]),
    );
}

#[tokio::test(start_paused = true)]
async fn tmr_4_15_19_web_stopwatch() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    stopwatch(&h.itop);
    h.load().await;
    select(&h, MINE, Some("UserRequest::1"));
    h.settle().await;
    let label = h.snap("detail:mine")["timerButton"]["label"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(label.starts_with("Web stopwatch since "), "{label}");
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let notice = h.snap("timer")["notice"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        notice.starts_with("Your web stopwatch is already running on R-000001 since "),
        "{notice}"
    );
    assert!(notice.ends_with(". Stop it in iTop, or keep using it there."));
    assert_eq!(h.snap("timer")["running"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn tmr_5_7_starting_another_stops_the_first() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "assigned", "7"),
        ticket("2", "assigned", "7"),
    ]));
    h.load().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    h.native.advance(chrono::Duration::seconds(30));
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::2"}),
    )
    .await
    .expect("ok");
    let t = h.snap("timer");
    assert_eq!(t["running"]["ticketId"], "UserRequest::2");
    assert_eq!(
        t["notice"],
        Value::Null,
        "the new start clears the under-a-minute notice"
    );
    send(
        &h,
        "timer",
        json!({"type": "toggle", "ticketId": "UserRequest::2"}),
    )
    .await
    .expect("ok");
    assert_eq!(
        h.snap("timer")["notice"]["text"],
        "The timer on R-000002 ran under a minute, so nothing was saved."
    );
}

#[tokio::test(start_paused = true)]
async fn tmr_8_10_saved_and_total_goes_up() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    h.load().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    h.native.advance(chrono::Duration::hours(5));
    send(&h, "timer", json!({"type": "stop"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("timer")["notice"]["text"],
        "Saved 4:00 to R-000001. It stopped at 4:00, like iTop's own stopwatch."
    );
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["sections"][0]["rows"][0]["time"],
        json!({"type": "spent", "text": "4h"})
    );
    let created = h
        .stub
        .requests()
        .into_iter()
        .find(|r| {
            r.class().as_deref() == Some("TimeSpent")
                && r.operation().as_deref() == Some("core/create")
        })
        .expect("TimeSpent");
    assert_eq!(created.json_data()["fields"]["duration"], "14400");
}

#[tokio::test(start_paused = true)]
async fn tmr_11_12_failed_save_retry_and_discard() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    h.load().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    h.native.advance(chrono::Duration::minutes(5));
    *h.itop
        .refuse_writes
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((100, "Nope".into()));
    send(&h, "timer", json!({"type": "stop"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("timer")["notice"],
        json!({"text": "Couldn't save 0:05 to R-000001. iTop returned error 100: Nope.", "isError": true, "canRetry": true, "dismissLabel": "Discard this time"})
    );
    *h.itop
        .refuse_writes
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    send(&h, "timer", json!({"type": "retry"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("timer")["notice"]["text"],
        "Saved 0:05 to R-000001.",
        "no prefix"
    );
    send(&h, "timer", json!({"type": "clearNotice"}))
        .await
        .expect("ok");
    assert_eq!(h.snap("timer")["notice"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn tmr_14_left_assigned_stops_on_the_next_my_tickets_reload() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    h.load().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    h.native.advance(chrono::Duration::minutes(10));
    h.itop.set("1", |t| t.status = "pending".into());
    h.load().await;
    assert_eq!(
        h.snap("timer")["notice"]["text"],
        "R-000001 left Assigned, so its timer stopped. Saved 0:10 to R-000001."
    );
}

#[tokio::test(start_paused = true)]
async fn tmr_17_totals_open_tickets_only() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can(
        "Activity",
        objects(vec![object("Activity", "40", json!({"obj_id": "1"}))]),
    );
    h.itop.can(
        "TimeSpent",
        objects(vec![object(
            "TimeSpent",
            "50",
            json!({"activity_id": "40", "duration": "4800"}),
        )]),
    );
    h.load().await;
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["sections"][0]["rows"][0]["time"],
        json!({"type": "spent", "text": "1h 20m"})
    );
}

#[tokio::test(start_paused = true)]
async fn tmr_19_timer_button_states() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    assert_eq!(
        h.snap("detail:mine")["timerButton"],
        json!({"type": "start", "enabled": true, "help": "Start timer (⌘T)"})
    );
    send(&h, "detail:mine", json!({"type": "toggleTimer"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("detail:mine")["timerButton"],
        json!({"type": "running"})
    );
    assert_eq!(
        h.snap("list:mine")["content"]["body"]["sections"][0]["rows"][0]["time"],
        json!({"type": "running"}),
        "TMR-22"
    );
    let h = detail_harness(ticket("1", "pending", "7"), true).await;
    assert_eq!(
        h.snap("detail:mine")["timerButton"],
        json!({"type": "none"})
    );
}

#[tokio::test(start_paused = true)]
async fn tmr_6_the_saved_timer_comes_back() {
    let prefs = MemoryPrefs::new();
    use itop_kit::SettingsStore;
    prefs.set_string("timer.running", Some(r#"{"ticketClass":"UserRequest","ticketKey":"1","ref":"R-000001","title":"T","orgID":"4","start":785000000}"#));
    let h = Harness::with(
        FakeITop::default(),
        Setup {
            prefs,
            ..Setup::default()
        },
    );
    assert_eq!(h.snap("timer")["running"]["ref"], "R-000001");
    assert_eq!(
        h.snap("timer")["running"]["startedAt"],
        "2025-11-16T15:33:20Z"
    );
}

// =============================================================================================
// Menu bar (MB-*)
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn mb_2_5_tray_title_and_label() {
    let mut esc = ticket("1", "escalated_ttr", "7");
    esc.title = "Mail".into();
    let h = Harness::new(FakeITop::with(vec![esc, ticket("2", "assigned", "7")]));
    h.load().await;
    let m = h.snap("menubar");
    assert_eq!(m["trayTitle"], "1");
    assert_eq!(m["trayAccessibilityLabel"], "iTop Wrap: 1 ticket needs you");
    assert!(h.native.calls().contains(&Call::SetTray(
        "1".into(),
        "iTop Wrap: 1 ticket needs you".into()
    )));
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::2"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("menubar")["trayTitle"], "00:00:00  1");
}

#[tokio::test(start_paused = true)]
async fn mb_8_to_15_panel() {
    let h = Harness::new(FakeITop::with(vec![
        ticket("1", "escalated_ttr", "7"),
        ticket("2", "assigned", "7"),
        ticket("3", "assigned", "7"),
        ticket("4", "new", "7"),
        ticket("5", "pending", "7"),
    ]));
    h.load().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::2"}),
    )
    .await
    .expect("ok");
    let c = &h.snap("menubar")["content"];
    assert_eq!(c["type"], "list");
    assert_eq!(c["running"]["ticketId"], "UserRequest::2", "MB-10");
    assert_eq!(c["running"]["right"], json!({"type": "running"}));
    assert_eq!(c["running"]["button"], json!({"type": "stop"}));
    let titles: Vec<&str> = c["sections"]
        .as_array()
        .map(|s| s.iter().filter_map(|x| x["title"].as_str()).collect())
        .unwrap_or_default();
    assert_eq!(
        titles,
        vec!["Escalated TTR  1", "Assigned  1", "Pending  1"],
        "MB-8, MB-11: New left out"
    );
    assert_eq!(
        c["sections"][1]["rows"][0]["button"],
        json!({"type": "start", "enabled": true, "help": "Stops the timer on R-000002, saves its time, and starts one here"}),
        "MB-13"
    );
    assert_eq!(
        c["sections"][0]["rows"][0]["button"],
        json!({"type": "slot"})
    );
    assert_eq!(c["height"], (3 + 1) * 30 + 4 * 48 + 6, "MB-15");
    send(&h, "menubar", json!({"type": "setQuery", "query": "zzz"}))
        .await
        .expect("ok");
    assert_eq!(
        h.snap("menubar")["content"],
        json!({"type": "empty", "text": "No tickets of yours match “zzz”."})
    );
    assert_eq!(h.snap("list:mine")["query"], "", "the panel's own search");
}

#[tokio::test(start_paused = true)]
async fn mb_14_16_states() {
    let h = Harness::new(FakeITop::default());
    assert_eq!(h.snap("menubar")["content"], json!({"type": "loading"}));
    assert_eq!(h.snap("menubar")["updatedText"], Value::Null);
    h.load().await;
    let m = h.snap("menubar");
    assert_eq!(
        m["content"],
        json!({"type": "empty", "text": "Nothing waiting on you. New assignments show up here."})
    );
    assert_eq!(m["updatedText"], "Updated now");
    h.native.advance(chrono::Duration::minutes(2));
    assert_eq!(h.snap("menubar")["updatedText"], "Updated 2 minutes ago");
    let h = Harness::new(FakeITop::default());
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = true;
    h.load().await;
    assert_eq!(
        h.snap("menubar")["content"]["text"],
        "Couldn't load your tickets. Can't reach itop.example.com. Check your connection, then retry."
    );
}

#[tokio::test(start_paused = true)]
async fn mb_12_open_ticket_from_the_panel() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    send(&h, "shell", json!({"type": "select", "item": "people"}))
        .await
        .expect("ok");
    h.ws.set_query(MINE, "x");
    send(
        &h,
        "menubar",
        json!({"type": "openTicket", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    assert_eq!(h.snap("shell")["selection"], "myTickets");
    let l = h.snap("list:mine");
    assert_eq!(l["query"], "");
    assert_eq!(l["selection"], "UserRequest::1");
    assert!(h.native.calls().contains(&Call::ShowMainWindow));
    send(&h, "menubar", json!({"type": "newTicket"}))
        .await
        .expect("ok");
    assert!(h.native.calls().contains(&Call::ShowNewTicketWindow));
}

// =============================================================================================
// Polling (POLL-*)
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn poll_1_my_tickets_every_120_seconds() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.ws.start();
    h.settle().await;
    let first = loads(&h);
    assert!(first >= 2, "loaded at start");
    tokio::time::sleep(Duration::from_secs(121)).await;
    h.settle().await;
    assert_eq!(loads(&h), first + 2, "one reload (two classes) after 120 s");
    h.ws.stop();
}

#[tokio::test(start_paused = true)]
async fn poll_2_team_polls_only_while_shown() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "9")]));
    send(
        &h,
        "shell",
        json!({"type": "select", "item": "teamTickets"}),
    )
    .await
    .expect("ok");
    let gets = || {
        h.stub
            .requests()
            .iter()
            .filter(|r| {
                r.json_data()["key"]
                    .as_str()
                    .is_some_and(|k| k.contains("team_id IN") && !k.contains("agent_id ="))
            })
            .count()
    };
    assert_eq!(gets(), 2, "loads on appear");
    tokio::time::sleep(Duration::from_secs(121)).await;
    h.settle().await;
    assert_eq!(gets(), 4);
    send(&h, "shell", json!({"type": "select", "item": "myTickets"}))
        .await
        .expect("ok");
    tokio::time::sleep(Duration::from_secs(250)).await;
    h.settle().await;
    assert_eq!(gets(), 4, "stopped");
}

#[tokio::test(start_paused = true)]
async fn poll_8_tray_ticks_every_second_with_a_timer() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.ws.start();
    h.settle().await;
    send(
        &h,
        "timer",
        json!({"type": "start", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    for _ in 0..3 {
        h.native.advance(chrono::Duration::seconds(1));
        tokio::time::sleep(Duration::from_millis(1000)).await;
        h.settle().await;
    }
    assert_eq!(h.hub.get("menubar")["trayTitle"], "00:00:03");
    assert_eq!(h.hub.get("timer")["running"]["counter"], "00:00:03");
    h.ws.stop();
}

// =============================================================================================
// Native events
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn key_2_menu_selects_sections_and_new_1_opens_the_sheet() {
    let a = saved_app();
    a.router.native_event(NativeEvent::Launched).await;
    settle().await;
    a.router
        .native_event(NativeEvent::MenuSelectSection(4))
        .await;
    assert_eq!(a.hub.get("shell")["selection"], "people");
    a.router.native_event(NativeEvent::MenuNewTicket).await;
    settle().await;
    assert_eq!(a.hub.get("newTicket:sheet")["type"], "open");
    a.router.native_event(NativeEvent::PanelNewTicket).await;
    settle().await;
    assert_eq!(a.hub.get("newTicket:window")["type"], "open");
    a.router
        .native_event(NativeEvent::NewTicketWindowClosed)
        .await;
    settle().await;
    assert_eq!(
        a.hub.get("newTicket:window")["type"],
        "closed",
        "dismissed, no draft"
    );
}

// =============================================================================================
// JSON shapes against contract.ts
// =============================================================================================

#[tokio::test(start_paused = true)]
async fn shapes_list_and_detail_snapshots() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    let l = h.snap("list:mine");
    let keys = |v: &Value| {
        let mut k: Vec<String> = v
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        k.sort();
        k
    };
    assert_eq!(
        keys(&l),
        [
            "content",
            "footerSpinner",
            "needCount",
            "orgFilter",
            "pager",
            "query",
            "refreshing",
            "scope",
            "selection",
            "showControls",
            "updatedText",
            "working"
        ]
    );
    assert_eq!(
        keys(&l["content"]["body"]),
        [
            "extraSection",
            "lookupBar",
            "noMatchText",
            "notice",
            "orgNoMatch",
            "refreshError",
            "sections",
            "showNotices"
        ]
    );
    let row = &l["content"]["body"]["sections"][0]["rows"][0];
    assert_eq!(
        keys(row),
        [
            "age",
            "agent",
            "openedTooltip",
            "priorityMark",
            "ticket",
            "time"
        ]
    );
    assert_eq!(
        row["ticket"],
        json!({"id": "UserRequest::1", "key": "1", "ticketClass": "UserRequest", "ref": "R-000001", "title": "Ticket 1",
            "status": "assigned", "statusLabel": "Assigned", "priority": 4, "callerName": "Caller", "orgName": "Org A",
            "orgId": "4", "teamId": "3", "teamName": "Service desk", "agentId": "7", "agentName": "Agent Seven",
            "startDate": null, "lastUpdate": null, "isUnassigned": false, "isMine": true, "needsYou": false})
    );
    assert_eq!(l["content"]["body"]["sections"][0]["group"], "assigned");
    assert_eq!(l["content"]["body"]["sections"][0]["title"], "Assigned");
    let d = h.snap("detail:mine");
    assert_eq!(
        keys(&d),
        [
            "attachments",
            "body",
            "classLabel",
            "closeConfirm",
            "composer",
            "copied",
            "more",
            "primary",
            "strip",
            "ticket",
            "timerButton",
            "type"
        ]
    );
    assert_eq!(
        keys(&d["composer"]),
        [
            "accessibilityLabel",
            "canAddImages",
            "canSend",
            "draft",
            "editable",
            "images",
            "imagesAccessibilityLabel",
            "isPrivate",
            "maxIntakeFileBytes",
            "paperclipHelp",
            "placeholder",
            "sendHelp",
            "sendLabel",
            "sending",
            "status"
        ]
    );
    assert_eq!(d["composer"]["maxIntakeFileBytes"], 41_943_040);
    assert_eq!(
        keys(&d["body"]),
        [
            "description",
            "emptyLogText",
            "entries",
            "serviceLine",
            "tab",
            "type"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn shapes_options_timer_menubar_and_sheets() {
    let h = detail_harness(ticket("1", "assigned", "7"), true).await;
    let o = h.snap("options");
    assert_eq!(o["range"], 0);
    assert_eq!(o["sort"], "priority");
    assert_eq!(o["pageSize"], 20);
    assert_eq!(o["summary"], "All tickets, any time");
    assert_eq!(
        h.snap("timer"),
        json!({"running": null, "saving": false, "notice": null})
    );
    let m = h.snap("menubar");
    let mut keys: Vec<&String> = m
        .as_object()
        .map(|o| o.keys().collect())
        .unwrap_or_default();
    keys.sort();
    assert_eq!(
        keys,
        [
            "content",
            "query",
            "refreshing",
            "trayAccessibilityLabel",
            "trayTitle",
            "updatedText"
        ]
    );
    send(
        &h,
        "pending:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let p = h.snap("pending:mine");
    let mut keys: Vec<&String> = p
        .as_object()
        .map(|o| o.keys().collect())
        .unwrap_or_default();
    keys.sort();
    assert_eq!(
        keys,
        [
            "busy",
            "confirmEnabled",
            "error",
            "reason",
            "reasonError",
            "reasonRequired",
            "subtitle",
            "ticketId",
            "timerLine",
            "title",
            "type"
        ]
    );
    send(
        &h,
        "resolve:mine",
        json!({"type": "open", "ticketId": "UserRequest::1"}),
    )
    .await
    .expect("ok");
    let r = h.snap("resolve:mine");
    assert_eq!(r["service"]["search"], Value::Null);
    let mut keys: Vec<&String> = r
        .as_object()
        .map(|o| o.keys().collect())
        .unwrap_or_default();
    keys.sort();
    assert_eq!(
        keys,
        [
            "busy",
            "code",
            "confirmEnabled",
            "error",
            "service",
            "serviceRequired",
            "solution",
            "solutionError",
            "solutionRequired",
            "subcategory",
            "subcategoryRequired",
            "subtitle",
            "ticketId",
            "timerLine",
            "title",
            "type"
        ]
    );
}

#[test]
fn shapes_session_snapshots() {
    let s =
        super::session::SessionModelState::new(Some("http://itop.lan".into()), Some("me".into()));
    let mut s2 = s;
    s2.set_state(super::session::SessionState::SignedOut { notice: None });
    assert_eq!(
        serde_json::to_value(s2.snapshot()).ok(),
        Some(
            json!({"type": "signedOut", "form": {"server": "http://itop.lan", "user": "me", "httpWarning": true,
            "signingIn": false, "message": null, "initialFocus": "password"}})
        )
    );
    s2.set_state(super::session::SessionState::Unreachable("x".into()));
    assert_eq!(
        serde_json::to_value(s2.snapshot()).ok(),
        Some(json!({"type": "unreachable", "message": "x"}))
    );
}

#[test]
fn shapes_intents_deserialize() {
    use super::detail::DetailIntent;
    use super::list::ListIntent;
    use super::shell::ShellIntent;
    let i: ListIntent =
        serde_json::from_value(json!({"type": "rowAssignToMe", "ticketId": "UserRequest::1"}))
            .expect("list");
    assert_eq!(
        i,
        ListIntent::RowAssignToMe {
            ticket_id: "UserRequest::1".into()
        }
    );
    let i: ShellIntent = serde_json::from_value(
        json!({"type": "newTicketForCI", "key": "1", "name": "n", "orgName": "o"}),
    )
    .expect("shell");
    assert!(matches!(i, ShellIntent::NewTicketForCi { .. }));
    let i: DetailIntent = serde_json::from_value(
        json!({"type": "addImages", "items": [{"filename": "", "dataBase64": null, "size": 1}]}),
    )
    .expect("detail");
    assert!(matches!(i, DetailIntent::AddImages { .. }));
    let i: super::options::OptionsIntent =
        serde_json::from_value(json!({"type": "setPageSize", "pageSize": null})).expect("options");
    assert_eq!(
        i,
        super::options::OptionsIntent::SetPageSize { page_size: None }
    );
    let i: super::sheets::ResolveSheetIntent =
        serde_json::from_value(json!({"type": "setCode", "codeId": "other"})).expect("resolve");
    assert_eq!(
        i,
        super::sheets::ResolveSheetIntent::SetCode {
            code_id: Some("other".into())
        }
    );
    let i: super::menubar::MenuBarIntent =
        serde_json::from_value(json!({"type": "openMain"})).expect("menubar");
    assert_eq!(i, super::menubar::MenuBarIntent::OpenMain);
}

#[test]
fn records_selection_lists() {
    assert_eq!(
        SidebarItem::ALL.map(SidebarItem::raw),
        [
            "myTickets",
            "teamTickets",
            "configItems",
            "organizations",
            "people"
        ]
    );
    let _: &dyn Prefs = &MemoryPrefs::new();
}

#[tokio::test(start_paused = true)]
async fn list_64_intermediate_spinner_states_are_published() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    h.sink.take();
    h.load().await;
    let refreshing: Vec<bool> = h
        .sink
        .take()
        .into_iter()
        .filter(|(m, _)| m == "list:mine")
        .filter_map(|(_, v)| v["footerSpinner"].as_bool())
        .collect();
    assert_eq!(
        refreshing.first(),
        Some(&true),
        "the spinner shows while reloading"
    );
    assert_eq!(refreshing.last(), Some(&false));
}
