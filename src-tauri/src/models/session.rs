//! Port of `SessionModel.swift` + `LoginView`'s submit and message rules + `RootView`
//! (AUTH-1…33). The session model lives for the whole app; it creates and drops the workspace.

use itop_kit::{ITopError, SessionError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum LoginMessage {
    MissingFields { text: String },
    RestRefused { bold: String, rest: String },
    Error { text: String },
    Notice { text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum LoginField {
    Server,
    User,
    Password,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct LoginForm {
    pub server: String,
    pub user: String,
    pub http_warning: bool,
    pub signing_in: bool,
    pub message: Option<LoginMessage>,
    pub initial_focus: LoginField,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum SessionSnapshot {
    Restoring,
    SignedOut {
        form: LoginForm,
    },
    Unreachable {
        message: String,
    },
    SignedIn {
        #[serde(rename = "displayName")]
        #[ts(rename = "displayName")]
        display_name: String,
        host: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum SessionIntent {
    SetServer { server: String },
    SetUser { user: String },
    SignIn { password: String },
    RetryRestore,
    SignInWithAnotherAccount,
    SignOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum FocusField {
    User,
    Password,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct SignInResult {
    pub ok: bool,
    pub focus: Option<FocusField>,
}

/// `SessionModel.State`.
#[derive(Clone, Debug)]
pub enum SessionState {
    Restoring,
    SignedOut { notice: Option<String> },
    SignedIn(std::sync::Arc<itop_kit::Session>),
    Unreachable(String),
}

/// The sign-in form's own state (`LoginView`'s `@State`), new each time the form appears.
#[derive(Clone, Debug)]
pub struct FormState {
    pub signing: bool,
    pub missing_fields: bool,
    pub error: Option<ITopError>,
    pub initial_focus: LoginField,
}

pub struct SessionModelState {
    pub state: SessionState,
    pub server: String,
    pub user: String,
    pub form: FormState,
}

/// AUTH-12: Server URL if its text is just "https://", else Username if empty, else Password.
pub fn initial_focus(server: &str, user: &str) -> LoginField {
    if server.chars().count() <= "https://".len() {
        LoginField::Server
    } else if user.is_empty() {
        LoginField::User
    } else {
        LoginField::Password
    }
}

/// `trimmingCharacters(in: .whitespaces)`.
fn trim_spaces(s: &str) -> &str {
    super::list::trim_spaces(s)
}

impl SessionModelState {
    pub fn new(saved_server: Option<String>, saved_user: Option<String>) -> Self {
        let server = saved_server.unwrap_or_else(|| "https://".to_owned());
        let user = saved_user.unwrap_or_default();
        let form = FormState { signing: false, missing_fields: false, error: None, initial_focus: initial_focus(&server, &user) };
        SessionModelState { state: SessionState::Restoring, server, user, form }
    }

    /// The `LoginView` message, in priority order (AUTH-15…19).
    pub fn message(&self, notice: Option<&str>) -> Option<LoginMessage> {
        if self.form.missing_fields {
            return Some(LoginMessage::MissingFields { text: "Enter your username and password.".to_owned() });
        }
        match &self.form.error {
            Some(ITopError::RestNotAllowed) => Some(LoginMessage::RestRefused {
                bold: "This account can't use the iTop REST API.".to_owned(),
                rest: format!(
                    " Ask your iTop admin to add the \"REST Services User\" profile to {}, then sign in again.",
                    self.user
                ),
            }),
            Some(e) => Some(LoginMessage::Error { text: e.to_string() }),
            None => notice.map(|n| LoginMessage::Notice { text: n.to_owned() }),
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        match &self.state {
            SessionState::Restoring => SessionSnapshot::Restoring,
            SessionState::SignedOut { notice } => SessionSnapshot::SignedOut {
                form: LoginForm {
                    server: self.server.clone(),
                    user: self.user.clone(),
                    http_warning: trim_spaces(&self.server).to_lowercase().starts_with("http://"),
                    signing_in: self.form.signing,
                    message: self.message(notice.as_deref()),
                    initial_focus: self.form.initial_focus,
                },
            },
            SessionState::Unreachable(message) => SessionSnapshot::Unreachable { message: message.clone() },
            SessionState::SignedIn(session) => SessionSnapshot::SignedIn {
                display_name: session.user.display_name.clone(),
                host: super::shell::account_host(session.client.server.host()),
            },
        }
    }

    /// Sets the state; a sign-in form that newly appears starts clean (a new `LoginView`).
    pub fn set_state(&mut self, state: SessionState) {
        let was_signed_out = matches!(self.state, SessionState::SignedOut { .. });
        if matches!(state, SessionState::SignedOut { .. }) && !was_signed_out {
            self.form = FormState {
                signing: false,
                missing_fields: false,
                error: None,
                initial_focus: initial_focus(&self.server, &self.user),
            };
        }
        self.state = state;
    }

    /// `restore()`'s outcome as a state (AUTH-2…4).
    pub fn restored(result: Result<Option<itop_kit::Session>, SessionError>) -> SessionState {
        match result {
            Ok(Some(session)) => SessionState::SignedIn(std::sync::Arc::new(session)),
            Ok(None) => SessionState::SignedOut { notice: None },
            Err(SessionError::ITop(ITopError::InvalidCredentials)) => {
                SessionState::SignedOut { notice: Some("Your password was rejected. Sign in again.".to_owned()) }
            }
            Err(e) => SessionState::Unreachable(e.to_string()),
        }
    }
}
