//! What the standalone models (new ticket, CMDB, orgs, people) need from the rest of the workspace.
use std::sync::Arc;
use itop_kit::{Session, Ticket, TicketScope};
use crate::{hub::Hub, native::Native};

/// Shared by every model of one signed-in session.
#[derive(Clone)]
pub struct ModelContext {
    pub session: Arc<Session>,
    pub hub: Hub,
    pub native: Arc<dyn Native>,
}

/// Calls from the standalone models into the workspace (Swift: closures/`Workspace` methods the views call).
#[async_trait::async_trait]
pub trait WorkspaceLink: Send + Sync {
    /// The View options' current scope (ListOptions.scope).
    fn scope(&self) -> TicketScope;
    /// Open a ticket from a CI, org or person (ShellView's open-ticket routing, SHELL-9).
    async fn open_ticket(&self, ticket: Ticket);
    /// Workspace.created(_:assign:): show it in My tickets, and assign if asked.
    async fn ticket_created(&self, ticket: Ticket, assign: bool);
    /// Switch the sidebar section ("myTickets" | "teamTickets" | "configItems" | "organizations" | "people").
    async fn select_section(&self, section: &str);
}

/// One standalone model, as the router sees it.
#[async_trait::async_trait]
pub trait Model: Send + Sync {
    /// Handles an intent addressed to `model_id`; returns the intent's result (contract.ts ResultOf).
    async fn dispatch(&self, model_id: &str, intent: serde_json::Value) -> Result<serde_json::Value, crate::error::AppError>;
}

/// Where New ticket starts from (Swift `NewTicketStart`): plain (all `None`), an org
/// (`org_id` + `org_name`, SHELL-12) or a CI (`org_name` + `ci_id` + `ci_name`, SHELL-13).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewTicketStart {
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub ci_id: Option<String>,
    pub ci_name: Option<String>,
}

/// The shared View options (Swift `ListOptions`) as the org and person ticket tables read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewOptions {
    pub scope: TicketScope,
    pub sort: itop_kit::TicketSort,
    /// `None` = All on one page.
    pub page_size: Option<i64>,
}

/// Calls from the workspace into the standalone models: the ShellView closures that reach into
/// `cmdb`, `orgs`, `people` and New ticket, and the view lifecycle SwiftUI ran for them
/// (`.task` on appear, `onChange` of the View options in the shown view).
#[async_trait::async_trait]
pub trait StandaloneLinks: Send + Sync {
    /// SHELL-10 (`ShellView.openOrg`): clear the org search and select this org. The workspace
    /// then switches to Organizations.
    async fn open_org(&self, org_id: &str, org_name: &str);
    /// SHELL-11 (`ShellView.openCI`): `cmdb.open(ConfigItem(key, className, name, orgName: ""))`.
    /// The workspace then switches to Config items.
    async fn open_ci(&self, key: &str, class_name: &str, name: &str);
    /// SHELL-12: `people.showOrg(id:name:)`. The workspace then switches to People.
    async fn show_org_people(&self, org_id: &str, org_name: &str);
    /// SHELL-12: `cmdb.showOrg(id:name:)`. The workspace then switches to Config items.
    async fn show_org_config_items(&self, org_id: &str, org_name: &str);
    /// Opens New ticket: `window` false is the main window's sheet (`newTicket:sheet`, NEW-1,
    /// SHELL-14), true the menu bar's separate window (`newTicket:window`, NEW-2). A plain start
    /// restores the kept draft (NEW-22); an org or CI start begins fresh.
    async fn open_new_ticket(&self, window: bool, start: NewTicketStart);
    /// A sidebar section came on screen ("configItems" | "organizations" | "people"): its views'
    /// on-appear work (the org list loads once, ORG-2, POLL-3).
    async fn section_shown(&self, section: &str);
    /// The View options changed (or are being told for the first time, `shown` = None). `shown` is
    /// the section on screen, the only one whose view reacts (contract §5.3): an org's or a
    /// person's tickets reload when `scope_changed`, and their table goes back to page 1 when the
    /// scope, sort or page size changed.
    async fn view_changed(&self, view: ViewOptions, shown: Option<&str>, scope_changed: bool);
}
