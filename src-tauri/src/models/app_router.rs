//! The app's model registry: the session model (always) and the workspace (while signed in),
//! behind [`crate::router::Router`]. Port of `SessionModel`'s workspace handling, `RootView` and
//! the App scene's commands (W/App.swift).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use itop_kit::{ITopError, Session, SessionError, SessionService, SettingsStore, TransportError};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::format::Fmt;
use super::new_ticket::NewTicketStart;
use super::prefs::Prefs;
use super::records::{ListScope, SidebarItem};
use super::seams::{Model, ModelContext};
use super::session::{FocusField, SessionIntent, SessionModelState, SessionState, SignInResult};
use super::workspace::{WORKSPACE_MODEL_IDS, Workspace};
use crate::error::AppError;
use crate::hub::Hub;
use crate::native::Native;
use crate::router::{NativeEvent, Router};

/// What the app is built from. `run()` fills it with the real Keychain, defaults store, Tauri
/// hub sink and native layer; tests with in-memory ones.
pub struct AppConfig {
    /// Sign-in, restore and sign-out (see [`session_service`]).
    pub service: SessionService,
    /// The settings store (UserDefaults): View options and the running timer.
    pub prefs: Arc<dyn Prefs>,
    pub hub: Hub,
    pub native: Arc<dyn Native>,
    /// Dates and sizes (en_US, the Mac's time zone in the app).
    pub fmt: Fmt,
    /// `ITOPWRAP_TRACE=1` in debug builds.
    pub trace: bool,
}

/// The real `SessionService`: the system's TLS through reqwest, the given Keychain and settings,
/// and the write access the build and its debug switches allow (checklist section 17).
pub fn session_service(
    secrets: Arc<dyn itop_kit::SecretStore>,
    settings: Arc<dyn SettingsStore>,
    env: &HashMap<String, String>,
) -> Result<SessionService, TransportError> {
    let switches = super::debug::switches(env, !cfg!(debug_assertions));
    Ok(
        SessionService::new(secrets, itop_kit::SessionSettings::new(settings))?
            .with_write_access(switches.write_access),
    )
}

struct RouterInner {
    service: SessionService,
    prefs: Arc<dyn Prefs>,
    hub: Hub,
    native: Arc<dyn Native>,
    fmt: Fmt,
    trace: bool,
    session: Mutex<SessionModelState>,
    workspace: Mutex<Option<Workspace>>,
}

/// Routes intents to the models, serves the `itop://` assets and handles native events.
#[derive(Clone)]
pub struct AppRouter(Arc<RouterInner>);

fn decode<T: DeserializeOwned>(intent: Value) -> Result<T, AppError> {
    serde_json::from_value(intent).map_err(AppError::invalid_intent)
}

fn is_known(model: &str) -> bool {
    model == "session" || WORKSPACE_MODEL_IDS.contains(&model)
}

impl AppRouter {
    /// Starts in `restoring`; send [`NativeEvent::Launched`] to run the restore (AUTH-1).
    pub fn new(config: AppConfig) -> AppRouter {
        let session =
            SessionModelState::new(config.service.saved_server(), config.service.saved_user());
        let router = AppRouter(Arc::new(RouterInner {
            service: config.service,
            prefs: config.prefs,
            hub: config.hub,
            native: config.native,
            fmt: config.fmt,
            trace: config.trace,
            session: Mutex::new(session),
            workspace: Mutex::new(None),
        }));
        router.publish_session();
        router
    }

