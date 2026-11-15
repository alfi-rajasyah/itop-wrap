//! Port of `SessionService.swift`.
//!
//! `UserDefaults` isn't available here, so settings go through the [`SettingsStore`] trait. The
//! app crate implements it over the same defaults domain and keys ("server", "user"), so what the
//! Swift app remembered carries over.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::client::ITopClient;
use crate::current_user::CurrentUser;
use crate::error::ITopError;
use crate::oql;
use crate::rest_operation::RestOperation;
use crate::secret_store::{KeychainError, SecretStore};
use crate::server_address::{Credentials, ServerAddress};
use crate::ticket::{Ticket, TicketClass};
use crate::transport::{ReqwestTransport, Transport, TransportError};

/// String settings, like the parts of `UserDefaults` the session uses.
pub trait SettingsStore: Send + Sync {
    fn string(&self, key: &str) -> Option<String>;
    /// `None` removes the value (`defaults.set(nil, forKey:)`).
    fn set_string(&self, key: &str, value: Option<&str>);
}

/// Settings kept in memory, for tests and previews.
#[derive(Debug, Default)]
pub struct MemorySettingsStore {
    values: Mutex<HashMap<String, String>>,
}

impl MemorySettingsStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.values.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SettingsStore for MemorySettingsStore {
    fn string(&self, key: &str) -> Option<String> {
        self.lock().get(key).cloned()
    }

    fn set_string(&self, key: &str, value: Option<&str>) {
        match value {
            Some(v) => self.lock().insert(key.to_string(), v.to_string()),
            None => self.lock().remove(key),
        };
    }
}

/// The server address and username, remembered between launches. Never the password.
#[derive(Clone)]
pub struct SessionSettings {
    store: Arc<dyn SettingsStore>,
}

impl SessionSettings {
    /// The `UserDefaults` key for the server's base URL.
    pub const SERVER_KEY: &'static str = "server";
    /// The `UserDefaults` key for the username.
    pub const USER_KEY: &'static str = "user";

    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        SessionSettings { store }
    }

    /// Settings in a fresh in-memory store.
    pub fn in_memory() -> Self {
        Self::new(Arc::new(MemorySettingsStore::new()))
    }

    pub fn server(&self) -> Option<String> {
        self.store.string(Self::SERVER_KEY)
    }

    pub fn set_server(&self, value: Option<&str>) {
        self.store.set_string(Self::SERVER_KEY, value);
    }

    pub fn user(&self) -> Option<String> {
        self.store.string(Self::USER_KEY)
    }

    pub fn set_user(&self, value: Option<&str>) {
        self.store.set_string(Self::USER_KEY, value);
    }
}

impl fmt::Debug for SessionSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSettings")
            .field("server", &self.server())
            .field("user", &self.user())
            .finish()
    }
}

/// A signed-in client and who it belongs to.
#[derive(Clone, Debug)]
pub struct Session {
    pub client: ITopClient,
    pub user: CurrentUser,
}

/// Whether the session may change anything in iTop (SPEC.md, Boundaries: read-only until you say so).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum WriteAccess {
    #[default]
    Off,
    /// Writes on for every ticket your account may edit; iTop's own rights still apply.
    /// Release builds, switched on by you on 2026-11-11.
    All,
    /// Writes only on these tickets, given by ref ("R-079999"), for testing on a real iTop;
    /// `new_tickets` also allows creating tickets.
    Tickets {
        refs: Vec<String>,
        new_tickets: bool,
    },
}

/// Why signing in, restoring or signing out failed: iTop, or the secret store.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    ITop(#[from] ITopError),
    #[error(transparent)]
    Keychain(#[from] KeychainError),
}

/// Makes the client for a server and credentials (tests route it to a stub).
pub type MakeClient = Arc<dyn Fn(ServerAddress, Credentials) -> ITopClient + Send + Sync>;

/// Sign in, restore after a relaunch, and sign out (PLAN.md, Task 2).
#[derive(Clone)]
pub struct SessionService {
    secrets: Arc<dyn SecretStore>,
    settings: SessionSettings,
    write_access: WriteAccess,
    make_client: MakeClient,
}

