//! Port of `MenuBarStatus.swift`: what the menu bar item says, and the panel's sections.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use chrono::{DateTime, Utc};

use crate::ticket::Ticket;
use crate::ticket_queries::{TicketGroup, TicketSection};
use crate::time_tracking::{RunningTimer, TimeFormat};

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

/// What the menu bar item says (DESIGN.md: Menu bar item): the running timer, then the Needs
/// you count, each only when there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBarStatus {
    pub need_count: i64,
    pub timer: Option<RunningTimer>,
}

impl MenuBarStatus {
    pub fn new(need_count: i64, timer: Option<RunningTimer>) -> Self {
        Self { need_count, timer }
    }

    /// Whole seconds since the timer started, never negative.
    fn seconds(timer: &RunningTimer, now: DateTime<Utc>) -> i64 {
        (now - timer.start).num_seconds().max(0)
    }

    /// The title ticks by the second (`00:12:40`), like the window's counter, so a running timer
    /// visibly runs; VoiceOver keeps hours and minutes.
    pub fn title(&self, now: DateTime<Utc>) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(timer) = &self.timer {
            parts.push(TimeFormat::counter(Self::seconds(timer, now)));
        }
        if self.need_count > 0 {
            parts.push(self.need_count.to_string());
        }
        parts.join("  ")
    }

    pub fn accessibility_label(&self, now: DateTime<Utc>) -> String {
        let mut text = String::from("iTop Wrap: ");
        match self.need_count {
            0 => text += "nothing needs you",
            1 => text += "1 ticket needs you",
            n => text += &format!("{n} tickets need you"),
        }
        if let Some(timer) = &self.timer {
            text += &format!(
                ", timer {} on {}",
                TimeFormat::clock(Self::seconds(timer, now)),
                timer.r#ref
            );
        }
        text
    }
}