    fn session(&self) -> MutexGuard<'_, SessionModelState> {
        self.0.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The signed-in workspace, if any.
    pub fn workspace(&self) -> Option<Workspace> {
        self.0
            .workspace
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn publish_session(&self) {
        let snapshot = self.session().snapshot();
        self.0.hub.publish("session", &snapshot);
    }

    /// The `state` setter with its `didSet { updateWorkspace() }`.
    fn set_state(&self, state: SessionState) {
        let (was_in, now_in, session) = {
            let mut s = self.session();
            let was_in = matches!(s.state, SessionState::SignedIn(_));
            s.set_state(state);
            let session = match &s.state {
                SessionState::SignedIn(session) => Some(session.clone()),
                _ => None,
            };
            (was_in, session.is_some(), session)
        };
        self.update_workspace(session);
        self.publish_session();
        if was_in != now_in {
            self.0.native.session_changed(now_in);
        }
    }

    /// `updateWorkspace()`: the same user on the same server keeps the workspace (AUTH-32); any
    /// other session builds a new one; signed out there is none.
    fn update_workspace(&self, session: Option<Arc<Session>>) {
        let mut slot = self.0.workspace.lock().unwrap_or_else(|e| e.into_inner());
        match session {
            Some(session) => {
                if let Some(ws) = slot.as_ref() {
                    let same = ws.session().user.user_id == session.user.user_id
                        && ws.session().client.server == session.client.server;
                    if same {
                        return;
                    }
                    ws.stop();
                    ws.clear_snapshots();
                }
                let ctx = ModelContext {
                    session,
                    hub: self.0.hub.gated(),
                    native: self.0.native.clone(),
                };
                let ws = Workspace::new(ctx, self.0.fmt, self.0.prefs.clone(), self.0.trace);
                ws.start();
                *slot = Some(ws);
            }
            None => {
                if let Some(ws) = slot.take() {
                    ws.stop();
                    ws.clear_snapshots();
                }
            }
        }
    }

    /// `restore()` (AUTH-1…5). It doesn't go back to `restoring` on "Try again".
    pub async fn restore(&self) {
        let result = self.0.service.restore().await;
        self.set_state(SessionModelState::restored(result));
    }

    /// `LoginView.submit` + `SessionModel.signIn(password:)` (AUTH-14…21).
    async fn sign_in(&self, password: &str) -> SignInResult {
        let (server, user) = {
            let mut s = self.session();
            if s.form.signing {
                return SignInResult {
                    ok: false,
                    focus: None,
                };
            }
            s.form.error = None;
            let user_empty = super::list::trim_spaces(&s.user).is_empty();
            s.form.missing_fields = user_empty || password.is_empty();
            if s.form.missing_fields {
                drop(s);
                self.publish_session();
                return SignInResult {
                    ok: false,
                    focus: Some(if user_empty {
                        FocusField::User
                    } else {
                        FocusField::Password
                    }),
                };
            }
            s.form.signing = true;
            (s.server.clone(), s.user.clone())
        };
        self.publish_session();
        match self.0.service.sign_in(&server, &user, password).await {
            Ok(session) => {
                {
                    let mut s = self.session();
                    s.server = session.client.server.base_url().to_owned();
                }
                // `selection = .myTickets`: a new workspace starts there; a kept one is switched.
                let kept = self.workspace();
                self.set_state(SessionState::SignedIn(Arc::new(session)));
                if let (Some(old), Some(now)) = (kept, self.workspace())
                    && Arc::ptr_eq(&old.0, &now.0)
                {
                    now.shell_select(SidebarItem::MyTickets).await;
                }
                self.session().form.signing = false;
                self.publish_session();
                SignInResult {
                    ok: true,
                    focus: None,
                }
            }
            Err(e) => {
                let error = match e {
                    SessionError::ITop(e) => e,
                    SessionError::Keychain(_) => ITopError::UnexpectedResponse,
                };
                let focus =
                    (error == ITopError::InvalidCredentials).then_some(FocusField::Password);
                {
                    let mut s = self.session();
                    s.form.error = Some(error);
                    s.form.signing = false;
                }
                self.publish_session();
                SignInResult { ok: false, focus }
            }
        }
    }

    /// `signOut()` (AUTH-30, AUTH-31): the Keychain item goes (errors swallowed); the timer stays
    /// saved.
    fn sign_out(&self) {
        let session = match &self.session().state {
            SessionState::SignedIn(s) => Some(s.clone()),
            _ => None,
        };
        if let Some(session) = session {
            let _ = self.0.service.sign_out(&session);
        }
        self.set_state(SessionState::SignedOut { notice: None });
    }

    async fn dispatch_session(&self, intent: SessionIntent) -> Result<Value, AppError> {
        match intent {
            SessionIntent::SetServer { server } => {
                self.session().server = server;
                self.publish_session();
            }
            SessionIntent::SetUser { user } => {
                self.session().user = user;
                self.publish_session();
            }
            SessionIntent::SignIn { password } => {
                let result = self.sign_in(&password).await;
                return serde_json::to_value(result).map_err(AppError::internal);
            }
            SessionIntent::RetryRestore => self.restore().await,
            SessionIntent::SignInWithAnotherAccount => {
                self.set_state(SessionState::SignedOut { notice: None })
            }
            SessionIntent::SignOut => self.sign_out(),
        }
        Ok(Value::Null)
    }
}

/// Routes an intent to a workspace model (every id but `session`).
pub(crate) async fn route_workspace(
    ws: &Workspace,
    model: &str,
    intent: Value,
) -> Result<Value, AppError> {
    {
        let scope_of = |id: &str| -> Result<ListScope, AppError> {
            ListScope::from_raw(id.split_once(':').map_or("", |(_, s)| s))
                .ok_or_else(|| AppError::unknown_model(model))
        };
        match model {
            "shell" => ws.dispatch_shell(decode(intent)?).await,
            "options" => ws.dispatch_options(decode(intent)?).await,
            "timer" => ws.dispatch_timer(decode(intent)?).await,
            "menubar" => ws.dispatch_menubar(decode(intent)?).await,
            m if m.starts_with("list:") => ws.dispatch_list(scope_of(m)?, decode(intent)?).await,
            m if m.starts_with("detail:") => {
                ws.dispatch_detail(scope_of(m)?, decode(intent)?).await
            }
            m if m.starts_with("assign:") => {
                ws.dispatch_assign(scope_of(m)?, decode(intent)?).await
            }
            m if m.starts_with("pending:") => {
                ws.dispatch_pending(scope_of(m)?, decode(intent)?).await
            }
            "resolve:window" => {
                ws.dispatch_resolve(super::sheets::ResolveAt::Window, decode(intent)?)
                    .await
            }
            m if m.starts_with("resolve:") => {
                ws.dispatch_resolve(
                    super::sheets::ResolveAt::Sheet(scope_of(m)?),
                    decode(intent)?,
                )
                .await
            }
            "newTicket:sheet" | "newTicket:window" | "cmdb" | "orgs" | "people" => {
                let Some(s) = ws.standalone().cloned() else {
                    return Err(AppError::internal(format!("{model} doesn't exist.")));
                };
                match model {
                    "newTicket:sheet" | "newTicket:window" => {
                        s.new_tickets.dispatch(model, intent).await
                    }
                    "cmdb" => s.cmdb.dispatch(model, intent).await,
                    "orgs" => s.orgs.dispatch(model, intent).await,
                    _ => s.people.dispatch(model, intent).await,
                }
            }
            other => Err(AppError::unknown_model(other)),
        }
    }
}

#[async_trait::async_trait]
impl Router for AppRouter {
    async fn dispatch(&self, model: &str, intent: Value) -> Result<Value, AppError> {
        if !is_known(model) {
            return Err(AppError::unknown_model(model));
        }
        if model == "session" {
            // serde's message can quote the value it refused, which may be the password.
            let intent = serde_json::from_value(intent)
                .map_err(|_| AppError::invalid_intent("That isn't a session intent."))?;
            return self.dispatch_session(intent).await;
        }
        let Some(ws) = self.workspace() else {
            return Err(AppError::not_signed_in());
        };
        route_workspace(&ws, model, intent).await
    }

