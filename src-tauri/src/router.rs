//! The seam between the native shell and the models (contract.md §2, §6). The shell owns windows,
//! menus, the tray and the Tauri commands; everything it can't decide on its own goes through a
//! [`Router`]. `models::AppRouter` implements `Router`.

use serde_json::Value;

use crate::error::AppError;

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
    /// The main window was closed while signed in (it only hides, SHELL-16) / came back (Dock,
    /// ⌘O, the panel, a ticket opened from the panel). The workspace's main-window views
    /// disappear and reappear like SwiftUI's.
    MainWindowHidden,
    MainWindowShown,
    /// View ▸ Hide Sidebar / Show Sidebar (⌃⌘S).
    ToggleSidebar,
    /// The menu bar panel was shown (MB: it refreshes/reads state on open if the Swift app does).
    PanelShown,
    /// The New Ticket window's close button (contract: dispatches dismiss without a draft).
    /// Not sent when the shell closes the window itself (`Native::close_new_ticket_window`).
    NewTicketWindowClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_are_the_contracts_twenty() {
        assert_eq!(MODEL_IDS.len(), 20);
        assert!(is_model_id("list:mine"));
        assert!(!is_model_id("list:other"));
    }
}
