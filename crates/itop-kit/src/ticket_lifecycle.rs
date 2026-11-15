//! Port of `TicketLifecycle.swift`: `TicketAction`, `TicketLifecycle`, `Resolution`,
//! `TicketChange`, `TicketWrites::apply`, `ServiceRef`, `ServiceQueries`, `ServiceCatalog` and
//! `ResolutionCode`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::ITopClient;
use crate::error::ITopError;
use crate::json::JsonObject;
use crate::new_ticket::NewTicket;
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_writes::{
    COMMENT, Failure, TicketWrites, WriteError, friendly_name, localized_standard_compare,
};

/// What can be done to a ticket, each with its iTop stimulus (PLAN.md Task 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketAction {
    Assign,
    Reassign,
    Pending,
    Resolve,
    Reopen,
    Close,
}

impl TicketAction {
    /// `CaseIterable.allCases`, in declaration order.
    pub const ALL: [TicketAction; 6] = [
        TicketAction::Assign,
        TicketAction::Reassign,
        TicketAction::Pending,
        TicketAction::Resolve,
        TicketAction::Reopen,
        TicketAction::Close,
    ];

    /// Swift's `rawValue`.
    pub fn raw_value(self) -> &'static str {
        match self {
            TicketAction::Assign => "assign",
            TicketAction::Reassign => "reassign",
            TicketAction::Pending => "pending",
            TicketAction::Resolve => "resolve",
            TicketAction::Reopen => "reopen",
            TicketAction::Close => "close",
        }
    }

    pub fn stimulus(self) -> &'static str {
        match self {
            TicketAction::Assign => "ev_assign",
            TicketAction::Reassign => "ev_reassign",
            TicketAction::Pending => "ev_pending",
            TicketAction::Resolve => "ev_resolve",
            TicketAction::Reopen => "ev_reopen",
            TicketAction::Close => "ev_close",
        }
    }
}

/// The standard iTop request/incident lifecycle, as offered in the app. Your iTop has some
/// customisation, so iTop still has the last word: a refused stimulus comes back as its message.
pub struct TicketLifecycle;

impl TicketLifecycle {
    pub fn actions(status: &str) -> Vec<TicketAction> {
        use TicketAction::*;
        match status {
            "new" | "escalated_tto" => vec![Assign],
            "assigned" | "escalated_ttr" => vec![Reassign, Pending, Resolve],
            "pending" => vec![Reassign, Resolve],
            "resolved" => vec![Reopen, Close],
            _ => Vec::new(),
        }
    }

    /// The one action shown as a button in the header (DESIGN.md: Ticket detail). A resolved
    /// ticket's is Close, the usual next step; Reopen is in the More menu.
    pub fn primary(status: &str) -> Option<TicketAction> {
        let actions = Self::actions(status);
        [
            TicketAction::Assign,
            TicketAction::Resolve,
            TicketAction::Close,
        ]
        .into_iter()
        .find(|candidate| actions.contains(candidate))
    }
}

/// What Resolve sends, matching iTop's "Mark as resolved" form. No time field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Resolution {
    pub service_id: String,
    pub subcategory_id: String,
    pub code: String,
    pub solution: String,
}

impl Resolution {
    pub fn new(
        service_id: impl Into<String>,
        subcategory_id: impl Into<String>,
        code: impl Into<String>,
        solution: impl Into<String>,
    ) -> Self {
        Resolution {
            service_id: service_id.into(),
            subcategory_id: subcategory_id.into(),
            code: code.into(),
            solution: solution.into(),
        }
    }
}

/// A lifecycle change other than assigning (Task 7 has its own call).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketChange {
    #[serde(rename_all = "camelCase")]
    Pending {
        reason: String,
    },
    Resolve(Resolution),
    Reopen,
    Close,
}

impl TicketChange {
    pub fn action(&self) -> TicketAction {
        match self {
            TicketChange::Pending { .. } => TicketAction::Pending,
            TicketChange::Resolve(_) => TicketAction::Resolve,
            TicketChange::Reopen => TicketAction::Reopen,
            TicketChange::Close => TicketAction::Close,
        }
    }
}

