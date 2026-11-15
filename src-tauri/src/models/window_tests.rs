//! The main window hiding and coming back (POLL-2/3/5, LIST-22, TMR-15), and a stopped
//! workspace staying quiet (sign-out while work is in flight).

use std::time::Duration;

use serde_json::{Value, json};

use super::app_router::route_workspace;
use super::records::ListScope;
use super::testkit::{FakeITop, Harness, ticket};
use crate::native::fake::Call;

async fn send(h: &Harness, model: &str, intent: Value) -> Value {
    let r = route_workspace(&h.ws, model, intent)
        .await
        .expect("dispatch");
    h.settle().await;
    r
}

/// Requests whose OQL lists Team tickets.
fn team_loads(h: &Harness) -> usize {
    h.stub
        .requests()
        .iter()
        .filter(|r| {
            r.json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("team_id IN") && !k.contains("agent_id ="))
        })
        .count()
}

fn mine_loads(h: &Harness) -> usize {
    h.stub
        .requests()
        .iter()
        .filter(|r| {
            r.json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("agent_id ="))
        })
        .count()
}

/// Every request that reads one ticket by key (the detail load and its log).
fn reads_of(h: &Harness, key: &str) -> usize {
    h.stub
        .requests()
        .iter()
        .filter(|r| r.json_data().to_string().contains(&format!("id = {key}")))
        .count()
}

#[tokio::test(start_paused = true)]
async fn hidden_window_stops_team_polling_and_resumes_on_show() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "9")]));
    h.ws.start();
    h.settle().await;
    send(
        &h,
        "shell",
        json!({"type": "select", "item": "teamTickets"}),
    )
    .await;
    assert_eq!(team_loads(&h), 2, "loads on appear");

    h.ws.main_window_hidden();
    h.settle().await;
    let mine_before = mine_loads(&h);
    tokio::time::sleep(Duration::from_secs(250)).await;
    h.settle().await;
    assert_eq!(team_loads(&h), 2, "no Team poll while the window is closed");
    assert!(
        mine_loads(&h) > mine_before,
        "My tickets keeps its 120 s refresh while hidden"
    );

    h.ws.main_window_shown();
    h.settle().await;
    assert_eq!(team_loads(&h), 2, "loaded already: no load on appear");
    tokio::time::sleep(Duration::from_secs(121)).await;
    h.settle().await;
    assert_eq!(team_loads(&h), 4, "the poll runs again");
    h.ws.stop();
}

#[tokio::test(start_paused = true)]
async fn hidden_window_drops_the_detail_and_show_reloads_it() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    send(
        &h,
        "list:mine",
        json!({"type": "select", "ticketId": "UserRequest::1"}),
    )
    .await;
    assert_eq!(h.snap("detail:mine")["type"], "ticket");
    let reads = reads_of(&h, "1");
    assert!(reads > 0, "the detail loaded");

    h.ws.main_window_hidden();
    h.settle().await;
    assert_eq!(h.snap("detail:mine")["type"], "none", "its views went away");

    h.ws.main_window_shown();
    h.settle().await;
    assert_eq!(h.snap("detail:mine")["type"], "ticket");
    assert!(reads_of(&h, "1") > reads, "the shown detail loads again");
}

#[tokio::test(start_paused = true)]
async fn selecting_a_section_while_hidden_waits_for_the_window() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "9")]));
    h.load().await;
    h.ws.main_window_hidden();
    send(
        &h,
        "shell",
        json!({"type": "select", "item": "teamTickets"}),
    )
    .await;
    assert_eq!(
        team_loads(&h),
        0,
        "nothing appears while the window is closed"
    );
    h.ws.main_window_shown();
    h.settle().await;
    assert_eq!(team_loads(&h), 2, "Team tickets appears with the window");
    h.ws.stop();
}

