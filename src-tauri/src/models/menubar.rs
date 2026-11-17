//! Port of `MenuBarPanel` and `MenuBarLabel` (W/MenuBarView.swift, MB-*): the tray's title and
//! the panel's list of My tickets.

use chrono::{DateTime, Utc};
use itop_kit::{TicketList, TicketSearch, TimeFormat, menu_bar_status};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::list::Phase;
use super::new_ticket::{NewTicketInstance, NewTicketStart};
use super::records::{SidebarItem, TicketGroup};
use super::workspace::{State, Workspace};
use crate::error::AppError;

#[derive(Clone, Debug, Default)]
pub struct MenuState {
    /// The panel's own search (MB-7).
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum MenuRowRight {
    None,
    /// The live "HH:MM:SS" at the time of this snapshot (re-sent every second while it runs),
    /// so the row shows its counter even before the `timer` snapshot arrives (MB-13).
    Running {
        counter: String,
    },
    Spent {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum MenuRowButton {
    Stop,
    Start { enabled: bool, help: String },
    Slot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct MenuRow {
    pub ticket_id: String,
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub r#ref: String,
    pub title: String,
    pub needs_you: bool,
    pub is_running: bool,
    /// Resolve is offered for this status (iTop Wrap addition: quick access to the sheet).
    pub can_resolve: bool,
    pub right: MenuRowRight,
    pub button: MenuRowButton,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct MenuSection {
    pub group: TicketGroup,
    pub title: String,
    pub rows: Vec<MenuRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum MenuContent {
    Loading,
    Failed {
        text: String,
    },
    Empty {
        text: String,
    },
    List {
        running: Option<MenuRow>,
        sections: Vec<MenuSection>,
        height: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct MenuBarSnapshot {
    pub tray_title: String,
    pub tray_accessibility_label: String,
    pub query: String,
    pub content: MenuContent,
    pub refreshing: bool,
    pub updated_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum MenuBarIntent {
    SetQuery {
        query: String,
    },
    OpenTicket {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    /// Opens the ticket's Resolve form in its own window, without the main window (as New ticket).
    Resolve {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    OpenMain,
    NewTicket,
    Refresh,
    /// Esc in the panel: hides it, like a click outside it.
    Close,
}

impl Workspace {
    fn menu_row(&self, st: &State, t: &itop_kit::Ticket, now: DateTime<Utc>) -> MenuRow {
        let me = self.me();
        let timer = &st.timer;
        let id = t.id();
        let running = timer.is_running(t);
        MenuRow {
            ticket_id: id.clone(),
            r#ref: t.r#ref.clone(),
            title: t.title.clone(),
            needs_you: TicketList::needs_you(t, me),
            is_running: running,
            can_resolve: itop_kit::TicketLifecycle::actions(&t.status)
                .contains(&itop_kit::TicketAction::Resolve),
            right: if running {
                MenuRowRight::Running {
                    counter: timer.running.as_ref().map_or_else(String::new, |r| {
                        TimeFormat::counter((now - r.start).num_seconds().max(0))
                    }),
                }
            } else {
                match timer.totals.get(&id) {
                    Some(&s) if s > 0 => MenuRowRight::Spent {
                        text: TimeFormat::total(s),
                    },
                    _ => MenuRowRight::None,
                }
            },
            button: if running {
                MenuRowButton::Stop
            } else if timer.can_start(t, me) && !timer.web_stopwatches.contains_key(&id) {
                MenuRowButton::Start {
                    enabled: !timer.saving,
                    help: timer.running.as_ref().map_or_else(
                        || "Start timer".to_owned(),
                        |r| {
                            format!(
                                "Stops the timer on {}, saves its time, and starts one here",
                                r.r#ref
                            )
                        },
                    ),
                }
            } else {
                MenuRowButton::Slot
            },
        }
    }

    pub(crate) fn menubar_snapshot(&self, st: &State, now: DateTime<Utc>) -> MenuBarSnapshot {
        let me = self.me();
        let mine = &st.lists[0];
        let (title, label) = self.tray_status(st);
        let query = &st.menubar.query;
        let content = match &mine.phase {
            Phase::Loading => MenuContent::Loading,
            Phase::Failed(m) => MenuContent::Failed {
                text: format!("Couldn't load your tickets. {m}"),
            },
            Phase::Loaded => {
                // MB-8: My tickets minus resolved/closed, the panel's search, the shared sort, only
                // Escalated TTR, Escalated TTO, Assigned and Pending.
                let open: Vec<itop_kit::Ticket> = mine
                    .tickets
                    .iter()
                    .filter(|t| t.status != "resolved" && t.status != "closed")
                    .cloned()
                    .collect();
                let filtered = TicketSearch::filter_query(&open, query);
                let groups = TicketList::groups(
                    &filtered,
                    me,
                    itop_kit::TicketListView::Mine,
                    st.options.sort,
                );
                let sections: Vec<itop_kit::TicketSection> = menu_bar_status::GROUP_ORDER
                    .iter()
                    .filter_map(|g| groups.iter().find(|s| s.group == *g).cloned())
                    .collect();
                if sections.is_empty() {
                    MenuContent::Empty {
                        text: if query.is_empty() {
                            "Nothing waiting on you. New assignments show up here.".to_owned()
                        } else {
                            format!("No tickets of yours match “{query}”.")
                        },
                    }
                } else {
                    let running_id = st.timer.running.as_ref().map(|r| r.ticket_id());
                    let pinned = menu_bar_status::pin_running(&sections, running_id.as_deref());
                    let rows: i64 = sections.iter().map(|s| s.tickets.len() as i64).sum();
                    let height = 420.min(
                        (pinned.sections.len() as i64 + i64::from(pinned.running.is_some())) * 30
                            + rows * 48
                            + 6,
                    );
                    MenuContent::List {
                        running: pinned.running.as_ref().map(|t| self.menu_row(st, t, now)),
                        sections: pinned
                            .sections
                            .iter()
                            .map(|s| MenuSection {
                                group: s.group.into(),
                                title: format!("{}  {}", s.group.title(), s.tickets.len()),
                                rows: s
                                    .tickets
                                    .iter()
                                    .map(|t| self.menu_row(st, t, now))
                                    .collect(),
                            })
                            .collect(),
                        height,
                    }
                }
            }
        };
        MenuBarSnapshot {
            tray_title: title,
            tray_accessibility_label: label,
            query: query.clone(),
            content,
            refreshing: mine.refreshing,
            updated_text: mine.updated_at.map(|at| {
                format!(
                    "Updated {}",
                    self.0
                        .fmt
                        .relative_named(self.0.ctx.native.as_ref(), at, now)
                )
            }),
        }
    }

    pub(crate) async fn dispatch_menubar(
        &self,
        intent: MenuBarIntent,
    ) -> Result<serde_json::Value, AppError> {
        let native = self.0.ctx.native.clone();
        match intent {
            MenuBarIntent::SetQuery { query } => {
                self.lock().menubar.query = query;
                self.publish();
            }
            // `MenuBarPanelHost.openTicket` (MB-12): My tickets, its search cleared, the ticket
            // selected, the main window forward.
            // Like New ticket: only the Resolve window, the main window untouched.
            MenuBarIntent::Resolve { ticket_id } => {
                native.hide_menubar_panel();
                self.dispatch_resolve(
                    super::sheets::ResolveAt::Window,
                    super::sheets::ResolveSheetIntent::Open { ticket_id },
                )
                .await?;
            }
            MenuBarIntent::OpenTicket { ticket_id } => {
                let known = self.lock().lists[0]
                    .tickets
                    .iter()
                    .any(|t| t.id() == ticket_id);
                if !known {
                    return Err(AppError::invalid_intent(format!(
                        "No ticket {ticket_id} in My tickets."
                    )));
                }
                self.shell_select(SidebarItem::MyTickets).await;
                self.set_query(super::records::ListScope::Mine, "");
                self.lock().lists[0].selection = Some(ticket_id);
                self.publish();
                native.hide_menubar_panel();
                native.show_main_window();
            }
            MenuBarIntent::OpenMain => {
                native.hide_menubar_panel();
                native.show_main_window();
            }
            // NEW-2: the separate New Ticket window, without the main window.
            MenuBarIntent::NewTicket => {
                native.hide_menubar_panel();
                if let Some(s) = self.standalone().cloned() {
                    s.new_tickets
                        .request(NewTicketInstance::Window, NewTicketStart::default())
                        .await;
                }
            }
            MenuBarIntent::Refresh => self.list_refresh(super::records::ListScope::Mine).await,
            MenuBarIntent::Close => native.hide_menubar_panel(),
        }
        Ok(serde_json::Value::Null)
    }
}
