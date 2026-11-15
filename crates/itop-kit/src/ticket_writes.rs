//! Port of `TicketWrites.swift`: `TicketWrites`, its `Failure`, `Agent`, `TeamQueries` and
//! `TeamDirectory`.
//!
//! The Swift methods `throw` either a `TicketWrites.Failure` (refused locally, nothing sent) or an
//! `ITopError` (from the client). Rust needs one error type per function, so they return
//! [`WriteError`], which carries either one unchanged; callers match on it the way the Swift app
//! catches `TicketWrites.Failure.x` or `ITopError.x`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ITopClient, ITopResponse};
use crate::current_user::{CurrentUser, Team};
use crate::error::ITopError;
use crate::json::{JsonObject, JsonValue};
use crate::rest_operation::RestOperation;
use crate::ticket::Ticket;
use crate::ticket_lifecycle::{TicketAction, TicketLifecycle};

/// Why a change was refused before anything was sent (Swift's `TicketWrites.Failure`).
///
/// Swift gives these no text of their own: the app turns each into its own sentence (see the
/// parity checklist, ASG-*, LOG-8, NEW-14). The `Display` text here is for logs only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
// Two Swift types are called `Failure`; the TypeScript names keep them apart.
#[ts(
    rename = "TicketWriteFailure",
    export,
    export_to = "../../../ui/src/bindings/"
)]
pub enum Failure {
    #[error("empty entry")]
    EmptyEntry,
    /// The ticket's state has no assign or reassign transition.
    #[error("cannot assign in status {status}")]
    #[serde(rename_all = "camelCase")]
    CannotAssign { status: String },
    /// A team or agent id that isn't a plain number.
    #[error("invalid choice")]
    InvalidChoice,
    /// Assign to me found the ticket already taken by someone else (UX-FLOWS F4).
    #[error("already taken by {by}")]
    #[serde(rename_all = "camelCase")]
    AlreadyTaken { by: String },
    /// Assign to me needs one of your teams.
    #[error("no team")]
    NoTeam,
    /// A required field is empty (or not a valid id): "reason", "service", "subcategory",
    /// "resolution code", "solution" (and for New ticket "organization", "caller", "title",
    /// "description").
    #[error("missing {0}")]
    Missing(String),
    /// The ticket's state doesn't offer this action.
    #[error("not allowed in status {status}")]
    #[serde(rename_all = "camelCase")]
    NotAllowed { status: String },
    /// More images than `LogImage::MAX_PER_ENTRY` in one entry.
    #[error("too many images (max {max})")]
    #[serde(rename_all = "camelCase")]
    TooManyImages {
        #[ts(type = "number")]
        max: i64,
    },
}

