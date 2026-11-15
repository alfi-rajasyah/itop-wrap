//! iTop Wrap: the Tauri app. Models, commands, tray and native pieces.
//!
//! Start-up: [`run`] builds the Tauri app; in `setup` it creates the [`Hub`] (whose sink emits the
//! `snapshot` event), the native shell ([`native::mac::Shell`] / [`native::mac::MacNative`]), the
//! Keychain store and the settings, hands them to a router factory as an [`AppContext`], manages
//! [`commands::AppState`], creates the windows and sends [`NativeEvent::Launched`].

pub mod commands;
pub mod error;
pub mod hub;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod models;
pub mod native;
#[cfg(target_os = "macos")]
pub mod prefs;
pub mod router;

use std::sync::Arc;

use itop_kit::secret_store::SecretStore;
use tauri::{AppHandle, Manager, RunEvent, Wry};

use crate::commands::{AppState, EventSink};
use crate::hub::Hub;
use crate::native::Native;
use crate::native::mac::{MacNative, Shell};
use crate::prefs::Prefs;
pub use crate::router::{NativeEvent, Router, StubRouter};

/// Everything a router (the models) is built from.
pub struct AppContext {
    pub app: AppHandle<Wry>,
    /// Publish snapshots here (contract.md §2).
    pub hub: Hub,
    /// The Mac, behind the trait the models are tested with.
    pub native: Arc<dyn Native>,
    /// Session settings: server, user, list options, timer (the debug suite with `ITOPWRAP_SETTINGS`).
    /// Also an `itop_kit::SettingsStore`.
    pub prefs: Prefs,
    /// The standard domain, always (`newTicket.assignToMe`, PERS-8).
    pub standard_prefs: Prefs,
    /// The Keychain under "iTop Wrap" (or the debug service).
    pub secrets: Arc<dyn SecretStore>,
}

/// Runs the app with the stub router. The models agent replaces the marked line.
pub fn run() {
    run_with(|ctx| {
        // MODELS: replace this line with the real router, e.g. `Arc::new(models::AppRouter::new(ctx))`.
        Arc::new(StubRouter::new(ctx.hub, ctx.native)) as Arc<dyn Router>
    })
}

/// Runs the app with the router `make_router` builds from the [`AppContext`].
pub fn run_with(make_router: impl FnOnce(AppContext) -> Arc<dyn Router> + Send + 'static) {
    let mut make_router = Some(make_router);
    let app = tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("itop", commands::itop_protocol)
        .invoke_handler(tauri::generate_handler![commands::get_snapshot, commands::dispatch, commands::open_url])
        .on_menu_event(|app, event| {
            if let Some(shell) = app.try_state::<Arc<Shell>>() {
                shell.menu_event(event.id().as_ref());
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let shell = Shell::new(handle.clone());
            let sink_shell = shell.clone();
            let hub = Hub::new(Arc::new(EventSink::new(handle.clone(), move |s| sink_shell.session_snapshot(s))));
            let native: Arc<dyn Native> = Arc::new(MacNative::new(shell.clone()));
            let ctx = AppContext {
                app: handle.clone(),
                hub: hub.clone(),
                native: native.clone(),
                prefs: Prefs::session(),
                standard_prefs: Prefs::standard(),
                secrets: Arc::new(keychain::KeychainStore::new(prefs::keychain_service())),
            };
            let router = match make_router.take() {
                Some(make) => make(ctx),
                None => return Err("setup ran twice".into()),
            };
            shell.set_router(router.clone());
            app.manage(AppState { hub, router, native });
            app.manage(shell.clone());
            let mtm = objc2::MainThreadMarker::new().ok_or("setup isn't on the main thread")?;
            shell.create_windows(mtm)?;
            shell.send(NativeEvent::Launched);
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match app {
        Ok(app) => app,
        Err(e) => {
            eprintln!("iTop Wrap failed to start: {e}");
            std::process::exit(1);
        }
    };
    app.run(|app, event| {
        if let RunEvent::Reopen { has_visible_windows, .. } = event
            && let Some(shell) = app.try_state::<Arc<Shell>>()
        {
            shell.reopen(has_visible_windows);
        }
    });
}
