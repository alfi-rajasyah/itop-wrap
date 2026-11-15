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

// Reads: ticket lists, detail, logs, rich text, CMDB, organizations, menu bar.
pub mod case_log;
pub mod config_item;
pub mod menu_bar_status;
pub mod organization;
pub mod rich_text;
pub mod ticket_detail;
pub mod ticket_queries;
pub use case_log::{CaseLog, DateLocale, HtmlText, LogDate, LogEntry};
pub use config_item::{
    CIField, CIRef, CMDBQueries, CMDBService, ConfigItem, ConfigItemDetail, Direction,
};
pub use menu_bar_status::PinnedSections;
pub use organization::{OrgCounts, OrgQueries, OrgService, Organization, PeoplePage, Person};
pub use rich_text::{
    AttachmentFile, AttachmentRef, InlineImage, InlineImageError, InlineImageFailure,
    InlineImageLoader, RichBlock, RichImage, RichRun, RichText,
};
pub use ticket_detail::{LinkedCI, TicketDetail};
pub use ticket_queries::{
    Age, TicketGroup, TicketList, TicketListView, TicketPage, TicketQueries, TicketScope,
    TicketSearch, TicketSection, TicketService, TicketSort, localized_standard_compare,
};
