//! `orgs`: the Organizations section. Port of `OrgsModel` (W/OrgsModel.swift:37-91) plus the
//! rules of `OrgsListView` and `OrgDetailView` (W/OrgsView.swift:111-216). contract.md §5.12.
//!
//! SwiftUI loads the list in `OrgsListView.task` and the selected org in
//! `OrgDetailView.task(id: org.id)`, both when the views appear. The shell reports that with
//! [`OrgsModel::appear`] / [`OrgsModel::disappear`]; an intent always acts as if the section is
//! on screen.

use std::sync::{Arc, Mutex, MutexGuard, Weak};

use itop_kit::{OrgCounts, OrgService, Organization, Ticket, TicketScope, TicketSort};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::{AbortHandle, JoinHandle};
use ts_rs::TS;

use super::cmdb::text::{self, Loadable, describe, trim_whitespaces};
use super::seams::{Model, ModelContext, WorkspaceLink};
use crate::error::AppError;

#[path = "orgs_table.rs"]
pub mod table;
#[cfg(test)]
#[path = "orgs_tests.rs"]
mod tests;

use table::{LastColumn, TableOptions, TableState};

pub const MODEL_ID: &str = "orgs";

// MARK: - Contract shapes

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgRow {
    pub id: String,
    pub name: String,
    pub code: String,
    pub open_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum OrgList {
    Loading,
    Failed { text: String },
    Empty { text: String },
    Loaded { orgs: Vec<OrgRow> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgDetailView {
    pub id: String,
    pub name: String,
    pub code: String,
    /// "Organization · A" or "Organization".
    pub header_line: String,
    /// "People 6" or "People".
    pub people_label: String,
    /// "Config items 3" or "Config items".
    pub config_items_label: String,
    /// "Open tickets" / "Tickets".
    pub tickets_heading: String,
    pub tickets: TableState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgsSnapshot {
    pub query: String,
    pub list: OrgList,
    pub selected_id: Option<String>,
    pub detail: Option<OrgDetailView>,
    pub placeholder: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum OrgsIntent {
    SetQuery { query: String },
    Select { id: Option<String> },
    RetryList,
    RetryTickets,
    SetTicketPage { page: i32 },
}

// MARK: - State

struct State {
    orgs: Loadable<Vec<Organization>>,
    query: String,
    selected_id: Option<String>,
    tickets: Loadable<Vec<Ticket>>,
    counts: Option<OrgCounts>,
    table: TableOptions,
    /// `options.scope` as of the last ticket load (heading and empty text).
    scope: TicketScope,
    /// The Organizations section is on screen.
    visible: bool,
    /// The org whose detail view is alive (`OrgDetailView.task(id: org.id)`).
    shown_id: Option<String>,
    list_generation: u64,
    list_task: Option<AbortHandle>,
    detail_generation: u64,
    detail_task: Option<AbortHandle>,
}

impl State {
    /// `selected`: the chosen org, once the list has it.
    fn selected(&self) -> Option<&Organization> {
        let all = self.orgs.value()?;
        all.iter()
            .find(|o| Some(&o.id) == self.selected_id.as_ref())
    }

    /// `visible`: filtered locally on "<name> <code>" (ORG-3).
    fn visible_orgs(&self) -> Vec<&Organization> {
        let Some(all) = self.orgs.value() else {
            return Vec::new();
        };
        let q = trim_whitespaces(&self.query).to_lowercase();
        if q.is_empty() {
            return all.iter().collect();
        }
        all.iter()
            .filter(|o| format!("{} {}", o.name, o.code).to_lowercase().contains(&q))
            .collect()
    }
}

pub struct OrgsModel {
    ctx: ModelContext,
    link: Arc<dyn WorkspaceLink>,
    me: Weak<OrgsModel>,
    state: Mutex<State>,
}

impl OrgsModel {
    pub fn new(ctx: ModelContext, link: Arc<dyn WorkspaceLink>) -> Arc<Self> {
        let model = Arc::new_cyclic(|me| OrgsModel {
            ctx,
            link,
            me: me.clone(),
            state: Mutex::new(State {
                orgs: Loadable::Idle,
                query: String::new(),
                selected_id: None,
                tickets: Loadable::Idle,
                counts: None,
                table: TableOptions::default(),
                scope: TicketScope::OPEN,
                visible: false,
                shown_id: None,
                list_generation: 0,
                list_task: None,
                detail_generation: 0,
                detail_task: None,
            }),
        });
        model.publish();
        model
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn publish(&self) {
        let st = self.lock();
        self.ctx.hub.publish(MODEL_ID, &self.build(&st));
    }

    pub fn snapshot(&self) -> OrgsSnapshot {
        self.build(&self.lock())
    }

    /// The model stopped existing (sign-out).
    pub fn clear(&self) {
        let mut st = self.lock();
        for h in [st.list_task.take(), st.detail_task.take()]
            .into_iter()
            .flatten()
        {
            h.abort();
        }
        drop(st);
        self.ctx.hub.clear(MODEL_ID);
    }

    // MARK: Cross-model calls

    /// Organizations became the shown section: the list loads once (ORG-2) and the selected
    /// org's detail loads again (`.task(id:)` runs on every appearance).
    pub fn appear(&self) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.visible = true;
        self.start_list(&mut st);
        st.shown_id = None;
        self.sync_detail(&mut st, true, scope);
        drop(st);
        self.publish();
    }

    /// Another section was chosen: the views' tasks are cancelled.
    pub fn disappear(&self) {
        let mut st = self.lock();
        st.visible = false;
        st.shown_id = None;
        st.list_generation += 1;
        st.detail_generation += 1;
        for h in [st.list_task.take(), st.detail_task.take()]
            .into_iter()
            .flatten()
        {
            h.abort();
        }
        drop(st);
        self.publish();
    }

    /// `ShellView.openOrg`: "See all of its tickets" and a person's organization (SHELL-10): the
    /// search is cleared and the org selected. The shell then selects Organizations.
    pub fn show_org(&self, id: &str) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.query.clear();
        st.selected_id = Some(id.to_owned());
        let active = st.visible;
        self.sync_detail(&mut st, active, scope);
        drop(st);
        self.publish();
    }

    /// The View range or "Include resolved and closed" changed while Organizations is shown:
    /// `OrgDetailView.onChange(of: options.scope)` reloads the selected org (ORG-9).
    pub fn scope_changed(&self) {
        let scope = self.link.scope();
        let mut st = self.lock();
        if st.shown_id.is_some() {
            self.start_detail(&mut st, scope);
        }
        drop(st);
        self.publish();
    }

    /// The shared sort or page size (ListOptions); also call it once at start with the saved
    /// values. A change sends the table back to page 1 (ORG-11).
    pub fn set_table_options(&self, sort: TicketSort, page_size: Option<i64>) {
        let changed = self.lock().table.set_view(sort, page_size);
        if changed {
            self.publish();
        }
    }

    // MARK: Intents

    fn set_query(&self, query: String) {
        self.lock().query = query;
        self.publish();
    }

    fn select(&self, id: Option<String>) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.selected_id = id;
        self.sync_detail(&mut st, true, scope);
        drop(st);
        self.publish();
    }

    /// "Retry" under a failed list: `reloadList()`.
    async fn retry_list(&self) {
        let handle = {
            let mut st = self.lock();
            st.orgs = Loadable::Idle;
            self.start_list(&mut st)
        };
        self.publish();
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    /// "Retry" under failed tickets: `loadSelected()`.
    async fn retry_tickets(&self) {
        let scope = self.link.scope();
        let handle = {
            let mut st = self.lock();
            self.start_detail(&mut st, scope)
        };
        self.publish();
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    fn set_ticket_page(&self, page: i32) {
        self.lock().table.page = i64::from(page);
        self.publish();
    }

    // MARK: Loading

    /// `loadList()`: does nothing once loaded.
    fn start_list(&self, st: &mut State) -> Option<JoinHandle<()>> {
        if matches!(st.orgs, Loadable::Loaded(_)) {
            return None;
        }
        st.orgs = Loadable::Loading;
        st.list_generation += 1;
        if let Some(h) = st.list_task.take() {
            h.abort();
        }
        let generation = st.list_generation;
        let me = self.me.clone();
        let handle = tokio::spawn(async move {
            let Some(model) = me.upgrade() else { return };
            let host = text::host(&model.ctx.session);
            let answer = OrgService::new(model.ctx.session.client.clone())
                .organizations()
                .await;
            let scope = model.link.scope();
            {
                let mut st = model.lock();
                if st.list_generation != generation {
                    return;
                }
                st.orgs = match answer {
                    Ok(list) => Loadable::Loaded(list),
                    Err(e) => Loadable::Failed(describe(&e, &host)),
                };
                // The selected org can show now: its detail view appears and loads.
                model.sync_detail(&mut st, true, scope);
            }
            model.publish();
        });
        st.list_task = Some(handle.abort_handle());
        Some(handle)
    }

    /// `OrgDetailView.task(id: org.id)`: a newly shown org loads.
    fn sync_detail(&self, st: &mut State, active: bool, scope: TicketScope) {
        if !active {
            return;
        }
        let shown = st.selected().map(|o| o.id.clone());
        if shown != st.shown_id {
            st.shown_id = shown.clone();
            if shown.is_some() {
                self.start_detail(st, scope);
            }
        }
    }

    /// `loadSelected()` (W/OrgsModel.swift:78-90): tickets and counts together, tickets first.
    fn start_detail(&self, st: &mut State, scope: TicketScope) -> Option<JoinHandle<()>> {
        let id = st.selected_id.clone()?;
        st.detail_generation += 1;
        if let Some(h) = st.detail_task.take() {
            h.abort();
        }
        st.tickets = Loadable::Loading;
        st.counts = None;
        st.scope = scope;
        // The table is rebuilt after loading, so it starts on page 1.
        st.table.page = 1;
        let generation = st.detail_generation;
        let me = self.me.clone();
        let handle = tokio::spawn(async move {
            let Some(model) = me.upgrade() else { return };
            model.publish();
            let host = text::host(&model.ctx.session);
            let service = OrgService::new(model.ctx.session.client.clone());
            let counter = service.clone();
            let counts_id = id.clone();
            let counts = tokio::spawn(async move { counter.counts(&counts_id).await });
            let tickets = service.open_tickets(&id, &scope).await;
            {
                let mut st = model.lock();
                if st.detail_generation != generation {
                    counts.abort();
                    return;
                }
                match tickets {
                    Ok(t) => st.tickets = Loadable::Loaded(t),
                    Err(e) => {
                        // `tickets = .failed`; the counts stay unknown.
                        st.tickets = Loadable::Failed(describe(&e, &host));
                        counts.abort();
                        drop(st);
                        model.publish();
                        return;
                    }
                }
            }
            model.publish();
            // `counts = try? await c`.
            let Ok(answer) = counts.await else { return };
            let mut st = model.lock();
            if st.detail_generation != generation {
                return;
            }
            st.counts = answer.ok();
            drop(st);
            model.publish();
        });
        st.detail_task = Some(handle.abort_handle());
        Some(handle)
    }

    // MARK: Snapshot

    fn build(&self, st: &State) -> OrgsSnapshot {
        OrgsSnapshot {
            query: st.query.clone(),
            list: Self::list_view(st),
            selected_id: st.selected_id.clone(),
            detail: st.selected().map(|org| self.detail_view(st, org)),
            placeholder: "Select an organization to see its open tickets.".to_owned(),
        }
    }

    fn list_view(st: &State) -> OrgList {
        match &st.orgs {
            Loadable::Idle | Loadable::Loading => OrgList::Loading,
            Loadable::Failed(e) => OrgList::Failed {
                text: format!("Couldn't load organizations. {e}"),
            },
            Loadable::Loaded(_) => {
                let orgs = st.visible_orgs();
                if orgs.is_empty() {
                    OrgList::Empty {
                        text: if st.query.is_empty() {
                            "Your iTop account can't see any organizations.".to_owned()
                        } else {
                            format!(
                                "No organization matches \u{201C}{}\u{201D}.",
                                trim_whitespaces(&st.query)
                            )
                        },
                    }
                } else {
                    OrgList::Loaded {
                        orgs: orgs
                            .into_iter()
                            .map(|o| OrgRow {
                                id: o.id.clone(),
                                name: o.name.clone(),
                                code: o.code.clone(),
                                open_count: o.open_count as i32,
                            })
                            .collect(),
                    }
                }
            }
        }
    }

    fn detail_view(&self, st: &State, org: &Organization) -> OrgDetailView {
        let tickets = match &st.tickets {
            Loadable::Idle | Loadable::Loading => TableState::Loading {
                text: format!("Loading {}'s tickets", org.name),
            },
            Loadable::Failed(e) => TableState::Failed {
                text: format!("Couldn't load the tickets. {e}"),
                can_retry: true,
            },
            Loadable::Loaded(t) if t.is_empty() => TableState::Empty {
                text: if st.scope == TicketScope::OPEN {
                    format!("{} has no open tickets.", org.name)
                } else {
                    format!("{} has no tickets in this range.", org.name)
                },
            },
            Loadable::Loaded(t) => TableState::Loaded {
                table: table::table(t, &self.ctx.session.user, LastColumn::Assignee, &st.table),
            },
        };
        OrgDetailView {
            id: org.id.clone(),
            name: org.name.clone(),
            code: org.code.clone(),
            header_line: if org.code.is_empty() {
                "Organization".to_owned()
            } else {
                format!("Organization · {}", org.code)
            },
            people_label: st
                .counts
                .map_or_else(|| "People".to_owned(), |c| format!("People {}", c.people)),
            config_items_label: st.counts.map_or_else(
                || "Config items".to_owned(),
                |c| format!("Config items {}", c.config_items),
            ),
            tickets_heading: if st.scope.include_closed {
                "Tickets"
            } else {
                "Open tickets"
            }
            .to_owned(),
            tickets,
        }
    }
}

#[async_trait::async_trait]
impl Model for OrgsModel {
    async fn dispatch(&self, model_id: &str, intent: Value) -> Result<Value, AppError> {
        if model_id != MODEL_ID {
            return Err(AppError::unknown_model(model_id));
        }
        let intent: OrgsIntent =
            serde_json::from_value(intent).map_err(AppError::invalid_intent)?;
        match intent {
            OrgsIntent::SetQuery { query } => self.set_query(query),
            OrgsIntent::Select { id } => self.select(id),
            OrgsIntent::RetryList => self.retry_list().await,
            OrgsIntent::RetryTickets => self.retry_tickets().await,
            OrgsIntent::SetTicketPage { page } => self.set_ticket_page(page),
        }
        Ok(Value::Null)
    }
}
