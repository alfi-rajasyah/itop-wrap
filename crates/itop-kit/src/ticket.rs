//! Port of `Ticket.swift`.

use chrono::{DateTime, Local, LocalResult, NaiveDateTime, Offset, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::ITopObject;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketClass {
    #[serde(rename = "UserRequest")]
    UserRequest,
    #[serde(rename = "Incident")]
    Incident,
}

impl TicketClass {
    /// `CaseIterable.allCases`, in declaration order.
    pub const ALL: [TicketClass; 2] = [TicketClass::UserRequest, TicketClass::Incident];

    /// The iTop class name (Swift's `rawValue`).
    pub fn raw_value(self) -> &'static str {
        match self {
            TicketClass::UserRequest => "UserRequest",
            TicketClass::Incident => "Incident",
        }
    }

    /// `TicketClass(rawValue:)`.
    pub fn from_raw(raw: &str) -> Option<TicketClass> {
        TicketClass::ALL.into_iter().find(|c| c.raw_value() == raw)
    }

    /// The prefix iTop uses for this class's refs: R-000412, I-000088.
    pub fn ref_prefix(self) -> &'static str {
        if self == TicketClass::UserRequest {
            "R"
        } else {
            "I"
        }
    }
}

/// One open ticket as the list shows it. Values come straight from iTop; `status` is the
/// iTop code ("assigned", "escalated_ttr"), and ids are strings, "0" meaning empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct Ticket {
    pub key: String,
    pub ticket_class: TicketClass,
    pub r#ref: String,
    pub title: String,
    pub status: String,
    #[ts(type = "number")]
    pub priority: i64,
    pub caller_name: String,
    pub org_name: String,
    /// Swift's init defaults this to "0".
    pub org_id: String,
    pub team_id: String,
    pub team_name: String,
    pub agent_id: String,
    pub agent_name: String,
    pub start_date: Option<DateTime<Utc>>,
    pub last_update: Option<DateTime<Utc>>,
}

impl Ticket {
    /// The fields the list needs, so iTop doesn't send logs and descriptions for every row.
    pub const LIST_FIELDS: &'static [&'static str] = &[
        "ref",
        "title",
        "status",
        "priority",
        "caller_id_friendlyname",
        "org_id",
        "org_id_friendlyname",
        "team_id",
        "team_id_friendlyname",
        "agent_id",
        "agent_id_friendlyname",
        "start_date",
        "last_update",
    ];

    /// `Identifiable.id`: "UserRequest::7".
    pub fn id(&self) -> String {
        format!("{}::{}", self.ticket_class.raw_value(), self.key)
    }

    pub fn is_unassigned(&self) -> bool {
        self.agent_id == "0" || self.agent_id.is_empty()
    }

    /// `Ticket(object:)`: `None` when the object isn't a UserRequest or Incident.
    pub fn from_object(object: &ITopObject) -> Option<Ticket> {
        let cls = TicketClass::from_raw(&object.class_name)?;
        let f = &object.fields;
        let text = |k: &str| {
            f.get(k)
                .and_then(|v| v.string_value())
                .unwrap_or("")
                .to_string()
        };
        let ident = |k: &str| {
            f.get(k)
                .and_then(|v| v.int_value())
                .map_or_else(|| "0".to_string(), |n| n.to_string())
        };
        Some(Ticket {
            key: object.key.clone(),
            ticket_class: cls,
            r#ref: text("ref"),
            title: text("title"),
            status: text("status"),
            priority: f.get("priority").and_then(|v| v.int_value()).unwrap_or(4),
            caller_name: text("caller_id_friendlyname"),
            org_name: text("org_id_friendlyname"),
            org_id: ident("org_id"),
            team_id: ident("team_id"),
            team_name: text("team_id_friendlyname"),
            agent_id: ident("agent_id"),
            agent_name: text("agent_id_friendlyname"),
            start_date: Self::parse_date(&text("start_date")),
            last_update: Self::parse_date(&text("last_update")),
        })
    }

    /// iTop sends dates as "2026-11-09 11:00:32" without a time zone, in the server's zone.
    /// They're read in the Mac's zone, which is right when both are set the same way.
    ///
    /// Like `DateFormatter` (ICU), a time repeated by a DST change reads as its first
    /// occurrence, and a time skipped by one reads with the offset from before the change.
    pub fn parse_date(text: &str) -> Option<DateTime<Utc>> {
        if text.is_empty() {
            return None;
        }
        let naive = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").ok()?;
        local_to_utc(&Local, naive)
    }
}

/// Reads a wall-clock time in `zone`, the way ICU does (see `Ticket::parse_date`).
pub(crate) fn local_to_utc<Z: TimeZone>(zone: &Z, naive: NaiveDateTime) -> Option<DateTime<Utc>> {
    match zone.from_local_datetime(&naive) {
        LocalResult::Single(t) => Some(t.with_timezone(&Utc)),
        LocalResult::Ambiguous(first, _) => Some(first.with_timezone(&Utc)),
        LocalResult::None => {
            let before = naive.checked_sub_signed(chrono::Duration::days(1))?;
            let offset = zone.offset_from_utc_datetime(&before).fix();
            let utc = naive.checked_sub_signed(chrono::Duration::seconds(i64::from(
                offset.local_minus_utc(),
            )))?;
            Some(Utc.from_utc_datetime(&utc))
        }
    }
}
