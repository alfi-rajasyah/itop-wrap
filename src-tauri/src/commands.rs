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
        Self {
            app,
            on_session: Box::new(on_session),
        }
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
pub async fn dispatch(
    state: State<'_, AppState>,
    model: String,
    intent: Value,
) -> Result<Value, AppError> {
    if !is_model_id(&model) {
        return Err(AppError::unknown_model(&model));
    }
    let router = state.router.clone();
    router.dispatch(&model, intent).await
}

/// The URL `open_url` may open, as parsed (the string handed to macOS): http and https with a
/// host and no user name or password (`https://itop.corp@evil.example` is refused), and mailto.
pub fn openable_url(url: &str) -> Option<tauri::Url> {
    let u = tauri::Url::parse(url).ok()?;
    let ok = match u.scheme() {
        "http" | "https" => {
            u.host_str().is_some_and(|h| !h.is_empty())
                && u.username().is_empty()
                && u.password().is_none()
        }
        "mailto" => true,
        _ => false,
    };
    ok.then_some(u)
}

/// Whether `open_url` may open this (see [`openable_url`]).
pub fn is_openable_url(url: &str) -> bool {
    openable_url(url).is_some()
}

/// contract.md §2 `open_url`.
#[tauri::command]
pub fn open_url(state: State<'_, AppState>, url: String) -> Result<(), AppError> {
    let Some(parsed) = openable_url(&url) else {
        return Err(AppError::invalid_intent(format!(
            "Only http, https and mailto links can be opened, not {url}."
        )));
    };
    state.native.open_url(parsed.as_str());
    Ok(())
}

/// Whether a web view may navigate to `url`: only the app's own pages (`tauri://localhost`, and
/// the Vite dev server in debug builds). Everything else — a dropped link or file, a remote
/// page — is refused, so no other page ever runs in an app window.
pub fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "about" => url.as_str() == "about:blank",
        "http" if cfg!(debug_assertions) => {
            url.host_str() == Some("localhost") && url.port() == Some(1420)
        }
        _ => false,
    }
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
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Image types an `itop://` asset may be served as. The server names the type of a pasted image,
/// so anything else (text/html, application/xhtml+xml, …) is only served if its bytes sniff as
/// PNG, JPEG or GIF, and then as that.
const IMAGE_TYPES: [&str; 11] = [
    "image/png",
    "image/jpeg",
    "image/jpg",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/tiff",
    "image/heic",
    "image/heif",
    "image/avif",
    "image/svg+xml",
];