/// Review finding O: sign-out while a reload is in flight must not bring the old user's
/// snapshots or tray back.
#[tokio::test(start_paused = true)]
async fn a_stopped_workspace_publishes_nothing() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    let pending = tokio::spawn(h.ws.list_refresh(ListScope::Mine));
    let tray_calls = |h: &Harness| {
        h.native
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::SetTray(..)))
            .count()
    };
    h.ws.stop();
    h.ws.clear_snapshots();
    let trays = tray_calls(&h);
    let _ = pending.await;
    h.settle().await;
    h.ws.publish();
    for id in super::workspace::WORKSPACE_MODEL_IDS {
        assert_eq!(h.hub.get(id), Value::Null, "{id} stays cleared");
    }
    assert_eq!(tray_calls(&h), trays, "the tray isn't set again");
}

/// Review finding S: a lookup superseded by a new query writes nothing, failure included.
#[tokio::test(start_paused = true)]
async fn a_superseded_failing_lookup_writes_nothing() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.load().await;
    *h.itop.offline.lock().unwrap_or_else(|e| e.into_inner()) = true;
    let ws = h.ws.clone();
    ws.lock().lists[0].query = "R-000999".to_owned();
    let lookup = tokio::spawn({
        let ws = ws.clone();
        async move { ws.search_changed(ListScope::Mine).await }
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    ws.lock().lists[0].query = "something else".to_owned();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = lookup.await;
    assert!(
        !matches!(ws.lock().lists[0].lookup(), super::list::Lookup::Failed(_)),
        "the failure of a superseded lookup isn't shown"
    );
}

const PNG: [u8; 33] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
    2, 0, 0, 0, 3, 8, 6, 0, 0, 0, 0, 0, 0, 0,
];

async fn shown_detail(h: &Harness) {
    h.load().await;
    send(
        h,
        "list:mine",
        json!({"type": "select", "ticketId": "UserRequest::1"}),
    )
    .await;
}

/// LOG-14: ⌘V asks Rust to read the pasteboard; images go to the intake, otherwise the UI
/// pastes text normally.
#[tokio::test(start_paused = true)]
async fn log_14_paste_reads_the_pasteboard() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    shown_detail(&h).await;
    let r = send(&h, "detail:mine", json!({"type": "pasteFromClipboard"})).await;
    assert_eq!(r, json!("text"), "no images: paste text");
    assert!(h.native.calls().contains(&Call::ReadPasteboard));

    *h.native
        .pasteboard
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(vec![
        crate::native::PastedImage {
            filename: "shot.png".into(),
            data: Some(PNG.to_vec()),
            size: 33,
        },
        crate::native::PastedImage {
            filename: "huge.png".into(),
            data: None,
            size: 50 * 1_048_576,
        },
    ]);
    let r = send(&h, "detail:mine", json!({"type": "pasteFromClipboard"})).await;
    assert_eq!(r, json!("images"));
    let c = &h.snap("detail:mine")["composer"];
    assert_eq!(c["images"].as_array().map(Vec::len), Some(1));
    assert_eq!(c["images"][0]["filename"], "shot.png");
    assert_eq!(c["status"]["text"], "huge.png is larger than 4 MB.");
}

/// DET-34: a failed write shows Foundation's own description, not an io::Error.
#[tokio::test(start_paused = true)]
async fn det_34_save_failure_reads_like_foundation() {
    let h = Harness::new(FakeITop::with(vec![ticket("1", "assigned", "7")]));
    h.itop.can(
        "Attachment",
        super::testkit::objects(vec![super::testkit::object(
            "Attachment",
            "3",
            json!({"friendlyname": "vpn.log", "contents": {"data": "aGVsbG8=", "filename": "vpn.log", "mimetype": "text/plain"}}),
        )]),
    );
    shown_detail(&h).await;
    *h.native.save_to.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(std::path::PathBuf::from("/x/vpn.log"));
    let why = "You don’t have permission to save the file “vpn.log” in the folder “x”.";
    *h.native
        .write_failure
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(why.into());
    send(
        &h,
        "detail:mine",
        json!({"type": "downloadAttachment", "attachmentId": "3"}),
    )
    .await;
    assert_eq!(
        h.snap("detail:mine")["attachments"]["error"],
        format!("Couldn't download vpn.log. {why}")
    );
}
