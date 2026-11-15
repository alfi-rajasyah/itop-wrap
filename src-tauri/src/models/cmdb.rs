//! `cmdb`: the Config items section. Port of `CMDBModel` (W/CMDBModel.swift) plus the rules of
//! `CMDBListView` and `CIDetailView` (W/CMDBView.swift). contract.md §5.11.
//!
//! SwiftUI runs the search as `.task(id: query + scope id)` and the detail load as
//! `.task(id: item.id)`, both started when the view appears and cancelled when it goes. Here the
//! shell reports that with [`CmdbModel::appear`] / [`CmdbModel::disappear`]; an intent always acts
//! as if the section is on screen (React only sends them from it).

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use itop_kit::{CMDBQueries, CMDBService, ConfigItem, ConfigItemDetail, TicketList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::seams::{Model, ModelContext, WorkspaceLink};
use crate::error::AppError;

#[cfg(test)]
#[path = "cmdb_test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "cmdb_tests.rs"]
mod tests;
#[path = "cmdb_text.rs"]
pub mod text;

use text::{OrgScope, SearchRun, character_count, describe, status_label, trim_whitespaces};

pub const MODEL_ID: &str = "cmdb";

// MARK: - Contract shapes

/// A search result row (CMDB-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CIRow {
    /// "<Class>::<key>".
    pub id: String,
    pub key: String,
    pub class_name: String,
    pub name: String,
    /// "<Class label> · <Org>".
    pub subtitle: String,
}

/// A relationship chip (CMDB-14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CIChip {
    pub key: String,
    pub class_name: String,
    pub name: String,
    /// "Network device: open vpn-gw-01".
    pub help: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum CIResults {
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
    /// `header`: "1 result", "3 results", "First 50 results".
    Loaded {
        header: String,
        items: Vec<CIRow>,
    },
}

/// One cell of the CI fields grid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CIDetailField {
    pub label: String,
    pub value: String,
}