/// The Content-Type to serve `bytes` with, or `None` (404) when they aren't an image.
pub fn asset_content_type(declared: &str, bytes: &[u8]) -> Option<String> {
    let essence = declared
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if IMAGE_TYPES.contains(&essence.as_str()) {
        return Some(essence);
    }
    if let Some((m, _)) = itop_kit::LogImage::sniff(bytes) {
        return Some(m.to_owned());
    }
    // The other raster types macOS shows, by their signatures (no declared type, as Swift's
    // `NSImage(data:)` would still have shown them).
    let at = |range: std::ops::Range<usize>| bytes.get(range);
    let sniffed = if at(0..4) == Some(b"RIFF") && at(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if at(0..4) == Some(b"II*\0") || at(0..4) == Some(b"MM\0*") {
        "image/tiff"
    } else if at(0..2) == Some(b"BM") {
        "image/bmp"
    } else if at(4..8) == Some(b"ftyp") {
        match at(8..12) {
            Some(b"avif" | b"avis") => "image/avif",
            Some(b"heic" | b"heix" | b"hevc" | b"heim" | b"heis") => "image/heic",
            Some(b"mif1" | b"msf1") => "image/heif",
            _ => return None,
        }
    } else {
        return None;
    };
    Some(sniffed.to_owned())
}

/// An asset response: the type pinned (`nosniff`), never a document that can run anything
/// (`sandbox`), no CORS (an `<img>` doesn't need it), private cache.
pub fn asset_response(bytes: Vec<u8>, declared: &str) -> Response<Vec<u8>> {
    let Some(mime) = asset_content_type(declared, &bytes) else {
        return not_found();
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; sandbox",
        )
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(bytes)
        .unwrap_or_else(|_| not_found())
}

/// The `itop` asynchronous URI scheme protocol: asks the router for the bytes.
pub fn itop_protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
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
            Some((bytes, mime)) => asset_response(bytes, &mime),
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
        assert!(is_openable_url(
            "https://itop.example.com/pages/UI.php?operation=details"
        ));
        assert!(is_openable_url("http://itop.local/"));
        assert!(is_openable_url("mailto:jane@example.com"));
        assert!(!is_openable_url("file:///etc/passwd"));
        assert!(!is_openable_url("javascript:alert(1)"));
        assert!(!is_openable_url("itop://localhost/inline-image/1"));
        assert!(!is_openable_url("not a url"));
        assert!(!is_openable_url("https://itop.corp@evil.example/"));
        assert!(!is_openable_url("https://user:pw@itop.example.com/"));
    }

    #[test]
    fn only_app_pages_load_in_the_web_views() {
        let u = |s: &str| tauri::Url::parse(s).unwrap();
        assert!(is_app_url(&u(
            "tauri://localhost/index.html?window=new-ticket"
        )));
        assert!(!is_app_url(&u("tauri://evil/index.html")));
        assert!(!is_app_url(&u("https://evil.example/login")));
        assert!(!is_app_url(&u("file:///Users/me/Downloads/page.html")));
        assert!(!is_app_url(&u("itop://localhost/inline-image/1")));
        assert_eq!(
            is_app_url(&u("http://localhost:1420/")),
            cfg!(debug_assertions)
        );
        assert!(!is_app_url(&u("http://localhost:8080/")));
    }

    #[test]
    fn open_url_hands_over_the_parsed_url() {
        // WHATWG drops tabs and newlines; macOS gets the parsed spelling, not the raw one.
        let u = openable_url("ht\ttps://itop.example.com/a b").expect("openable");
        assert_eq!(u.as_str(), "https://itop.example.com/a%20b");
    }

    #[test]
    fn assets_are_only_ever_images() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0];
        assert_eq!(
            asset_content_type("image/png", &png).as_deref(),
            Some("image/png")
        );
        assert_eq!(
            asset_content_type("IMAGE/WEBP; q=1", b"x").as_deref(),
            Some("image/webp")
        );
        // A server that says text/html gets the sniffed type, or a 404.
        assert_eq!(
            asset_content_type("text/html", &png).as_deref(),
            Some("image/png")
        );
        assert_eq!(asset_content_type("text/html", b"<script>"), None);
        assert_eq!(asset_content_type("", b"<svg/>"), None);
        assert_eq!(
            asset_content_type("application/octet-stream", b"RIFF\0\0\0\0WEBPVP8 ").as_deref(),
            Some("image/webp")
        );

        let ok = asset_response(png.to_vec(), "text/html");
        assert_eq!(ok.status(), StatusCode::OK);
        let h = ok.headers();
        assert_eq!(h[header::CONTENT_TYPE], "image/png");
        assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert!(
            h[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("sandbox")
        );
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
        assert_eq!(
            asset_response(b"<html>".to_vec(), "text/html").status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn asset_paths() {
        assert_eq!(
            parse_asset_path("/inline-image/42"),
            Some(("inline-image", "42"))
        );
        assert_eq!(
            parse_asset_path("/draft-image/3F2A-11"),
            Some(("draft-image", "3F2A-11"))
        );
        assert_eq!(parse_asset_path("/other/42"), None);
        assert_eq!(parse_asset_path("/inline-image/"), None);
        assert_eq!(parse_asset_path("/inline-image/1/2"), None);
        assert_eq!(parse_asset_path("/"), None);
    }
}