/// What a ticket write can fail with: refused locally, or by the client / iTop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum WriteError {
    #[error(transparent)]
    Failure(#[from] Failure),
    #[error(transparent)]
    ITop(#[from] ITopError),
}

/// Changes to tickets (PLAN.md Phase 3). Every one goes through `ITopClient`, which refuses it
/// unless writes are on and, with an allowlist, the ticket is on it.
#[derive(Clone, Debug)]
pub struct TicketWrites {
    pub(crate) client: ITopClient,
}

/// Shown in the ticket's history next to the change, so it's traceable (SPEC.md, Boundaries).
pub const COMMENT: &str = "iTop Wrap";

impl TicketWrites {
    /// Same as the module-level [`COMMENT`] (Swift's `TicketWrites.comment`).
    pub const COMMENT: &'static str = COMMENT;

    pub fn new(client: ITopClient) -> Self {
        TicketWrites { client }
    }

    pub fn allowlist_key(ticket: &Ticket) -> String {
        format!("{}::{}", ticket.ticket_class.raw_value(), ticket.key)
    }

    /// Appends an entry to the public or private log. Text format keeps the line breaks.
    pub async fn add_log_entry(
        &self,
        ticket: &Ticket,
        text: &str,
        is_private: bool,
    ) -> Result<(), WriteError> {
        // `.whitespacesAndNewlines` is Unicode White_Space, which `trim` uses.
        let message = text.trim();
        if message.is_empty() {
            return Err(Failure::EmptyEntry.into());
        }
        let field = if is_private {
            "private_log"
        } else {
            "public_log"
        };
        self.update(ticket, log_item(field, message, "text"))
            .await?;
        Ok(())
    }

    /// The stimulus that sets team and agent for a ticket in this state: unowned tickets are
    /// assigned, owned ones reassigned. `None` when the state has neither (resolved, closed).
    pub fn assign_stimulus(status: &str) -> Option<&'static str> {
        let actions = TicketLifecycle::actions(status);
        if actions.contains(&TicketAction::Assign) {
            return Some(TicketAction::Assign.stimulus());
        }
        if actions.contains(&TicketAction::Reassign) {
            return Some(TicketAction::Reassign.stimulus());
        }
        None
    }

    /// Sets the ticket's team and agent with one stimulus.
    pub async fn assign(
        &self,
        ticket: &Ticket,
        team_id: &str,
        agent_id: &str,
    ) -> Result<(), WriteError> {
        let Some(stimulus) = Self::assign_stimulus(&ticket.status) else {
            return Err(Failure::CannotAssign {
                status: ticket.status.clone(),
            }
            .into());
        };
        if !Self::is_id(team_id) || !Self::is_id(agent_id) {
            return Err(Failure::InvalidChoice.into());
        }
        let mut fields = JsonObject::new();
        fields.insert("team_id".into(), team_id.into());
        fields.insert("agent_id".into(), agent_id.into());
        let response = self
            .client
            .send(RestOperation::ApplyStimulus {
                class_name: ticket.ticket_class.raw_value().into(),
                key: ticket.key.clone(),
                stimulus: stimulus.into(),
                fields,
                comment: COMMENT.into(),
            })
            .await?;
        Self::check(&response)?;
        Ok(())
    }

    /// Assigns the ticket to you: its own team when you're in it, otherwise your first team.
    /// Reads the ticket first so a colleague who just took it keeps it.
    pub async fn assign_to_me(&self, ticket: &Ticket, me: &CurrentUser) -> Result<(), WriteError> {
        let Some(team) = me
            .teams
            .iter()
            .find(|t| t.id == ticket.team_id)
            .or_else(|| me.teams.first())
        else {
            return Err(Failure::NoTeam.into());
        };
        if !Self::is_id(&ticket.key) {
            return Err(Failure::InvalidChoice.into());
        }
        let cls = ticket.ticket_class.raw_value();
        let latest = self
            .client
            .send(RestOperation::Get {
                class_name: cls.into(),
                key: format!("SELECT {cls} WHERE id = {}", ticket.key),
                output_fields: vec![
                    "agent_id".into(),
                    "agent_id_friendlyname".into(),
                    "status".into(),
                ],
                limit: Some(1),
            })
            .await?;
        let mut current = ticket.clone();
        if let Some(object) = latest.objects().first() {
            // Only a string agent id counts, as in Swift (`stringValue ?? "0"`).
            let agent = object
                .fields
                .get("agent_id")
                .and_then(JsonValue::string_value)
                .unwrap_or("0")
                .to_string();
            if agent != "0" && !agent.is_empty() && agent != me.contact_id {
                return Err(Failure::AlreadyTaken {
                    by: object
                        .fields
                        .get("agent_id_friendlyname")
                        .and_then(JsonValue::string_value)
                        .unwrap_or("someone else")
                        .to_string(),
                }
                .into());
            }
            if let Some(status) = object
                .fields
                .get("status")
                .and_then(JsonValue::string_value)
                && !status.is_empty()
            {
                current.status = status.to_string();
                current.agent_id = agent;
            }
        }
        self.assign(&current, &team.id, &me.contact_id).await
    }

    /// A plain, non-empty run of ASCII digits: the only ids that are put into OQL or fields.
    pub fn is_id(value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
    }

    /// iTop's verdict on the object itself: the call can succeed while the object was refused.
    pub(crate) fn check(response: &ITopResponse) -> Result<(), ITopError> {
        if let Some(object) = response.objects().first()
            && object.code != 0
        {
            return Err(ITopError::Server {
                code: object.code,
                message: object.message.clone(),
            });
        }
        Ok(())
    }

    /// Runs a `core/update` and checks iTop's verdict on the object itself.
    pub(crate) async fn update(
        &self,
        ticket: &Ticket,
        fields: JsonObject,
    ) -> Result<(), ITopError> {
        let response = self
            .client
            .send(RestOperation::Update {
                class_name: ticket.ticket_class.raw_value().into(),
                key: ticket.key.clone(),
                fields,
                comment: COMMENT.into(),
            })
            .await?;
        Self::check(&response)
    }
}

