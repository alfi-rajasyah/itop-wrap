//! Port of `SessionModel.selection` + `ShellView`: the sidebar, the account menu and the routing
//! between sections (SHELL-*, AUTH-27…29).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::records::{ConfirmText, LinkedCI, ListScope, SidebarItem, TicketLink};
use super::new_ticket::{NewTicketInstance, NewTicketStart};
use itop_kit::ConfigItem;
use super::workspace::{State, Workspace};
use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct ShellState {
    pub selection: SidebarItem,
}

impl Default for ShellState {
    /// My tickets at launch and after sign-in (SHELL-6).
    fn default() -> Self {
        ShellState { selection: SidebarItem::MyTickets }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct SidebarRow {
    pub item: SidebarItem,
    pub title: String,
    pub count: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ShellAccount {
    pub initials: String,
    pub display_name: String,
    pub host: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ShellSnapshot {
    pub selection: SidebarItem,
    pub sidebar: Vec<SidebarRow>,
    pub content_title: String,
    pub need_count: i64,
    pub account: ShellAccount,
    pub sign_out_confirm: ConfirmText,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ShellIntent {
    Select {
        item: SidebarItem,
    },
    OpenTicket {
        ticket: TicketLink,
    },
    OpenOrg {
        #[serde(rename = "orgId")]
        #[ts(rename = "orgId")]
        org_id: String,
        #[serde(rename = "orgName")]
        #[ts(rename = "orgName")]
        org_name: String,
    },
    #[serde(rename = "openCI")]
    #[ts(rename = "openCI")]
    OpenCi {
        ci: LinkedCI,
    },
    OpenOrgPeople {
        #[serde(rename = "orgId")]
        #[ts(rename = "orgId")]
        org_id: String,
        #[serde(rename = "orgName")]
        #[ts(rename = "orgName")]
        org_name: String,
    },
    OpenOrgConfigItems {
        #[serde(rename = "orgId")]
        #[ts(rename = "orgId")]
        org_id: String,
        #[serde(rename = "orgName")]
        #[ts(rename = "orgName")]
        org_name: String,
    },
    NewTicket,
    NewTicketForOrg {
        #[serde(rename = "orgId")]
        #[ts(rename = "orgId")]
        org_id: String,
        #[serde(rename = "orgName")]
        #[ts(rename = "orgName")]
        org_name: String,
    },
    #[serde(rename = "newTicketForCI")]
    #[ts(rename = "newTicketForCI")]
    NewTicketForCi {
        key: String,
        name: String,
        #[serde(rename = "orgName")]
        #[ts(rename = "orgName")]
        org_name: String,
    },
}

/// The server's host, or "iTop" (W/ShellView.swift:143).
pub fn account_host(base_host: &str) -> String {
    if base_host.is_empty() { "iTop".to_owned() } else { base_host.to_owned() }
}

impl Workspace {
    pub(crate) fn shell_snapshot(&self, st: &State) -> ShellSnapshot {
        let me = self.me();
        let need = st.lists[0].need_count(me);
        let host = account_host(self.session().client.server.host());
        ShellSnapshot {
            selection: st.shell.selection,
            sidebar: SidebarItem::ALL
                .iter()
                .map(|&item| SidebarRow {
                    item,
                    title: item.title().to_owned(),
                    count: (item == SidebarItem::MyTickets && need > 0).then_some(need),
                })
                .collect(),
            content_title: st.shell.selection.title().to_owned(),
            need_count: need,
            account: ShellAccount { initials: me.initials(), display_name: me.display_name.clone(), host: host.clone() },
            sign_out_confirm: ConfirmText {
                title: format!("Sign out of {host}?"),
                message: "Your password is removed from the Keychain. Your username stays filled in.".to_owned(),
            },
        }
    }

    pub(crate) async fn dispatch_shell(&self, intent: ShellIntent) -> Result<serde_json::Value, AppError> {
        let s = self.standalone().cloned();
        match intent {
            ShellIntent::Select { item } => self.shell_select(item).await,
            ShellIntent::OpenTicket { ticket } => self.shell_open_ticket(&ticket.ticket_id, &ticket.r#ref).await,
            // SHELL-10: the org first, then the section, so it loads once.
            ShellIntent::OpenOrg { org_id, .. } => {
                if let Some(s) = &s {
                    s.orgs.show_org(&org_id);
                }
                self.shell_select(SidebarItem::Organizations).await;
            }
            // SHELL-11: the linked CI as the current CI, its org empty.
            ShellIntent::OpenCi { ci } => {
                if let Some(s) = &s {
                    s.cmdb.open(ConfigItem { key: ci.key, class_name: ci.class_name, name: ci.name, org_name: String::new() });
                }
                self.shell_select(SidebarItem::ConfigItems).await;
            }
            ShellIntent::OpenOrgPeople { org_id, org_name } => {
                if let Some(s) = &s {
                    s.people.show_org(&org_id, &org_name);
                }
                self.shell_select(SidebarItem::People).await;
            }
            ShellIntent::OpenOrgConfigItems { org_id, org_name } => {
                if let Some(s) = &s {
                    s.cmdb.show_org(&org_id, &org_name);
                }
                self.shell_select(SidebarItem::ConfigItems).await;
            }
            ShellIntent::NewTicket => self.request_new_ticket(NewTicketStart::default()).await,
            ShellIntent::NewTicketForOrg { org_id, org_name } => {
                self.request_new_ticket(NewTicketStart::for_org(&org_id, &org_name)).await
            }
            ShellIntent::NewTicketForCi { key, name, org_name } => {
                self.request_new_ticket(NewTicketStart::for_ci(&key, &name, &org_name)).await
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// Swift `Workspace.requestNewTicket(_:)`: New ticket as a sheet in the main window.
    pub(crate) async fn request_new_ticket(&self, start: NewTicketStart) {
        if let Some(s) = self.standalone().cloned() {
            s.new_tickets.request(NewTicketInstance::Sheet, start).await;
        }
    }

    /// Sidebar selection (and View ▸ ⌘1…5). The section that leaves disappears and the one that
    /// comes runs its views' on-appear work; Team tickets stops polling when it leaves.
    pub(crate) async fn shell_select(&self, item: SidebarItem) {
        let old = {
            let mut st = self.lock();
            let old = st.shell.selection;
            st.shell.selection = item;
            old
        };
        if old == item {
            self.publish();
            return;
        }
        self.trace(|| format!("sidebar selection applied: {}", item.raw()));
        if old == SidebarItem::TeamTickets
            && let Some(h) = self.tasks().team_poll.take()
        {
            h.abort();
        }
        if let Some(s) = self.standalone() {
            s.section(old, false);
            s.section(item, true);
        }
        self.section_appeared(item);
        self.publish();
    }

    /// What a list view does when it appears (`.task` and `.task(id: query)` in TicketListView):
    /// load if not loaded, Team tickets polls every 120 s, and the search re-runs.
    pub(crate) fn section_appeared(&self, item: SidebarItem) {
        let Some(scope) = item.list() else { return };
        self.restart_search(scope);
        let ws = self.clone();
        let handle = self.spawn(async move {
            let loaded = matches!(ws.lock().lists[scope.index()].phase, super::list::Phase::Loaded);
            if !loaded {
                ws.list_refresh(scope).await;
            }
            if scope != ListScope::Team {
                return;
            }
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                ws.list_refresh(scope).await;
            }
        });
        if scope == ListScope::Team
            && let Some(old) = self.tasks().team_poll.replace(handle)
        {
            old.abort();
        }
    }

    /// `ShellView.openTicket` (SHELL-9): in the list that has it, else as a search in My tickets.
    pub(crate) async fn shell_open_ticket(&self, ticket_id: &str, r#ref: &str) {
        let target = {
            let st = self.lock();
            if st.lists[0].tickets.iter().any(|t| t.id() == ticket_id) {
                Some(ListScope::Mine)
            } else if st.lists[1].tickets.iter().any(|t| t.id() == ticket_id) {
                Some(ListScope::Team)
            } else {
                None
            }
        };
        match target {
            Some(scope) => {
                self.set_query(scope, "");
                self.lock().lists[scope.index()].selection = Some(ticket_id.to_owned());
                self.shell_select(scope.section()).await;
            }
            None => {
                self.set_query(ListScope::Mine, r#ref);
                self.shell_select(SidebarItem::MyTickets).await;
            }
        }
    }
}