    async fn asset(&self, kind: &str, id: &str) -> Option<(Vec<u8>, String)> {
        let ws = self.workspace()?;
        match kind {
            // DET-44, DET-48: through REST by id, the last 60 cached.
            "inline-image" => {
                let image = ws.0.images.image(id).await.ok()?;
                let mime = if image.mimetype.is_empty() {
                    itop_kit::LogImage::sniff(&image.data)
                        .map_or("application/octet-stream", |(m, _)| m)
                        .to_owned()
                } else {
                    image.mimetype
                };
                Some((image.data, mime))
            }
            // A composer image's bytes, from any list's drafts.
            "draft-image" => {
                let st = ws.lock();
                st.lists
                    .iter()
                    .flat_map(|l| l.draft_images.values().flatten())
                    .find(|im| im.id == id)
                    .map(|im| (im.data.clone(), im.mimetype.clone()))
            }
            _ => None,
        }
    }

    async fn native_event(&self, event: NativeEvent) {
        let ws = self.workspace();
        match event {
            NativeEvent::Launched => {
                if !matches!(self.session().state, SessionState::SignedIn(_)) {
                    self.restore().await;
                }
            }
            // File ▸ New Ticket… (NEW-1): main window forward, New ticket as a sheet there.
            // The shell has already brought the main window forward.
            NativeEvent::MenuNewTicket => {
                if let Some(ws) = ws {
                    ws.request_new_ticket(NewTicketStart::default()).await;
                }
            }
            NativeEvent::MenuOpenMain => self.0.native.show_main_window(),
            // View ▸ My tickets … People (⌘1…5, KEY-2).
            NativeEvent::MenuSelectSection(index) => {
                if let (Some(ws), Some(&item)) = (ws, SidebarItem::ALL.get(index)) {
                    ws.shell_select(item).await;
                }
            }
            NativeEvent::PanelNewTicket => {
                if let Some(ws) = ws {
                    let _ = ws
                        .dispatch_menubar(super::menubar::MenuBarIntent::NewTicket)
                        .await;
                }
            }
            NativeEvent::PanelOpenMain => {
                if let Some(ws) = ws {
                    let _ = ws
                        .dispatch_menubar(super::menubar::MenuBarIntent::OpenMain)
                        .await;
                }
            }
            // ⌘R is the footer's Refresh button, which is disabled while My tickets reloads
            // (MB-16): nothing is queued then.
            NativeEvent::PanelRefresh => {
                if let Some(ws) = ws
                    && !ws.lock().lists[0].refreshing
                {
                    let _ = ws
                        .dispatch_menubar(super::menubar::MenuBarIntent::Refresh)
                        .await;
                }
            }
            NativeEvent::MainWindowHidden => {
                if let Some(ws) = ws {
                    ws.main_window_hidden();
                }
            }
            NativeEvent::ToggleSidebar => {
                if let Some(ws) = ws {
                    ws.shell_toggle_sidebar();
                }
            }
            NativeEvent::MainWindowShown => {
                if let Some(ws) = ws {
                    ws.main_window_shown();
                }
            }
            NativeEvent::PanelShown => {
                if let Some(ws) = ws {
                    ws.publish();
                }
            }
            // The New Ticket window's close button: no draft kept (NEW-21).
            NativeEvent::ResolveWindowClosed => {
                if let Some(ws) = ws {
                    let _ = ws
                        .dispatch_resolve(
                            super::sheets::ResolveAt::Window,
                            super::sheets::ResolveSheetIntent::Close,
                        )
                        .await;
                }
            }
            NativeEvent::NewTicketWindowClosed => {
                if let Some(s) = ws.as_ref().and_then(|w| w.standalone()).cloned() {
                    let _ = s
                        .new_tickets
                        .dispatch("newTicket:window", serde_json::json!({"type": "dismiss"}))
                        .await;
                }
            }
        }
    }
}
