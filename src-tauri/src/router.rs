//! What the Tauri layer (commands, the `itop` URI scheme, menus, tray, windows) calls on the app's
//! models. `models::AppRouter` implements it; the native layer only sees this trait.

use crate::error::AppError;

#[async_trait::async_trait]
pub trait Router: Send + Sync {
    async fn dispatch(&self, model: &str, intent: serde_json::Value) -> Result<serde_json::Value, AppError>;
    /// Bytes and mimetype for `itop://localhost/<kind>/<id>` (kinds: "inline-image", "draft-image"); None = 404.
    async fn asset(&self, kind: &str, id: &str) -> Option<(Vec<u8>, String)>;
    async fn native_event(&self, event: NativeEvent);
}

pub enum NativeEvent { Launched, MenuNewTicket, MenuOpenMain, MenuSelectSection(usize), PanelNewTicket, PanelOpenMain, PanelRefresh, PanelShown, NewTicketWindowClosed }