/// `{field: {add_item: {message, format}}}`, the shape iTop takes for a new log entry.
pub(crate) fn log_item(field: &str, message: &str, format: &str) -> JsonObject {
    let mut item = JsonObject::new();
    item.insert("message".into(), message.into());
    item.insert("format".into(), format.into());
    let mut add = JsonObject::new();
    add.insert("add_item".into(), JsonValue::Object(item));
    let mut fields = JsonObject::new();
    fields.insert(field.into(), JsonValue::Object(add));
    fields
}

/// Someone a ticket can be assigned to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Agent {
    pub id: String,
    pub name: String,
}

impl Agent {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Agent {
            id: id.into(),
            name: name.into(),
        }
    }
}

pub struct TeamQueries;

impl TeamQueries {
    pub const TEAMS: &'static str = "SELECT Team WHERE status = 'active'";

    /// Active people linked to the team. `None` for an id that isn't a number.
    pub fn agents(team_id: &str) -> Option<String> {
        if !TicketWrites::is_id(team_id) {
            return None;
        }
        Some(format!(
            "SELECT Person AS p JOIN lnkPersonToTeam AS l ON l.person_id = p.id WHERE l.team_id = {team_id} AND p.status = 'active'"
        ))
    }
}

/// Reads teams and their agents for the Assign sheet's pickers.
#[derive(Clone, Debug)]
pub struct TeamDirectory {
    client: ITopClient,
}

impl TeamDirectory {
    pub fn new(client: ITopClient) -> Self {
        TeamDirectory { client }
    }

    pub async fn teams(&self) -> Result<Vec<Team>, ITopError> {
        let response = self
            .client
            .send(RestOperation::get(
                "Team",
                TeamQueries::TEAMS,
                &["friendlyname"],
            ))
            .await?;
        let mut teams: Vec<Team> = response
            .objects()
            .iter()
            .map(|o| Team {
                id: o.key.clone(),
                name: friendly_name(o, "Team"),
            })
            .collect();
        teams.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(teams)
    }

    pub async fn agents(&self, team_id: &str) -> Result<Vec<Agent>, ITopError> {
        let Some(oql) = TeamQueries::agents(team_id) else {
            return Ok(Vec::new());
        };
        let response = self
            .client
            .send(RestOperation::get("Person", oql, &["friendlyname"]))
            .await?;
        let mut agents: Vec<Agent> = response
            .objects()
            .iter()
            .map(|o| Agent {
                id: o.key.clone(),
                name: friendly_name(o, "Person"),
            })
            .collect();
        agents.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(agents)
    }
}

/// The object's `friendlyname`, or "<Class> <key>" without one.
pub(crate) fn friendly_name(object: &crate::client::ITopObject, class_name: &str) -> String {
    object
        .fields
        .get("friendlyname")
        .and_then(JsonValue::string_value)
        .map_or_else(|| format!("{class_name} {}", object.key), str::to_string)
}

// MARK: - Natural sort

// One natural sort for the whole crate: the pickers here use the same order as the lists.
pub(crate) use crate::ticket_queries::localized_standard_compare;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn natural_order() {
        let mut v = vec![
            "team 10", "Team 2", "Zoe", "ann", "Émile", "Eve", "e", "", "Team 02x",
        ];
        v.sort_by(|a, b| localized_standard_compare(a, b));
        assert_eq!(
            v,
            vec![
                "", "ann", "e", "Émile", "Eve", "Team 2", "Team 02x", "team 10", "Zoe"
            ]
        );
        // Checked against Foundation on macOS 27: lowercase sorts first.
        assert_eq!(localized_standard_compare("ABC", "abc"), Ordering::Greater);
        assert_eq!(localized_standard_compare("e", "é"), Ordering::Less);
        assert_eq!(localized_standard_compare("éa", "eb"), Ordering::Less);
    }
}