/// A ticket as an open-ticket row under a config item (CMDB-15).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CITicketRow {
    pub ticket_id: String,
    pub r#ref: String,
    pub title: String,
    pub needs_you: bool,
    /// "Assigned · You", "New · Unassigned".
    pub status_line: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum CIDetailBody {
    Loading {
        text: String,
    },
    Failed {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Loaded {
        fields: Vec<CIDetailField>,
        description: Option<String>,
        depends_on: Vec<CIChip>,
        used_by: Vec<CIChip>,
        /// "Nothing in the CMDB depends on X." / "If X fails, A and B are affected."
        used_by_sentence: String,
        open_tickets: Vec<CITicketRow>,
        /// "No open tickets on X."
        open_tickets_empty_text: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CIDetailView {
    pub id: String,
    pub key: String,
    pub class_name: String,
    pub name: String,
    pub org_name: String,
    /// "<Class label> · <Status>" (status once loaded).
    pub header_line: String,
    pub copied: bool,
    pub web_url: String,
    pub body: CIDetailBody,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct CMDBSnapshot {
    pub query: String,
    /// "Config items in <org>" bar.
    pub scope: Option<OrgScope>,
    pub results: CIResults,
    pub selected_id: Option<String>,
    pub current: Option<CIDetailView>,
    pub placeholder: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum CMDBIntent {
    SetQuery {
        query: String,
    },
    ClearScope,
    Select {
        id: Option<String>,
    },
    /// A relationship chip.
    #[serde(rename_all = "camelCase")]
    OpenCI {
        key: String,
        class_name: String,
        name: String,
    },
    RetryDetail,
    CopyName,
}

// MARK: - State

/// `CMDBModel.Results`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Results {
    Idle,
    Searching,
    Loaded(Vec<ConfigItem>),
    Failed(String),
}

/// `CMDBModel.Detail`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Detail {
    Loading,
    Loaded(Box<ConfigItemDetail>),
    Failed(String),
}

struct State {
    query: String,
    results: Results,
    selected_id: Option<String>,
    /// The CI shown on the right; it can be one that isn't in the results.
    current: Option<ConfigItem>,
    detail: Detail,
    copied: bool,
    /// The query the current results belong to, so coming back doesn't search again.
    searched: Option<String>,
    /// Set by the org view's Config items link; typing a search clears it.
    scope: Option<(String, String)>,
    /// The Config items section is on screen.
    visible: bool,
    search: SearchRun<Results>,
    /// The id of the CI whose detail view is alive (`CIDetailView.task(id:)`), and its load.
    shown_id: Option<String>,
    detail_generation: u64,
    detail_task: Option<tokio::task::AbortHandle>,
}

/// What the synchronous start of `search()` decided to fetch.
enum Fetch {
    InOrg(String),
    Text(String),
}

pub struct CmdbModel {
    ctx: ModelContext,
    #[allow(dead_code)]
    link: Arc<dyn WorkspaceLink>,
    me: Weak<CmdbModel>,
    state: Mutex<State>,
}

impl CmdbModel {
    pub fn new(ctx: ModelContext, link: Arc<dyn WorkspaceLink>) -> Arc<Self> {
        let model = Arc::new_cyclic(|me| CmdbModel {
            ctx,
            link,
            me: me.clone(),
            state: Mutex::new(State {
                query: String::new(),
                results: Results::Idle,
                selected_id: None,
                current: None,
                detail: Detail::Loading,
                copied: false,
                searched: None,
                scope: None,
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

    /// Builds and publishes under the state lock, so snapshots leave in the order of the changes.
    fn publish(&self) {
        let st = self.lock();
        self.ctx.hub.publish(MODEL_ID, &self.build(&st));
    }

    /// The current snapshot (also what the hub holds).
    pub fn snapshot(&self) -> CMDBSnapshot {
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

    // MARK: Cross-model calls (ShellView)

    /// Config items became the shown section: CMDBListView's `.task` searches and CIDetailView's
    /// `.task(id:)` loads the current CI.
    pub fn appear(&self) {
        let mut st = self.lock();
        st.visible = true;
        self.start_search(&mut st);
        st.shown_id = None;
        self.sync_detail(&mut st, true);
        drop(st);
        self.publish();
    }

    /// Another section was chosen: SwiftUI cancels the views' tasks.
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

    /// `showOrg(id:name:)`: the org view's "Config items" link (SHELL-12, CMDB-7). The shell then
    /// selects Config items.
    pub fn show_org(&self, id: &str, name: &str) {
        let mut st = self.lock();
        st.scope = Some((id.to_owned(), name.to_owned()));
        st.query.clear();
        st.searched = None;
        let active = st.visible;
        self.task_id_changed(&mut st, active);
        drop(st);
        self.publish();
    }

    /// `open(_:)`: show a CI from a relationship chip or a ticket's linked CI (SHELL-11, CMDB-8).
    pub fn open(&self, item: ConfigItem) {
        let active = self.lock().visible;
        self.open_item(item, active);
    }

    // MARK: Intents

    fn set_query(&self, query: String) {
        let mut st = self.lock();
        st.query = query;
        self.task_id_changed(&mut st, true);
        drop(st);
        self.publish();
    }

    /// `clearScope()`.
    fn clear_scope(&self) {
        let mut st = self.lock();
        st.scope = None;
        st.results = Results::Idle;
        st.searched = None;
        self.task_id_changed(&mut st, true);
        drop(st);
        self.publish();
    }

    /// `selectedID` and its `didSet`.
    fn select(&self, id: Option<String>) {
        let mut st = self.lock();
        st.selected_id = id;
        let current_id = st.current.as_ref().map(ConfigItem::id);
        if st.selected_id != current_id
            && let Results::Loaded(items) = &st.results
            && let Some(item) = items
                .iter()
                .find(|i| Some(i.id()) == st.selected_id)
                .cloned()
        {
            st.current = Some(item);
        }
        self.sync_detail(&mut st, true);
        drop(st);
        self.publish();
    }

    fn open_item(&self, item: ConfigItem, active: bool) {
        let mut st = self.lock();
        // `selectedID = item.id`: its didSet stops at `selectedID != current?.id`.
        st.selected_id = Some(item.id());
        st.current = Some(item);
        self.sync_detail(&mut st, active);
        drop(st);
        self.publish();
    }

    /// `copyName(_:)`: the name on the pasteboard, "Copied" for 2 s (CMDB-11, POLL-9).
    fn copy_name(&self) {
        let mut st = self.lock();
        let Some(name) = st.current.as_ref().map(|c| c.name.clone()) else {
            return;
        };
        self.ctx.native.copy_text(&name);
        st.copied = true;
        drop(st);
        self.publish();
        let me = self.me.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if let Some(model) = me.upgrade() {
                model.lock().copied = false;
                model.publish();
            }
        });
    }

    // MARK: Search (`search()`, W/CMDBModel.swift:75-98)

    fn task_id(st: &State) -> String {
        format!(
            "{}{}",
            st.query,
            st.scope.as_ref().map_or("", |s| s.0.as_str())
        )
    }

    /// `.task(id: query + scope id)`: a new id restarts the search while the list is on screen.
    fn task_id_changed(&self, st: &mut State, active: bool) {
        if active && st.search.task_id.as_deref() != Some(Self::task_id(st).as_str()) {
            self.start_search(st);
        }
    }

    fn start_search(&self, st: &mut State) {
        let State {
            search, results, ..
        } = &mut *st;
        search.cancel(results);
        let fetch = Self::search_start(st);
        st.search.task_id = Some(Self::task_id(st));
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

    /// Everything `search()` does before its first `await`.
    fn search_start(st: &mut State) -> Option<Fetch> {
        let q = trim_whitespaces(&st.query).to_owned();
        if !q.is_empty() && st.scope.is_some() {
            st.scope = None;
        }
        if let Some((id, _)) = &st.scope {
            let key = format!("org:{id}");
            if st.searched.as_deref() == Some(key.as_str())
                && matches!(st.results, Results::Loaded(_))
            {
                return None;
            }
            st.results = Results::Searching;
            return Some(Fetch::InOrg(id.clone()));
        }
        if character_count(&q) < 2 {
            st.results = Results::Idle;
            st.searched = None;
            return None;
        }
        if st.searched.as_deref() == Some(q.as_str()) && matches!(st.results, Results::Loaded(_)) {
            return None;
        }
        st.search.previous = Some(std::mem::replace(&mut st.results, Results::Searching));
        Some(Fetch::Text(q))
    }

    async fn search_finish(self: Arc<Self>, generation: u64, fetch: Fetch) {
        self.publish();
        let service = CMDBService::new(self.ctx.session.client.clone());
        let host = text::host(&self.ctx.session);
        match fetch {
            Fetch::InOrg(id) => {
                let answer = service.items_in_org(&id).await;
                let mut st = self.lock();
                if st.search.generation != generation {
                    return;
                }
                match answer {
                    Ok(items) => {
                        st.results = Results::Loaded(items);
                        st.searched = Some(format!("org:{id}"));
                    }
                    Err(e) => st.results = Results::Failed(describe(&e, &host)),
                }
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
                let answer = service.search(&q).await;
                let mut st = self.lock();
                if st.search.generation != generation {
                    return;
                }
                match answer {
                    Ok(items) => {
                        if trim_whitespaces(&st.query) != q {
                            return;
                        }
                        st.results = Results::Loaded(items);
                        st.searched = Some(q);
                    }
                    Err(e) => st.results = Results::Failed(describe(&e, &host)),
                }
            }
        }
        self.publish();
    }

    // MARK: Detail (`loadDetail()`, W/CMDBModel.swift:106-116)

    /// `CIDetailView.task(id: item.id)`: a new current CI (or the view appearing) loads it.
    fn sync_detail(&self, st: &mut State, active: bool) {
        if !active {
            return;
        }
        let shown = st.current.as_ref().map(ConfigItem::id);
        if shown != st.shown_id {
            st.shown_id = shown.clone();
            if shown.is_some() {
                self.start_detail(st);
            }
        }
    }

    fn start_detail(&self, st: &mut State) -> Option<tokio::task::JoinHandle<()>> {
        let item = st.current.clone()?;
        st.detail_generation += 1;
        if let Some(h) = st.detail_task.take() {
            h.abort();
        }
        st.detail = Detail::Loading;
        let generation = st.detail_generation;
        let me = self.me.clone();
        let handle = tokio::spawn(async move {
            let Some(model) = me.upgrade() else { return };
            model.publish();
            let host = text::host(&model.ctx.session);
            let answer = CMDBService::new(model.ctx.session.client.clone())
                .detail(&item)
                .await;
            {
                let mut st = model.lock();
                if st.detail_generation != generation {
                    return;
                }
                match answer {
                    Ok(d) => {
                        if st.current.as_ref().map(ConfigItem::id) != Some(item.id()) {
                            return;
                        }
                        st.detail = Detail::Loaded(Box::new(d));
                    }
                    Err(e) => st.detail = Detail::Failed(describe(&e, &host)),
                }
            }
            model.publish();
        });
        st.detail_task = Some(handle.abort_handle());
        Some(handle)
    }

    /// "Retry" under a failed detail.
    async fn retry_detail(&self) {
        let handle = {
            let mut st = self.lock();
            self.start_detail(&mut st)
        };
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    // MARK: Snapshot

    fn build(&self, st: &State) -> CMDBSnapshot {
        CMDBSnapshot {
            query: st.query.clone(),
            scope: st.scope.as_ref().map(|(id, name)| OrgScope {
                org_id: id.clone(),
                org_name: name.clone(),
            }),
            results: Self::results_view(st),
            selected_id: st.selected_id.clone(),
            current: st.current.as_ref().map(|item| self.detail_view(st, item)),
            placeholder: "Select a config item to see its details.".to_owned(),
        }
    }

    fn results_view(st: &State) -> CIResults {
        match &st.results {
            Results::Idle => CIResults::Idle {
                text: "Search by name, for example a server or application.".to_owned(),
            },
            Results::Searching => CIResults::Searching,
            Results::Failed(e) => CIResults::Failed {
                text: format!("Couldn't search iTop. {e}"),
            },
            Results::Loaded(items) if items.is_empty() => CIResults::Empty {
                text: format!(
                    "No config items match \u{201C}{}\u{201D}.",
                    trim_whitespaces(&st.query)
                ),
            },
            Results::Loaded(items) => {
                let n = items.len();
                let header = if n == 1 {
                    "1 result".to_owned()
                } else if n as i64 >= CMDBQueries::SEARCH_LIMIT {
                    format!("First {n} results")
                } else {
                    format!("{n} results")
                };
                CIResults::Loaded {
                    header,
                    items: items
                        .iter()
                        .map(|item| CIRow {
                            id: item.id(),
                            key: item.key.clone(),
                            class_name: item.class_name.clone(),
                            name: item.name.clone(),
                            subtitle: [item.class_label(), item.org_name.clone()]
                                .into_iter()
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join(" · "),
                        })
                        .collect(),
                }
            }
        }
    }

    fn detail_view(&self, st: &State, item: &ConfigItem) -> CIDetailView {
        let session = &self.ctx.session;
        let status = match &st.detail {
            Detail::Loaded(d) => d
                .fields
                .iter()
                .find(|f| f.label == "Status")
                .map(|f| f.value.clone()),
            _ => None,
        };
        let header_line = std::iter::once(item.class_label())
            .chain(status)
            .collect::<Vec<_>>()
            .join(" · ");
        let body = match &st.detail {
            Detail::Loading => CIDetailBody::Loading {
                text: format!("Loading {} from iTop", item.name),
            },
            Detail::Failed(e) => CIDetailBody::Failed {
                text: format!("Couldn't load this config item. {e}"),
            },
            Detail::Loaded(d) => {
                let chip = |r: &itop_kit::CIRef| CIChip {
                    key: r.id.clone(),
                    class_name: r.class_name.clone(),
                    name: r.name.clone(),
                    help: format!(
                        "{}: open {}",
                        ConfigItem::label_for_class(&r.class_name),
                        r.name
                    ),
                };
                let names: Vec<String> = d.used_by.iter().map(|r| r.name.clone()).collect();
                let used_by_sentence = if d.used_by.is_empty() {
                    format!("Nothing in the CMDB depends on {}.", item.name)
                } else {
                    format!(
                        "If {} fails, {} {} affected.",
                        item.name,
                        text::join_list(&names),
                        if d.used_by.len() == 1 { "is" } else { "are" }
                    )
                };
                let me = &session.user;
                CIDetailBody::Loaded {
                    fields: d
                        .fields
                        .iter()
                        .map(|f| CIDetailField {
                            label: f.label.clone(),
                            value: f.value.clone(),
                        })
                        .collect(),
                    description: (!d.description.is_empty()).then(|| d.description.clone()),
                    depends_on: d.depends_on.iter().map(chip).collect(),
                    used_by: d.used_by.iter().map(chip).collect(),
                    used_by_sentence,
                    open_tickets: d
                        .open_tickets
                        .iter()
                        .map(|t| CITicketRow {
                            ticket_id: t.id(),
                            r#ref: t.r#ref.clone(),
                            title: t.title.clone(),
                            needs_you: TicketList::needs_you(t, me),
                            status_line: format!(
                                "{} · {}",
                                status_label(&t.status),
                                if t.is_unassigned() {
                                    "Unassigned".to_owned()
                                } else if t.agent_id == me.contact_id {
                                    "You".to_owned()
                                } else {
                                    t.agent_name.clone()
                                }
                            ),
                        })
                        .collect(),
                    open_tickets_empty_text: d
                        .open_tickets
                        .is_empty()
                        .then(|| format!("No open tickets on {}.", item.name)),
                }
            }
        };
        CIDetailView {
            id: item.id(),
            key: item.key.clone(),
            class_name: item.class_name.clone(),
            name: item.name.clone(),
            org_name: item.org_name.clone(),
            header_line,
            copied: st.copied,
            web_url: format!(
                "{}/pages/UI.php?operation=details&class={}&id={}",
                session.client.server.base_url(),
                item.class_name,
                item.key
            ),
            body,
        }
    }
}

#[async_trait::async_trait]
impl Model for CmdbModel {
    async fn dispatch(&self, model_id: &str, intent: Value) -> Result<Value, AppError> {
        if model_id != MODEL_ID {
            return Err(AppError::unknown_model(model_id));
        }
        let intent: CMDBIntent =
            serde_json::from_value(intent).map_err(AppError::invalid_intent)?;
        match intent {
            CMDBIntent::SetQuery { query } => self.set_query(query),
            CMDBIntent::ClearScope => self.clear_scope(),
            CMDBIntent::Select { id } => self.select(id),
            CMDBIntent::OpenCI {
                key,
                class_name,
                name,
            } => self.open_item(ConfigItem::new(key, class_name, name, ""), true),
            CMDBIntent::RetryDetail => self.retry_detail().await,
            CMDBIntent::CopyName => self.copy_name(),
        }
        Ok(Value::Null)
    }
}
