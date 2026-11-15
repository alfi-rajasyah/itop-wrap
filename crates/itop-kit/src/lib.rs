//! iTop REST client and ticket rules. Port of the Swift `ITopKit` target.
//!
//! One module per Swift file (`ITopClient.swift` → `client`, and so on), plus `transport` (the
//! injectable HTTP layer that replaces `URLSession`) and `stub` (the port of the test helper
//! `StubURLProtocol.swift`).

#![forbid(unsafe_code)]

pub mod client;
pub mod current_user;
pub mod error;
pub mod json;
pub mod oql;
pub mod rest_operation;
pub mod secret_store;
pub mod server_address;
pub mod session;
pub mod stub;
pub mod ticket;
pub mod transport;

pub use client::{ITopClient, ITopObject, ITopResponse};
pub use current_user::{CurrentUser, Team};
pub use error::ITopError;
pub use json::{JsonObject, JsonValue};
pub use rest_operation::RestOperation;
pub use secret_store::{KeychainError, MemorySecretStore, SecretStore};
pub use server_address::{Credentials, ServerAddress};
pub use session::{
    MakeClient, MemorySettingsStore, Session, SessionError, SessionService, SessionSettings,
    SettingsStore, WriteAccess,
};
pub use ticket::{Ticket, TicketClass};
pub use transport::{HttpRequest, HttpResponse, ReqwestTransport, Transport, TransportError};