impl TicketWrites {
    /// Runs one lifecycle change. Required fields and the state are checked first, so nothing
    /// is sent for a change iTop would have to refuse.
    pub async fn apply(&self, change: &TicketChange, ticket: &Ticket) -> Result<(), WriteError> {
        if !TicketLifecycle::actions(&ticket.status).contains(&change.action()) {
            return Err(Failure::NotAllowed {
                status: ticket.status.clone(),
            }
            .into());
        }
        let mut fields = JsonObject::new();
        match change {
            TicketChange::Pending { reason } => {
                let text = reason.trim();
                if text.is_empty() {
                    return Err(Failure::Missing("reason".into()).into());
                }
                fields.insert("pending_reason".into(), text.into());
            }
            TicketChange::Resolve(r) => {
                if !Self::is_id(&r.service_id) {
                    return Err(Failure::Missing("service".into()).into());
                }
                if !Self::is_id(&r.subcategory_id) {
                    return Err(Failure::Missing("subcategory".into()).into());
                }
                if r.code.is_empty() {
                    return Err(Failure::Missing("resolution code".into()).into());
                }
                let solution = r.solution.trim();
                if solution.is_empty() {
                    return Err(Failure::Missing("solution".into()).into());
                }
                fields.insert("service_id".into(), r.service_id.as_str().into());
                fields.insert(
                    "servicesubcategory_id".into(),
                    r.subcategory_id.as_str().into(),
                );
                fields.insert("resolution_code".into(), r.code.as_str().into());
                // HTML (SPEC answer 7).
                fields.insert("solution".into(), NewTicket::html(solution).into());
            }
            TicketChange::Reopen | TicketChange::Close => {}
        }
        let response = self
            .client
            .send(RestOperation::ApplyStimulus {
                class_name: ticket.ticket_class.raw_value().into(),
                key: ticket.key.clone(),
                stimulus: change.action().stimulus().into(),
                fields,
                comment: COMMENT.into(),
            })
            .await?;
        Self::check(&response)?;
        Ok(())
    }
}

// MARK: - Services and resolution codes

/// A service or service subcategory, for the Resolve sheet's pickers (also used for New
/// ticket's lists).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ServiceRef {
    pub id: String,
    pub name: String,
}

impl ServiceRef {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        ServiceRef {
            id: id.into(),
            name: name.into(),
        }
    }
}

pub struct ServiceQueries;

impl ServiceQueries {
    /// Services the ticket's organization has a customer contract for.
    pub fn services(org_id: &str) -> Option<String> {
        if !TicketWrites::is_id(org_id) {
            return None;
        }
        Some(format!(
            "SELECT Service AS s JOIN lnkCustomerContractToService AS l ON l.service_id = s.id JOIN CustomerContract AS cc ON l.customercontract_id = cc.id WHERE cc.org_id = {org_id} AND s.status != 'obsolete'"
        ))
    }

    pub const ALL_SERVICES: &'static str = "SELECT Service WHERE status != 'obsolete'";

    pub fn subcategories(service_id: &str) -> Option<String> {
        if !TicketWrites::is_id(service_id) {
            return None;
        }
        Some(format!(
            "SELECT ServiceSubcategory WHERE service_id = {service_id} AND status != 'obsolete'"
        ))
    }
}

#[derive(Clone, Debug)]
pub struct ServiceCatalog {
    client: ITopClient,
}

impl ServiceCatalog {
    pub fn new(client: ITopClient) -> Self {
        ServiceCatalog { client }
    }

    /// The org's contracted services, or every live service when it has none (or no contracts
    /// are kept).
    pub async fn services(&self, ticket: &Ticket) -> Result<Vec<ServiceRef>, ITopError> {
        if let Some(oql) = ServiceQueries::services(&ticket.org_id) {
            let found = self.refs("Service", &oql).await?;
            if !found.is_empty() {
                return Ok(found);
            }
        }
        self.refs("Service", ServiceQueries::ALL_SERVICES).await
    }

    pub async fn subcategories(&self, service_id: &str) -> Result<Vec<ServiceRef>, ITopError> {
        let Some(oql) = ServiceQueries::subcategories(service_id) else {
            return Ok(Vec::new());
        };
        self.refs("ServiceSubcategory", &oql).await
    }

    async fn refs(&self, class_name: &str, oql: &str) -> Result<Vec<ServiceRef>, ITopError> {
        let response = self
            .client
            .send(RestOperation::get(class_name, oql, &["friendlyname"]))
            .await?;
        // Each service once, even if the contract join named it more than once (as in Swift).
        let mut seen = HashSet::new();
        let mut refs: Vec<ServiceRef> = response
            .objects()
            .iter()
            .filter(|o| seen.insert(o.key.clone()))
            .map(|o| ServiceRef {
                id: o.key.clone(),
                name: friendly_name(o, class_name),
            })
            .collect();
        refs.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(refs)
    }
}

/// iTop's standard resolution codes. Read through REST there's no list of allowed values, so a
/// customised list shows up as iTop refusing the code, with its message.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct ResolutionCode {
    pub id: String,
}

impl ResolutionCode {
    pub fn new(id: impl Into<String>) -> Self {
        ResolutionCode { id: id.into() }
    }

    /// The id with its first letter uppercased: "bug fixed" → "Bug fixed".
    pub fn name(&self) -> String {
        let mut chars = self.id.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => String::new(),
        }
    }

    pub fn standard() -> Vec<ResolutionCode> {
        [
            "assistance",
            "bug fixed",
            "hardware repair",
            "other",
            "software patch",
            "system update",
            "training",
        ]
        .into_iter()
        .map(ResolutionCode::new)
        .collect()
    }
}
