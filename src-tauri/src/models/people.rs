//! `people`: the People section. Port of `PeopleModel` (W/OrgsModel.swift:93-179) plus the rules
//! of `PeopleListView` and `PersonDetailView` (W/OrgsView.swift:220-365). contract.md §5.13.
//!
//! As in the CMDB model, the search is SwiftUI's `.task(id: query + scope id)` and the detail load
//! `PersonDetailView.task(id: person.id)`; the shell reports the section appearing and going with
//! [`PeopleModel::appear`] / [`PeopleModel::disappear`], and intents act as if it is on screen.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use itop_kit::{OrgService, PeoplePage, Person, Ticket, TicketScope, TicketSort};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::cmdb::text::{
    self, Loadable, OrgScope, SearchRun, character_count, describe, trim_whitespaces,
};
use super::orgs::table::{self, LastColumn, TableOptions, TableState};
use super::seams::{Model, ModelContext, WorkspaceLink};
use crate::error::AppError;

#[cfg(test)]
#[path = "people_tests.rs"]
mod tests;

pub const MODEL_ID: &str = "people";

// MARK: - Contract shapes

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PeopleTab {
    Assigned,
    Reported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct PersonRow {
    pub id: String,
    pub display_name: String,
    pub initials: String,
    /// "Agent · Network, Service desk" / "Caller · Org A".
    pub subtitle: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PeopleResults {
    Idle {
        text: String,
    },
    Searching,
    Failed {
        text: String,
    },
    Empty {
        text: String,
    },
    /// `banner`: "Showing 100 of 240 people. …" when more were found than shown.
    Loaded {
        banner: Option<String>,
        people: Vec<PersonRow>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct PersonDetailView {
    pub id: String,
    pub display_name: String,
    pub initials: String,
    /// "Agent · IT support", "Caller".
    pub kind_line: String,
    pub organization: Option<OrgScope>,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// Agents only: "Network, Service desk".
    pub teams: Option<String>,
    pub tab: PeopleTab,
    /// "Assigned to them 3" (count once loaded).
    pub assigned_label: String,
    pub reported_label: String,
    pub tickets: TableState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct PeopleSnapshot {
    pub query: String,
    pub scope: Option<OrgScope>,
    pub results: PeopleResults,
    pub selected_id: Option<String>,
    pub detail: Option<PersonDetailView>,
    pub placeholder: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PeopleIntent {
    SetQuery { query: String },
    ClearScope,
    Select { id: Option<String> },
    SetTab { tab: PeopleTab },
    SetTicketPage { page: i32 },
}

// MARK: - State

type Results = Loadable<Vec<Person>>;

struct State {
    query: String,
    /// Set by the org view's People link; typing a search clears it.
    scope: Option<(String, String)>,
    results: Results,
    /// How many iTop found in total; more than shown when the search hit the 100-person cap.
    total: i64,
    selected_id: Option<String>,
    tab: PeopleTab,
    assigned: Loadable<Vec<Ticket>>,
    reported: Loadable<Vec<Ticket>>,
    searched: Option<String>,
    table: TableOptions,
    /// `options.scope` as of the last ticket load (empty texts).
    ticket_scope: TicketScope,
    visible: bool,
    search: SearchRun<Results>,
    /// The person whose detail view is alive (`PersonDetailView.task(id:)`).
    shown_id: Option<String>,
    detail_generation: u64,
    detail_task: Option<tokio::task::AbortHandle>,
}

impl State {
    fn selected(&self) -> Option<&Person> {
        let all = self.results.value()?;
        all.iter()
            .find(|p| Some(&p.id) == self.selected_id.as_ref())
    }

    fn current_tickets(&self) -> &Loadable<Vec<Ticket>> {
        match self.tab {
            PeopleTab::Assigned => &self.assigned,
            PeopleTab::Reported => &self.reported,
        }
    }
}

enum Fetch {
    InOrg(String),
    Text(String),
}

pub struct PeopleModel {
    ctx: ModelContext,
    link: Arc<dyn WorkspaceLink>,
    me: Weak<PeopleModel>,
    state: Mutex<State>,
}

impl PeopleModel {
    pub fn new(ctx: ModelContext, link: Arc<dyn WorkspaceLink>) -> Arc<Self> {
        let model = Arc::new_cyclic(|me| PeopleModel {
            ctx,
            link,
            me: me.clone(),
            state: Mutex::new(State {
                query: String::new(),
                scope: None,
                results: Loadable::Idle,
                total: 0,
                selected_id: None,
                tab: PeopleTab::Assigned,
                assigned: Loadable::Idle,
                reported: Loadable::Idle,
                searched: None,
                table: TableOptions::default(),
                ticket_scope: TicketScope::OPEN,
                visible: false,
                search: SearchRun::default(),
                shown_id: None,
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

    pub fn snapshot(&self) -> PeopleSnapshot {
        self.build(&self.lock())
    }

    /// The model stopped existing (sign-out).
    pub fn clear(&self) {
        let mut st = self.lock();
        let State {
            search, results, ..
        } = &mut *st;
        search.cancel(results);
        if let Some(h) = st.detail_task.take() {
            h.abort();
        }
        drop(st);
        self.ctx.hub.clear(MODEL_ID);
    }

    // MARK: Cross-model calls

    /// People became the shown section: the search task and the shown person's detail run.
    pub fn appear(&self) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.visible = true;
        self.start_search(&mut st, scope);
        st.shown_id = None;
        self.sync_detail(&mut st, true, scope);
        drop(st);
        self.publish();
    }

    /// Another section was chosen: the views' tasks are cancelled.
    pub fn disappear(&self) {
        let mut st = self.lock();
        st.visible = false;
        let State {
            search, results, ..
        } = &mut *st;
        search.cancel(results);
        st.search.task_id = None;
        st.detail_generation += 1;
        if let Some(h) = st.detail_task.take() {
            h.abort();
        }
        st.shown_id = None;
        drop(st);
        self.publish();
    }

    /// `showOrg(id:name:)`: the org view's "People" link (SHELL-12, PPL-5). The shell then
    /// selects People.
    pub fn show_org(&self, id: &str, name: &str) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.scope = Some((id.to_owned(), name.to_owned()));
        st.query.clear();
        st.searched = None;
        st.selected_id = None;
        let active = st.visible;
        self.sync_detail(&mut st, active, scope);
        self.task_id_changed(&mut st, active, scope);
        drop(st);
        self.publish();
    }

    /// The View range or "Include resolved and closed" changed while People is shown:
    /// `PersonDetailView.onChange(of: options.scope)` reloads, keeping the tab (PPL-10).
    pub fn scope_changed(&self) {
        let scope = self.link.scope();
        let mut st = self.lock();
        if st.shown_id.is_some() {
            self.start_detail(&mut st, true, scope);
        }
        drop(st);
        self.publish();
    }

    /// The shared sort or page size (ListOptions); also call it once at start with the saved
    /// values. A change sends the table back to page 1.
    pub fn set_table_options(&self, sort: TicketSort, page_size: Option<i64>) {
        let changed = self.lock().table.set_view(sort, page_size);
        if changed {
            self.publish();
        }
    }

    // MARK: Intents

    fn set_query(&self, query: String) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.query = query;
        self.task_id_changed(&mut st, true, scope);
        drop(st);
        self.publish();
    }

    fn clear_scope(&self) {
        let scope = self.link.scope();
        let mut st = self.lock();
        st.scope = None;
        st.results = Loadable::Idle;
        st.searched = None;
        self.sync_detail(&mut st, true, scope);
        self.task_id_changed(&mut st, true, scope);
        drop(st);
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

    /// The segmented picker. The table starts on page 1 when it shows other tickets.
    fn set_tab(&self, tab: PeopleTab) {
        let mut st = self.lock();
        if st.tab != tab {
            let before = st.current_tickets().value().cloned();
            st.tab = tab;
            let after = st.current_tickets().value().cloned();
            if before.is_none() || before != after {
                st.table.page = 1;
            }
        }
        drop(st);
        self.publish();
    }

    fn set_ticket_page(&self, page: i32) {
        self.lock().table.page = i64::from(page);
        self.publish();
    }

    // MARK: Search (`search()`, W/OrgsModel.swift:138-165)

    fn task_id(st: &State) -> String {
        format!(
            "{}{}",
            st.query,
            st.scope.as_ref().map_or("", |s| s.0.as_str())
        )
    }

    fn task_id_changed(&self, st: &mut State, active: bool, scope: TicketScope) {
        if active && st.search.task_id.as_deref() != Some(Self::task_id(st).as_str()) {
            self.start_search(st, scope);
        }
    }

    fn start_search(&self, st: &mut State, scope: TicketScope) {
        let State {
            search, results, ..
        } = &mut *st;
        search.cancel(results);
        let fetch = Self::search_start(st);
        st.search.task_id = Some(Self::task_id(st));
        // Results changed (idle, searching, restored): the selected person may be gone or back.
        self.sync_detail(st, true, scope);
        let Some(fetch) = fetch else { return };
        let generation = st.search.generation;
        let me = self.me.clone();
        let handle = tokio::spawn(async move {
            if let Some(model) = me.upgrade() {
                model.search_finish(generation, fetch).await;
            }
        });
        st.search.handle = Some(handle.abort_handle());
    }

    fn search_start(st: &mut State) -> Option<Fetch> {
        let q = trim_whitespaces(&st.query).to_owned();
        if !q.is_empty() && st.scope.is_some() {
            st.scope = None;
        }
        if let Some((id, _)) = &st.scope {
            let key = format!("org:{id}");
            if st.searched.as_deref() == Some(key.as_str())
                && matches!(st.results, Loadable::Loaded(_))
            {
                return None;
            }
            st.results = Loadable::Loading;
            return Some(Fetch::InOrg(id.clone()));
        }
        if character_count(&q) < 2 {
            st.results = Loadable::Idle;
            st.searched = None;
            return None;
        }
        if st.searched.as_deref() == Some(q.as_str()) && matches!(st.results, Loadable::Loaded(_)) {
            return None;
        }
        st.search.previous = Some(std::mem::replace(&mut st.results, Loadable::Loading));
        Some(Fetch::Text(q))
    }

    async fn search_finish(self: Arc<Self>, generation: u64, fetch: Fetch) {
        self.publish();
        let service = OrgService::new(self.ctx.session.client.clone());
        let host = text::host(&self.ctx.session);
        let apply = |st: &mut State, page: PeoplePage, searched: String| {
            st.results = Loadable::Loaded(page.people);
            st.total = page.total;
            st.searched = Some(searched);
        };
        match fetch {
            Fetch::InOrg(id) => {
                let answer = service.people_page_in_org(&id).await;
                let scope = self.link.scope();
                let mut st = self.lock();
                if st.search.generation != generation {
                    return;
                }
                match answer {
                    Ok(page) => apply(&mut st, page, format!("org:{id}")),
                    Err(e) => st.results = Loadable::Failed(describe(&e, &host)),
                }
                self.sync_detail(&mut st, true, scope);
            }
            Fetch::Text(q) => {
                tokio::time::sleep(Duration::from_millis(300)).await;
                {
                    let mut st = self.lock();
                    if st.search.generation != generation {
                        return;
                    }
                    st.search.previous = None;
                }
                let answer = service.people_page_matching(&q).await;
                let scope = self.link.scope();
                let mut st = self.lock();
                if st.search.generation != generation {
                    return;
                }
                match answer {
                    Ok(page) => {
                        if trim_whitespaces(&st.query) != q {
                            return;
                        }
                        apply(&mut st, page, q);
                    }
                    Err(e) => st.results = Loadable::Failed(describe(&e, &host)),
                }
                self.sync_detail(&mut st, true, scope);
            }
        }
        self.publish();
    }

    // MARK: Detail (`loadSelected(keepTab:)`, W/OrgsModel.swift:168-178)

    /// `PersonDetailView.task(id: person.id)`: a newly shown person loads, on their default tab.
    fn sync_detail(&self, st: &mut State, active: bool, scope: TicketScope) {
        if !active {
            return;
        }
        let shown = st.selected().map(|p| p.id.clone());
        if shown != st.shown_id {
            st.shown_id = shown.clone();
            if shown.is_some() {
                self.start_detail(st, false, scope);
            }
        }
    }

    fn start_detail(&self, st: &mut State, keep_tab: bool, scope: TicketScope) {
        let Some(person) = st.selected().cloned() else {
            return;
        };
        st.detail_generation += 1;
        if let Some(h) = st.detail_task.take() {
            h.abort();
        }
        if !keep_tab {
            st.tab = if person.is_agent() {
                PeopleTab::Assigned
            } else {
                PeopleTab::Reported
            };
        }
        st.assigned = Loadable::Loading;
        st.reported = Loadable::Loading;
        st.ticket_scope = scope;
        st.table.page = 1;
        let generation = st.detail_generation;
        let me = self.me.clone();
        let handle = tokio::spawn(async move {
            let Some(model) = me.upgrade() else { return };
            model.publish();
            let host = text::host(&model.ctx.session);
            let service = OrgService::new(model.ctx.session.client.clone());
            let other = service.clone();
            let id = person.id.clone();
            let reported =
                tokio::spawn(async move { other.tickets_reported_by(&id, &scope).await });
            let assigned = service.tickets_assigned_to(&person.id, &scope).await;
            {
                let mut st = model.lock();
                if st.detail_generation != generation {
                    reported.abort();
                    return;
                }
                st.assigned = match assigned {
                    Ok(t) => Loadable::Loaded(t),
                    Err(e) => Loadable::Failed(describe(&e, &host)),
                };
            }
            model.publish();
            let reported = match reported.await {
                Ok(r) => r,
                Err(_) => return,
            };
            {
                let mut st = model.lock();
                if st.detail_generation != generation {
                    return;
                }
                st.reported = match reported {
                    Ok(t) => Loadable::Loaded(t),
                    Err(e) => Loadable::Failed(describe(&e, &host)),
                };
            }
            model.publish();
        });
        st.detail_task = Some(handle.abort_handle());
    }

    // MARK: Snapshot

    fn build(&self, st: &State) -> PeopleSnapshot {
        PeopleSnapshot {
            query: st.query.clone(),
            scope: st.scope.as_ref().map(|(id, name)| OrgScope {
                org_id: id.clone(),
                org_name: name.clone(),
            }),
            results: Self::results_view(st),
            selected_id: st.selected_id.clone(),
            detail: st.selected().map(|p| self.detail_view(st, p)),
            placeholder: "Select a person to see their details and tickets.".to_owned(),
        }
    }

    fn results_view(st: &State) -> PeopleResults {
        match &st.results {
            Loadable::Idle => PeopleResults::Idle {
                text: "Search by name, for example a caller or an agent.".to_owned(),
            },
            Loadable::Loading => PeopleResults::Searching,
            Loadable::Failed(e) => PeopleResults::Failed {
                text: format!("Couldn't search iTop. {e}"),
            },
            Loadable::Loaded(people) if people.is_empty() => PeopleResults::Empty {
                text: if st.scope.is_some() {
                    "Nobody in this organization.".to_owned()
                } else {
                    format!(
                        "Nobody matches \u{201C}{}\u{201D}.",
                        trim_whitespaces(&st.query)
                    )
                },
            },
            Loadable::Loaded(people) => {
                let shown = people.len() as i64;
                let banner = (st.total > shown).then(|| {
                    if st.scope.is_some() {
                        format!("Showing {shown} of {} people. Search by name to find someone.", st.total)
                    } else {
                        format!("Showing {shown} of {} people. Type more of the name to narrow it down.", st.total)
                    }
                });
                PeopleResults::Loaded {
                    banner,
                    people: people
                        .iter()
                        .map(|p| PersonRow {
                            id: p.id.clone(),
                            display_name: p.display_name.clone(),
                            initials: p.initials(),
                            subtitle: p.subtitle(),
                        })
                        .collect(),
                }
            }
        }
    }

    fn detail_view(&self, st: &State, p: &Person) -> PersonDetailView {
        let count = |l: &Loadable<Vec<Ticket>>| {
            l.value()
                .map_or_else(String::new, |t| format!(" {}", t.len()))
        };
        let open = st.ticket_scope == TicketScope::OPEN;
        let kind = if open {
            "open tickets"
        } else {
            "tickets in this range"
        };
        let tickets = match st.current_tickets() {
            Loadable::Idle | Loadable::Loading => TableState::Loading {
                text: "Loading tickets".to_owned(),
            },
            Loadable::Failed(e) => TableState::Failed {
                text: format!("Couldn't load the tickets. {e}"),
                can_retry: false,
            },
            Loadable::Loaded(t) if t.is_empty() => TableState::Empty {
                text: match st.tab {
                    PeopleTab::Assigned => format!("No {kind} assigned to {}.", p.display_name),
                    PeopleTab::Reported => format!("No {kind} reported by {}.", p.display_name),
                },
            },
            Loadable::Loaded(t) => {
                let last = if st.tab == PeopleTab::Assigned {
                    LastColumn::Caller
                } else {
                    LastColumn::Assignee
                };
                TableState::Loaded {
                    table: table::table(t, &self.ctx.session.user, last, &st.table),
                }
            }
        };
        PersonDetailView {
            id: p.id.clone(),
            display_name: p.display_name.clone(),
            initials: p.initials(),
            kind_line: [
                if p.is_agent() { "Agent" } else { "Caller" },
                p.function.as_str(),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
            organization: (!p.org_name.is_empty()).then(|| OrgScope {
                org_id: p.org_id.clone(),
                org_name: p.org_name.clone(),
            }),
            email: (!p.email.is_empty()).then(|| p.email.clone()),
            phone: (!p.phone.is_empty()).then(|| p.phone.clone()),
            teams: p.is_agent().then(|| {
                p.teams
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            }),
            tab: st.tab,
            assigned_label: format!("Assigned to them{}", count(&st.assigned)),
            reported_label: format!("Reported by them{}", count(&st.reported)),
            tickets,
        }
    }
}

#[async_trait::async_trait]
impl Model for PeopleModel {
    async fn dispatch(&self, model_id: &str, intent: Value) -> Result<Value, AppError> {
        if model_id != MODEL_ID {
            return Err(AppError::unknown_model(model_id));
        }
        let intent: PeopleIntent =
            serde_json::from_value(intent).map_err(AppError::invalid_intent)?;
        match intent {
            PeopleIntent::SetQuery { query } => self.set_query(query),
            PeopleIntent::ClearScope => self.clear_scope(),
            PeopleIntent::Select { id } => self.select(id),
            PeopleIntent::SetTab { tab } => self.set_tab(tab),
            PeopleIntent::SetTicketPage { page } => self.set_ticket_page(page),
        }
        Ok(Value::Null)
    }
}
