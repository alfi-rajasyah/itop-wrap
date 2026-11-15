//! The transport of contract.md §2: the three commands, the `snapshot` event and the `itop`
//! URI scheme. All of it is thin: models live behind [`Router`], snapshots in [`Hub`].

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, UriSchemeContext, UriSchemeResponder};

use crate::error::AppError;
use crate::hub::{Hub, SnapshotSink};
use crate::native::Native;
use crate::router::{Router, is_model_id};

/// Tauri managed state: what the commands talk to.
pub struct AppState {
    pub hub: Hub,
    pub router: Arc<dyn Router>,
    pub native: Arc<dyn Native>,
}

/// The `snapshot` event's payload.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotEvent<'a> {
    pub model: &'a str,
    pub snapshot: &'a Value,
}

/// The app's sink: emits `snapshot` to every window, and tells `on_session` about each new
/// `session` snapshot (the main window's size follows it, contract.md §6).
pub struct EventSink<R: Runtime> {
    app: AppHandle<R>,
    on_session: Box<dyn Fn(&Value) + Send + Sync>,
}

impl<R: Runtime> EventSink<R> {
    pub fn new(app: AppHandle<R>, on_session: impl Fn(&Value) + Send + Sync + 'static) -> Self {
        Self { app, on_session: Box::new(on_session) }
    }
}

impl<R: Runtime> SnapshotSink for EventSink<R> {
    fn emit(&self, model: &str, snapshot: &Value) {
        if let Err(e) = self.app.emit("snapshot", SnapshotEvent { model, snapshot }) {
            eprintln!("snapshot event for {model} wasn't sent: {e}");
        }
        if model == "session" {
            (self.on_session)(snapshot);
        }
    }
}

/// contract.md §2 `get_snapshot`: the latest snapshot, `null` when the model doesn't exist.
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>, model: String) -> Result<Value, AppError> {
    if !is_model_id(&model) {
        return Err(AppError::unknown_model(&model));
    }
    Ok(state.hub.get(&model))
}

/// contract.md §2 `dispatch`: resolves when the model has finished the intent.
#[tauri::command]
pub async fn dispatch(state: State<'_, AppState>, model: String, intent: Value) -> Result<Value, AppError> {
    if !is_model_id(&model) {
        return Err(AppError::unknown_model(&model));
    }
    let router = state.router.clone();
    router.dispatch(&model, intent).await
}

/// Whether `open_url` may open this: http, https and mailto only.
pub fn is_openable_url(url: &str) -> bool {
    match tauri::Url::parse(url) {
        Ok(u) => match u.scheme() {
            "http" | "https" => u.host_str().is_some_and(|h| !h.is_empty()),
            "mailto" => true,
            _ => false,
        },
        Err(_) => false,
    }
}

/// contract.md §2 `open_url`.
#[tauri::command]
pub fn open_url(state: State<'_, AppState>, url: String) -> Result<(), AppError> {
    if !is_openable_url(&url) {
        return Err(AppError::invalid_intent(format!("Only http, https and mailto links can be opened, not {url}.")));
    }
    state.native.open_url(&url);
    Ok(())
}

/// `itop://localhost/<kind>/<id>` → `(kind, id)`.
pub fn parse_asset_path(path: &str) -> Option<(&str, &str)> {
    let mut parts = path.trim_start_matches('/').splitn(2, '/');
    let kind = parts.next().filter(|k| !k.is_empty())?;
    let id = parts.next().filter(|i| !i.is_empty() && !i.contains('/'))?;
    matches!(kind, "inline-image" | "draft-image").then_some((kind, id))
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// The `itop` asynchronous URI scheme protocol: asks the router for the bytes.
pub fn itop_protocol<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let Some(state) = ctx.app_handle().try_state::<AppState>() else {
        responder.respond(not_found());
        return;
    };
    let router = state.router.clone();
    let path = request.uri().path().to_owned();
    tauri::async_runtime::spawn(async move {
        let found = match parse_asset_path(&path) {
            Some((kind, id)) => router.asset(kind, id).await,
            None => None,
        };
        let response = match found {
            Some((bytes, mime)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(header::CACHE_CONTROL, "max-age=3600")
                .body(bytes)
                .unwrap_or_else(|_| not_found()),
            None => not_found(),
        };
        responder.respond(response);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_allows_only_web_and_mail() {
        assert!(is_openable_url("https://itop.example.com/pages/UI.php?operation=details"));
        assert!(is_openable_url("http://itop.local/"));
        assert!(is_openable_url("mailto:jane@example.com"));
        assert!(!is_openable_url("file:///etc/passwd"));
        assert!(!is_openable_url("javascript:alert(1)"));
        assert!(!is_openable_url("itop://localhost/inline-image/1"));
        assert!(!is_openable_url("not a url"));
    }

    #[test]
    fn asset_paths() {
        assert_eq!(parse_asset_path("/inline-image/42"), Some(("inline-image", "42")));
        assert_eq!(parse_asset_path("/draft-image/3F2A-11"), Some(("draft-image", "3F2A-11")));
        assert_eq!(parse_asset_path("/other/42"), None);
        assert_eq!(parse_asset_path("/inline-image/"), None);
        assert_eq!(parse_asset_path("/inline-image/1/2"), None);
        assert_eq!(parse_asset_path("/"), None);
    }
}
