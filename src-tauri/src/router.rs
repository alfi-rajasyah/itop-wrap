//! The seam between the native shell and the models (contract.md §2, §6). The shell owns windows,
//! menus, the tray and the Tauri commands; everything it can't decide on its own goes through a
//! [`Router`]. The models agent implements `Router`; until then [`StubRouter`] stands in.

use std::sync::Arc;

use serde_json::{Value, json};

use crate::error::AppError;
use crate::hub::Hub;
use crate::native::Native;

/// Every model id the contract lists (contract.md §3). `get_snapshot` and `dispatch` reject other
/// ids with `unknownModel` before the router sees them.
pub const MODEL_IDS: [&str; 20] = [
    "session",
    "shell",
    "options",
    "list:mine",
    "list:team",
    "detail:mine",
    "detail:team",
    "assign:mine",
    "assign:team",
    "pending:mine",
    "pending:team",
    "resolve:mine",
    "resolve:team",
    "newTicket:sheet",
    "newTicket:window",
    "timer",
    "cmdb",
    "orgs",
    "people",
    "menubar",
];

pub fn is_model_id(id: &str) -> bool {
    MODEL_IDS.contains(&id)
}

#[async_trait::async_trait]
pub trait Router: Send + Sync {
    /// contract.md §2 `dispatch`.
    async fn dispatch(&self, model: &str, intent: Value) -> Result<Value, AppError>;
    /// Bytes and mimetype for `itop://localhost/<kind>/<id>` (kinds: "inline-image", "draft-image"); None = 404.
    async fn asset(&self, kind: &str, id: &str) -> Option<(Vec<u8>, String)>;
    /// Something native happened that the models decide about.
    async fn native_event(&self, event: NativeEvent);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeEvent {
    /// The app finished launching (restore the session, start loops).
    Launched,
    /// File ▸ New Ticket… (⌘N) with the main window key, or no window. The shell has already
    /// brought the main window forward and activated the app; the router opens `newTicket:sheet`.
    MenuNewTicket,
    /// File ▸ Open iTop Wrap (⌘O). The shell has already brought the main window forward.
    MenuOpenMain,
    /// View ▸ item n (⌘1…⌘5), 0-based, sidebar order.
    MenuSelectSection(usize),
    /// ⌘N / ⌘O / ⌘R while the menu bar panel is the key window (KEY-8). The shell catches these
    /// keys before the web view, so the panel's React code never sees them.
    PanelNewTicket,
    PanelOpenMain,
    PanelRefresh,
    /// The menu bar panel was shown (MB: it refreshes/reads state on open if the Swift app does).
    PanelShown,
    /// The New Ticket window's close button (contract: dispatches dismiss without a draft).
    /// Not sent when the shell closes the window itself (`Native::close_new_ticket_window`).
    NewTicketWindowClosed,
}

/// Stands in for the models until they exist: `session` is `restoring` forever, every intent is
/// logged and answered with `null` (`session`) or `notSignedIn` (everything else).
///
/// Debug builds only: `ITOPWRAP_NATIVE_DEMO=1` pretends to be signed in so the tray, the panel and
/// the signed-in window size can be checked without the models: `session` becomes `signedIn`, the
/// tray appears and its title runs a fake timer that starts after 3 s and stops after 13 s, with a
/// count of 3 (so the slide out and back in can be watched).
pub struct StubRouter {
    hub: Hub,
    native: Arc<dyn Native>,
}

impl StubRouter {
    pub fn new(hub: Hub, native: Arc<dyn Native>) -> Self {
        hub.publish("session", &json!({ "type": "restoring" }));
        Self { hub, native }
    }

    #[cfg(debug_assertions)]
    fn native_demo(&self) {
        if std::env::var("ITOPWRAP_NATIVE_DEMO").as_deref() != Ok("1") {
            return;
        }
        eprintln!("stub router: ITOPWRAP_NATIVE_DEMO=1, pretending to be signed in");
        self.hub.publish("session", &json!({ "type": "signedIn", "displayName": "Demo Agent", "host": "itop.example.com" }));
        self.native.session_changed(true);
        let native = self.native.clone();
        tauri::async_runtime::spawn(async move {
            let mut second: u64 = 0;
            loop {
                let (title, label) = match second {
                    3..=12 => {
                        let t = second - 3;
                        (format!("00:00:{t:02}  3"), "iTop Wrap: 3 tickets need you, timer 0:00 on R-000412".to_owned())
                    }
                    _ => ("3".to_owned(), "iTop Wrap: 3 tickets need you".to_owned()),
                };
                native.set_tray(&title, &label);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                second = (second + 1) % 16;
            }
        });
    }
}

#[async_trait::async_trait]
impl Router for StubRouter {
    async fn dispatch(&self, model: &str, intent: Value) -> Result<Value, AppError> {
        eprintln!("stub router: dispatch {model} {intent}");
        if model == "session" { Ok(Value::Null) } else { Err(AppError::not_signed_in()) }
    }

    async fn asset(&self, kind: &str, id: &str) -> Option<(Vec<u8>, String)> {
        eprintln!("stub router: asset {kind}/{id}");
        None
    }

    async fn native_event(&self, event: NativeEvent) {
        eprintln!("stub router: native event {event:?}");
        match event {
            #[cfg(debug_assertions)]
            NativeEvent::Launched => self.native_demo(),
            NativeEvent::PanelNewTicket => {
                self.native.hide_menubar_panel();
                self.native.show_new_ticket_window();
            }
            NativeEvent::PanelOpenMain => {
                self.native.hide_menubar_panel();
                self.native.show_main_window();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::NoSink;
    use crate::native::fake::{Call, FakeNative};

    fn stub() -> (StubRouter, Arc<FakeNative>, Hub) {
        let hub = Hub::new(Arc::new(NoSink));
        let native = Arc::new(FakeNative::new(chrono::Utc::now()));
        (StubRouter::new(hub.clone(), native.clone()), native, hub)
    }

    #[test]
    fn model_ids_are_the_contracts_twenty() {
        assert_eq!(MODEL_IDS.len(), 20);
        assert!(is_model_id("list:mine"));
        assert!(!is_model_id("list:other"));
    }

    #[tokio::test]
    async fn stub_restores_forever() {
        let (router, _, hub) = stub();
        assert_eq!(hub.get("session"), json!({ "type": "restoring" }));
        assert_eq!(router.dispatch("session", json!({ "type": "retryRestore" })).await, Ok(Value::Null));
        assert_eq!(router.dispatch("timer", json!({ "type": "stop" })).await, Err(AppError::not_signed_in()));
        assert_eq!(router.asset("inline-image", "1").await, None);
    }

    #[tokio::test]
    async fn stub_panel_shortcuts_open_windows() {
        let (router, native, _) = stub();
        router.native_event(NativeEvent::PanelOpenMain).await;
        assert_eq!(native.calls(), vec![Call::HideMenubarPanel, Call::ShowMainWindow]);
    }
}
