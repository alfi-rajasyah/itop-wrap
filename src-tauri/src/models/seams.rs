//! What the standalone models (new ticket, CMDB, orgs, people) need from the rest of the workspace.
use crate::{hub::Hub, native::Native};
use itop_kit::{Session, Ticket, TicketScope};
use std::sync::Arc;

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
    async fn dispatch(
        &self,
        model_id: &str,
        intent: serde_json::Value,
    ) -> Result<serde_json::Value, crate::error::AppError>;
}
