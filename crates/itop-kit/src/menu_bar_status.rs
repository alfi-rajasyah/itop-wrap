//! Port of `MenuBarStatus.swift`, the parts that don't need the timer types.
//!
//! The `MenuBarStatus` struct itself (`need_count`, `timer`, `title`, `accessibility_label`)
//! holds a `RunningTimer` and formats with `TimeFormat`, both from TimeTracking.swift, which is
//! ported in its own module (`time_tracking`). It belongs here once that module exists:
//!
//! - `title(now)`: `TimeFormat::counter(elapsed)` when a timer runs, then the count when > 0,
//!   joined by two spaces ("00:12:40  3").
//! - `accessibility_label(now)`: "iTop Wrap: " + "nothing needs you" / "1 ticket needs you" /
//!   "<n> tickets need you", plus ", timer <TimeFormat::clock(elapsed)> on <ref>" when running.
//!
//! Elapsed is `max(0, now - timer.start)` in whole seconds, rounded down.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ticket::Ticket;
use crate::ticket_queries::{TicketGroup, TicketSection};

/// The panel's groups, escalated first (Swift's `MenuBarStatus.groupOrder`).
pub const GROUP_ORDER: [TicketGroup; 4] = [
    TicketGroup::EscalatedTtr,
    TicketGroup::EscalatedTto,
    TicketGroup::Assigned,
    TicketGroup::Pending,
];

/// The panel's sections with the running ticket taken out (Swift's `(running:sections:)` tuple).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PinnedSections {
    pub running: Option<Ticket>,
    pub sections: Vec<TicketSection>,
}

/// The ticket whose timer runs goes to the top of the panel, out of its group, so you see what
/// you're timing first. A group it leaves empty is dropped; a timer on a ticket that isn't
/// listed (filtered out, or not yours) changes nothing. `running_id` is a `Ticket::id()`
/// ("UserRequest::412"). Swift's `MenuBarStatus.pinRunning`.
pub fn pin_running(sections: &[TicketSection], running_id: Option<&str>) -> PinnedSections {
    let running = running_id.and_then(|id| {
        sections
            .iter()
            .flat_map(|s| s.tickets.iter())
            .find(|t| t.id() == id)
            .cloned()
    });
    let (Some(running), Some(id)) = (running, running_id) else {
        return PinnedSections {
            running: None,
            sections: sections.to_vec(),
        };
    };
    let rest = sections
        .iter()
        .filter_map(|s| {
            let tickets: Vec<Ticket> = s.tickets.iter().filter(|t| t.id() != id).cloned().collect();
            (!tickets.is_empty()).then(|| TicketSection::new(s.group, tickets))
        })
        .collect();
    PinnedSections {
        running: Some(running),
        sections: rest,
    }
}