impl SessionService {
    /// Writes off, and clients that talk to the real server through [`ReqwestTransport`].
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        settings: SessionSettings,
    ) -> Result<Self, TransportError> {
        let transport: Arc<dyn Transport> = Arc::new(ReqwestTransport::new()?);
        Ok(Self::with_make_client(
            secrets,
            settings,
            WriteAccess::Off,
            move |server, credentials| ITopClient::new(server, credentials, transport.clone()),
        ))
    }

    pub fn with_make_client(
        secrets: Arc<dyn SecretStore>,
        settings: SessionSettings,
        write_access: WriteAccess,
        make_client: impl Fn(ServerAddress, Credentials) -> ITopClient + Send + Sync + 'static,
    ) -> Self {
        SessionService {
            secrets,
            settings,
            write_access,
            make_client: Arc::new(make_client),
        }
    }

    /// The same service with another write access.
    pub fn with_write_access(mut self, write_access: WriteAccess) -> Self {
        self.write_access = write_access;
        self
    }

    /// The Keychain account for a user on a server.
    pub fn account(user: &str, server: &ServerAddress) -> String {
        format!("{user}@{}", server.base_url())
    }

    pub fn saved_server(&self) -> Option<String> {
        self.settings.server()
    }

    pub fn saved_user(&self) -> Option<String> {
        self.settings.user()
    }

    /// Checks the credentials, loads who you are, then saves the password to the Keychain.
    /// Nothing is saved if any step fails.
    pub async fn sign_in(
        &self,
        server: &str,
        user: &str,
        password: &str,
    ) -> Result<Session, SessionError> {
        let address = ServerAddress::new(server)?;
        let login = user.trim().to_string();
        let session = self
            .open(address.clone(), Credentials::new(login.clone(), password))
            .await?;
        self.secrets
            .set_password(password, &Self::account(&login, &address))?;
        self.settings.set_server(Some(address.base_url()));
        self.settings.set_user(Some(&login));
        Ok(session)
    }

    /// The saved session, or `None` when nothing is saved. Fails when the saved password
    /// no longer works; the server and username stay saved for the sign-in form.
    pub async fn restore(&self) -> Result<Option<Session>, SessionError> {
        let (Some(server), Some(user)) = (self.settings.server(), self.settings.user()) else {
            return Ok(None);
        };
        let Ok(address) = ServerAddress::new(&server) else {
            return Ok(None);
        };
        let Some(password) = self.secrets.password(&Self::account(&user, &address))? else {
            return Ok(None);
        };
        Ok(Some(
            self.open(address, Credentials::new(user, password)).await?,
        ))
    }

    /// Removes the password from the Keychain. The server and username stay filled in.
    pub fn sign_out(&self, session: &Session) -> Result<(), SessionError> {
        self.secrets.delete_password(&Self::account(
            &session.client.credentials.user,
            &session.client.server,
        ))?;
        Ok(())
    }

    async fn open(
        &self,
        address: ServerAddress,
        credentials: Credentials,
    ) -> Result<Session, ITopError> {
        let client = (self.make_client)(address, credentials);
        if !client.check_credentials().await? {
            return Err(ITopError::InvalidCredentials);
        }
        let me = CurrentUser::load(&client.credentials.user, &client).await?;
        match &self.write_access {
            WriteAccess::Off => Ok(Session { client, user: me }),
            WriteAccess::All => Ok(Session {
                client: client.allowing_writes(None, false),
                user: me,
            }),
            WriteAccess::Tickets { refs, new_tickets } => {
                // Resolve each ref to its exact object; refs that don't exist stay out of the allowlist.
                let mut allowed: HashSet<String> = HashSet::new();
                for r in refs {
                    if let Ok(Some(t)) = lookup(&client, r).await {
                        allowed.insert(t.id());
                    }
                }
                Ok(Session {
                    client: client.allowing_writes(Some(allowed), *new_tickets),
                    user: me,
                })
            }
        }
    }
}

impl fmt::Debug for SessionService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionService")
            .field("settings", &self.settings)
            .field("write_access", &self.write_access)
            .finish_non_exhaustive()
    }
}

/// `TicketService(client:).lookup(ref)` from TicketQueries.swift, which is ported in its own
/// module; kept here, private and identical, so the session doesn't depend on that module.
/// The allowlist key is `TicketWrites.allowlistKey(for:)`, which is `Ticket::id()`.
async fn lookup(client: &ITopClient, r: &str) -> Result<Option<Ticket>, ITopError> {
    let Some((cls, oql)) = ref_lookup(r) else {
        return Ok(None);
    };
    let response = client
        .send(RestOperation::get(
            cls.raw_value(),
            oql,
            Ticket::LIST_FIELDS,
        ))
        .await?;
    Ok(response.objects().iter().find_map(Ticket::from_object))
}

/// `TicketQueries.refLookup`: a full ref ("R-000412") and the OQL that finds it.
fn ref_lookup(text: &str) -> Option<(TicketClass, String)> {
    let r = text
        .trim_matches(crate::error::is_swift_whitespace)
        .to_uppercase();
    let b = r.as_bytes();
    let well_formed = b.len() == 8
        && (b[0] == b'R' || b[0] == b'I')
        && b[1] == b'-'
        && b[2..].iter().all(u8::is_ascii_digit);
    if !well_formed {
        return None;
    }
    let cls = TicketClass::ALL
        .into_iter()
        .find(|c| r.starts_with(&format!("{}-", c.ref_prefix())))?;
    Some((
        cls,
        format!("SELECT {} WHERE ref = {}", cls.raw_value(), oql::quote(&r)),
    ))
}
