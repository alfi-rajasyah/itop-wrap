//! Port of `TicketListModel.swift` + what `TicketListView` derives (LIST-*, ASG-1…5, ASG-13,
//! LC-1…6, LC-22…23): loading, refresh, search, the full-ref lookup, selection, assign and the
//! lifecycle changes.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use itop_kit::{
    Age, CurrentUser, LogImage, Team, Ticket as KitTicket, TicketChange, TicketList, TicketPage,
    TicketQueries, TicketSearch, TicketSection as KitSection, TicketWrites, TimeFormat, WriteError,
    ticket_writes::Failure,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::options::OptionsState;
use super::records::{
    ListScope, Notice, Pager, PriorityMark, RowAgent, RowTime, TicketRow, TicketSection, pager,
    status_label, ticket_view, with_period,
};
use super::workspace::{BoxFuture, State, Workspace};
use crate::error::AppError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Loaded,
    Failed(String),
}

/// One per list, so the size of `Found` doesn't matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    None,
    Searching(String),
    Found(KitTicket),
    Missing(String),
    Failed(String),
}

/// Foundation's `CharacterSet.whitespaces`: tab and the space separators, not newlines.
pub fn trim_spaces(s: &str) -> &str {
    s.trim_matches(|c: char| {
        c == '\t'
            || (c.is_whitespace()
                && !matches!(
                    c,
                    '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                ))
    })
}

pub struct ListState {
    pub scope: ListScope,
    pub phase: Phase,
    pub tickets: Vec<KitTicket>,
    pub updated_at: Option<DateTime<Utc>>,
    pub refreshing: bool,
    pub refresh_again: bool,
    pub refresh_error: Option<String>,
    lookup: Lookup,
    /// A ticket just resolved or closed while open: kept on screen (LIST-30).
    pub just_changed: Option<String>,
    pub query: String,
    pub selection: Option<String>,
    pub org_filter: Option<String>,
    pub drafts: HashMap<String, String>,
    pub draft_images: HashMap<String, Vec<LogImage>>,
    pub page: i64,
    pub working: Option<String>,
    pub notice: Option<Notice>,
}

impl ListState {
    pub fn new(scope: ListScope) -> Self {
        ListState {
            scope,
            phase: Phase::Loading,
            tickets: Vec::new(),
            updated_at: None,
            refreshing: false,
            refresh_again: false,
            refresh_error: None,
            lookup: Lookup::None,
            just_changed: None,
            query: String::new(),
            selection: None,
            org_filter: None,
            drafts: HashMap::new(),
            draft_images: HashMap::new(),
            page: 1,
            working: None,
            notice: None,
        }
    }

    pub fn lookup(&self) -> &Lookup {
        &self.lookup
    }

    /// The `lookup` setter with its `didSet`: anything but the just-changed ticket as the
    /// found one forgets it.
    pub fn set_lookup(&mut self, lookup: Lookup) {
        self.lookup = lookup;
        let keep = matches!(&self.lookup, Lookup::Found(t) if Some(t.id()) == self.just_changed);
        if !keep {
            self.just_changed = None;
        }
    }

    /// `ticket(_:)`: the found ticket first, then the list.
    pub fn ticket(&self, id: Option<&str>) -> Option<&KitTicket> {
        let id = id?;
        if let Lookup::Found(t) = &self.lookup
            && t.id() == id
        {
            return Some(t);
        }
        self.tickets.iter().find(|t| t.id() == id)
    }

    pub fn need_count(&self, me: &CurrentUser) -> i64 {
        self.tickets
            .iter()
            .filter(|t| TicketList::needs_you(t, me))
            .count() as i64
    }

    pub fn is_searching(&self) -> bool {
        !trim_spaces(&self.query).is_empty()
    }

    pub fn has_no_teams(&self, me: &CurrentUser) -> bool {
        self.scope == ListScope::Team && me.teams.is_empty()
    }

    /// Org filter, then search, then grouping (LIST-17).
    pub fn visible_sections(&self, me: &CurrentUser, options: &OptionsState) -> Vec<KitSection> {
        let orgs: HashSet<String> = self.org_filter.iter().cloned().collect();
        let filtered = TicketSearch::filter_query(
            &TicketSearch::filter_orgs(&self.tickets, &orgs),
            &self.query,
        );
        TicketList::groups(&filtered, me, self.scope.view(), options.sort)
    }

    pub fn current_page(&self, me: &CurrentUser, options: &OptionsState) -> TicketPage {
        TicketPage::new(
            self.visible_sections(me, options),
            options.page_size,
            self.page,
        )
    }

    pub fn org_choices(&self) -> Vec<String> {
        TicketSearch::orgs(&self.tickets)
    }

    /// `orgID(named:)`: the org id behind a name, for "See all of its tickets".
    pub fn org_id_named(&self, name: &str) -> Option<String> {
        self.tickets
            .iter()
            .find(|t| t.org_name == name && t.org_id != "0")
            .map(|t| t.org_id.clone())
    }

    /// The composer key for a ticket and log: "<ticketID>|public" / "|private".
    pub fn draft_key(ticket_id: &str, private: bool) -> String {
        format!("{ticket_id}|{}", if private { "private" } else { "public" })
    }
}

// MARK: Snapshot

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgChoice {
    pub name: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgFilter {
    pub visible: bool,
    pub selected: Option<String>,
    pub label: String,
    pub help: String,
    pub accessibility_label: String,
    pub choices: Vec<OrgChoice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum LookupBar {
    Searching { text: String },
    Missing { text: String },
    Failed { text: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgLink {
    pub org_id: String,
    pub org_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgNoMatch {
    pub text: String,
    pub see_all: Option<OrgLink>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ExtraSection {
    pub title: String,
    pub row: TicketRow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ListBody {
    pub org_no_match: Option<OrgNoMatch>,
    pub refresh_error: Option<String>,
    pub lookup_bar: Option<LookupBar>,
    pub show_notices: bool,
    pub notice: Option<Notice>,
    pub extra_section: Option<ExtraSection>,
    pub no_match_text: Option<String>,
    pub sections: Vec<TicketSection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ListContent {
    Loading,
    Failed { message: String },
    Message { text: String },
    List { body: Box<ListBody> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ListSnapshot {
    pub scope: ListScope,
    pub query: String,
    pub selection: Option<String>,
    pub content: ListContent,
    pub show_controls: bool,
    pub org_filter: OrgFilter,
    pub pager: Pager,
    pub refreshing: bool,
    pub footer_spinner: bool,
    pub updated_text: String,
    pub need_count: i64,
    pub working: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ListIntent {
    SetQuery {
        query: String,
    },
    Select {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: Option<String>,
    },
    SetPage {
        page: i64,
    },
    ToggleOrg {
        name: String,
    },
    ClearOrgFilter,
    Refresh,
    ClearNotice,
    RowAssignToMe {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
}

/// `TicketListView.updatedText(now:)` (LIST-68).
pub fn updated_text(l: &ListState, now: DateTime<Utc>) -> String {
    match l.updated_at {
        None => {
            if l.phase == Phase::Loading {
                "Loading".to_owned()
            } else {
                "Not updated yet".to_owned()
            }
        }
        Some(at) => {
            let minutes = (now - at).num_seconds() / 60;
            if minutes < 1 {
                "Updated just now".to_owned()
            } else {
                format!("Updated {minutes} min ago")
            }
        }
    }
}

impl Workspace {
    /// The row's "Assign to me" pill is offered (`assignAction`, LIST-57).
    pub(crate) fn can_row_assign(&self, t: &KitTicket) -> bool {
        t.is_unassigned() && TicketWrites::assign_stimulus(&t.status).is_some() && self.can_write(t)
    }

    /// One `TicketRow`. `with_time` is false for the lookup's row (no total, no timer).
    pub(crate) fn ticket_row(
        &self,
        st: &State,
        t: &KitTicket,
        show_agent: bool,
        with_time: bool,
        now: DateTime<Utc>,
    ) -> TicketRow {
        let me = self.me();
        let id = t.id();
        let agent = if !show_agent {
            RowAgent::Hidden
        } else if t.is_unassigned() && self.can_row_assign(t) {
            RowAgent::AssignToMe
        } else if t.is_unassigned() {
            RowAgent::Unassigned
        } else if t.agent_id == me.contact_id {
            RowAgent::You
        } else {
            RowAgent::Name {
                name: t.agent_name.clone(),
            }
        };
        let time = if !with_time {
            RowTime::None
        } else if st
            .timer
            .running
            .as_ref()
            .is_some_and(|r| r.ticket_id() == id)
        {
            RowTime::Running
        } else {
            match st.timer.totals.get(&id) {
                Some(&spent) if spent > 0 => RowTime::Spent {
                    text: TimeFormat::total(spent),
                },
                _ => RowTime::None,
            }
        };
        TicketRow {
            ticket: ticket_view(t, me),
            age: t.start_date.map(|s| Age::short(s, now)),
            opened_tooltip: t
                .start_date
                .map(|s| format!("Opened {}", self.0.fmt.date_time(s))),
            priority_mark: match t.priority {
                1 => Some(PriorityMark::P1),
                2 => Some(PriorityMark::P2),
                _ => None,
            },
            agent,
            time,
        }
    }

    pub(crate) fn list_snapshot(
        &self,
        st: &State,
        scope: ListScope,
        now: DateTime<Utc>,
    ) -> ListSnapshot {
        let me = self.me();
        let l = &st.lists[scope.index()];
        let options = &st.options;
        let page = l.current_page(me, options);
        let loaded = l.phase == Phase::Loaded;
        let content = match &l.phase {
            Phase::Loading => ListContent::Loading,
            Phase::Failed(message) => ListContent::Failed {
                message: message.clone(),
            },
            Phase::Loaded if l.has_no_teams(me) => ListContent::Message {
                text: "You're not in any iTop team, so there are no team tickets to show."
                    .to_owned(),
            },
            Phase::Loaded if l.tickets.is_empty() && !l.is_searching() => ListContent::Message {
                text: if options.scope() != itop_kit::TicketScope::OPEN {
                    "No tickets in this range. Try a longer range in the View menu.".to_owned()
                } else if scope == ListScope::Mine {
                    "Nothing waiting on you. New assignments show up here.".to_owned()
                } else {
                    "Your teams have no open tickets.".to_owned()
                },
            },
            Phase::Loaded => ListContent::List {
                body: Box::new(self.list_body(st, scope, &page, now)),
            },
        };
        let org_visible =
            scope == ListScope::Mine && (l.org_choices().len() > 1 || l.org_filter.is_some());
        ListSnapshot {
            scope,
            query: l.query.clone(),
            selection: l.selection.clone(),
            content,
            show_controls: loaded && !l.has_no_teams(me),
            org_filter: OrgFilter {
                visible: org_visible,
                selected: l.org_filter.clone(),
                label: l
                    .org_filter
                    .clone()
                    .unwrap_or_else(|| "All organizations".to_owned()),
                help: l.org_filter.as_ref().map_or_else(
                    || "Filter by organization".to_owned(),
                    |o| format!("Showing tickets for {o}"),
                ),
                accessibility_label: format!(
                    "Filter by organization: {}",
                    l.org_filter.as_deref().unwrap_or("all")
                ),
                choices: l
                    .org_choices()
                    .into_iter()
                    .map(|name| OrgChoice {
                        selected: l.org_filter.as_ref() == Some(&name),
                        name,
                    })
                    .collect(),
            },
            pager: if loaded { pager(&page) } else { Pager::None },
            refreshing: l.refreshing,
            footer_spinner: l.refreshing && loaded,
            updated_text: updated_text(l, now),
            need_count: l.need_count(me),
            working: l.working.clone(),
        }
    }

    fn list_body(
        &self,
        st: &State,
        scope: ListScope,
        page: &TicketPage,
        now: DateTime<Utc>,
    ) -> ListBody {
        let me = self.me();
        let l = &st.lists[scope.index()];
        let searching = l.is_searching();
        let org_no_match = match &l.org_filter {
            Some(name) if l.visible_sections(me, &st.options).is_empty() && !searching => {
                Some(OrgNoMatch {
                    text: format!("None of your tickets are for {name}."),
                    see_all: if scope == ListScope::Mine {
                        l.org_id_named(name).map(|org_id| OrgLink {
                            org_id,
                            org_name: name.clone(),
                        })
                    } else {
                        None
                    },
                })
            }
            _ => None,
        };
        let lookup_bar = match l.lookup() {
            Lookup::Searching(r) => Some(LookupBar::Searching {
                text: format!("Looking up {r} in iTop…"),
            }),
            Lookup::Missing(r) => Some(LookupBar::Missing {
                text: format!("{r} doesn't exist, or your iTop account can't see it."),
            }),
            Lookup::Failed(m) => Some(LookupBar::Failed {
                text: format!("Couldn't look that up: {m}"),
            }),
            Lookup::None | Lookup::Found(_) => None,
        };
        let extra_section = match l.lookup() {
            Lookup::Found(t) => Some(ExtraSection {
                title: if l.just_changed.as_deref() == Some(t.id().as_str()) {
                    format!("{} just now", status_label(&t.status))
                } else {
                    "Search result: not in your queue".to_owned()
                },
                row: self.ticket_row(st, t, true, false, now),
            }),
            _ => None,
        };
        let no_match_text = (searching && page.sections.is_empty() && *l.lookup() == Lookup::None)
            .then(|| format!("No tickets in your list match “{}”.", trim_spaces(&l.query)));
        ListBody {
            org_no_match,
            refresh_error: l
                .refresh_error
                .as_ref()
                .map(|e| format!("Couldn't refresh: {e}")),
            lookup_bar,
            show_notices: matches!(l.lookup(), Lookup::None | Lookup::Found(_)),
            notice: l.notice.clone(),
            extra_section,
            no_match_text,
            sections: page
                .sections
                .iter()
                .map(|s| TicketSection {
                    group: s.group.into(),
                    title: s.group.title().to_owned(),
                    total: page.group_total(s.group),
                    rows: s
                        .tickets
                        .iter()
                        .map(|t| {
                            self.ticket_row(
                                st,
                                t,
                                scope == ListScope::Team || t.is_unassigned(),
                                true,
                                now,
                            )
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    // MARK: Intents

    pub(crate) async fn dispatch_list(
        &self,
        scope: ListScope,
        intent: ListIntent,
    ) -> Result<serde_json::Value, AppError> {
        let i = scope.index();
        match intent {
            ListIntent::SetQuery { query } => self.set_query(scope, &query),
            ListIntent::Select { ticket_id } => {
                self.lock().lists[i].selection = ticket_id;
                self.publish();
            }
            ListIntent::SetPage { page } => {
                self.lock().lists[i].page = page;
                self.publish();
            }
            ListIntent::ToggleOrg { name } => {
                {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    let new = if l.org_filter.as_deref() == Some(name.as_str()) {
                        None
                    } else {
                        Some(name)
                    };
                    if new != l.org_filter {
                        l.org_filter = new;
                        l.page = 1;
                    }
                }
                self.publish();
            }
            ListIntent::ClearOrgFilter => {
                {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    if l.org_filter.take().is_some() {
                        l.page = 1;
                    }
                }
                self.publish();
            }
            ListIntent::Refresh => self.list_refresh(scope).await,
            ListIntent::ClearNotice => {
                self.lock().lists[i].notice = None;
                self.publish();
            }
            ListIntent::RowAssignToMe { ticket_id } => {
                let t = {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    let t = l.ticket(Some(&ticket_id)).cloned();
                    if t.is_some() {
                        l.selection = Some(ticket_id.clone());
                    }
                    t
                };
                let Some(t) = t else {
                    return Err(AppError::invalid_intent(format!(
                        "No ticket {ticket_id} in this list."
                    )));
                };
                self.publish();
                self.list_assign_to_me(scope, &t).await;
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// The `query` binding, with the view's `onChange` (page 1) and `.task(id: query)` (the
    /// lookup restarts, cancelling the last one).
    pub(crate) fn set_query(&self, scope: ListScope, query: &str) {
        let changed = {
            let mut st = self.lock();
            let l = &mut st.lists[scope.index()];
            if l.query == query {
                false
            } else {
                l.query = query.to_owned();
                l.page = 1;
                true
            }
        };
        if changed {
            self.restart_search(scope);
        }
        self.publish();
    }

    pub(crate) fn restart_search(&self, scope: ListScope) {
        let ws = self.clone();
        let handle = self.spawn(async move { ws.search_changed(scope).await });
        if let Some(old) = self.tasks().lookup[scope.index()].replace(handle) {
            old.abort();
        }
    }

    /// `refresh()`: the first load shows a spinner, later ones keep the list on screen. A reload
    /// asked for while one runs runs right after it (LIST-65).
    pub(crate) fn list_refresh(&self, scope: ListScope) -> BoxFuture {
        let ws = self.clone();
        Box::pin(async move { ws.refresh_now(scope).await })
    }

    async fn refresh_now(&self, scope: ListScope) {
        let i = scope.index();
        let options_scope = {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            if l.refreshing {
                l.refresh_again = true;
                return;
            }
            l.refreshing = true;
            st.options.scope()
        };
        self.trace(|| format!("{} list: reload started", scope.raw()));
        self.publish();
        let me = self.me().clone();
        let result = match scope {
            ListScope::Mine => self.0.tickets.load_mine(&me, &options_scope).await,
            ListScope::Team => self.0.tickets.load_team(&me, &options_scope).await,
        };
        match result {
            Ok(tickets) => {
                let now = self.now();
                {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    l.tickets = tickets.clone();
                    l.updated_at = Some(now);
                    l.phase = Phase::Loaded;
                    l.refresh_error = None;
                }
                self.trace(|| {
                    format!(
                        "{} list: reload finished, {} tickets",
                        scope.raw(),
                        tickets.len()
                    )
                });
                self.publish();
                self.refresh_lookup(scope).await;
                {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    if l.selection.is_some() && l.ticket(l.selection.as_deref()).is_none() {
                        l.selection = None;
                    }
                }
                self.publish();
                if scope == ListScope::Mine {
                    self.timer_my_tickets_changed(&tickets).await;
                }
                let ws = self.clone();
                self.spawn(async move { ws.timer_refresh_totals(&tickets).await });
            }
            Err(e) => {
                let message = self.describe(&e);
                let mut st = self.lock();
                let l = &mut st.lists[i];
                if l.phase == Phase::Loaded {
                    l.refresh_error = Some(message);
                } else {
                    l.phase = Phase::Failed(message);
                }
            }
        }
        let again = {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            l.refreshing = false;
            std::mem::take(&mut l.refresh_again)
        };
        self.publish();
        if again {
            tokio::spawn(self.list_refresh(scope));
        }
    }

    /// `refreshLookup()` (LIST-29, LIST-32): the list's copy wins once it has the ticket;
    /// otherwise the found ticket is read again.
    async fn refresh_lookup(&self, scope: ListScope) {
        let i = scope.index();
        let found = {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            let Lookup::Found(found) = l.lookup() else {
                return;
            };
            let found = found.clone();
            let id = found.id();
            if l.just_changed.as_deref() == Some(id.as_str())
                && l.selection.as_deref() != Some(id.as_str())
            {
                l.set_lookup(Lookup::None);
                return;
            }
            if l.tickets.iter().any(|t| t.id() == id) {
                l.set_lookup(Lookup::None);
                return;
            }
            found
        };
        if let Ok(Some(fresh)) = self.0.tickets.lookup(&found.r#ref).await {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            if matches!(l.lookup(), Lookup::Found(still) if still.id() == fresh.id()) {
                l.set_lookup(Lookup::Found(fresh));
            }
        }
    }

    /// `searchChanged()` (LIST-20…28): a full ref that isn't in the list is asked of iTop after
    /// 250 ms; anything else clears the lookup.
    async fn search_changed(&self, scope: ListScope) {
        let i = scope.index();
        let wanted = {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            let q = trim_spaces(&l.query).to_owned();
            let ok = TicketQueries::ref_lookup(&q).is_some()
                && !l
                    .tickets
                    .iter()
                    .any(|t| t.r#ref.to_lowercase() == q.to_lowercase());
            if !ok {
                l.set_lookup(Lookup::None);
                None
            } else {
                let r = q.to_uppercase();
                if matches!(l.lookup(), Lookup::Found(t) if t.r#ref == r) {
                    return;
                }
                l.set_lookup(Lookup::Searching(r.clone()));
                Some(r)
            }
        };
        self.publish();
        let Some(r) = wanted else { return };
        self.trace(|| format!("{} list: looking up a ref", scope.raw()));
        tokio::time::sleep(Duration::from_millis(250)).await;
        let result = self.0.tickets.lookup(&r).await;
        {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            match result {
                Ok(found) => {
                    if trim_spaces(&l.query).to_uppercase() != r {
                        return;
                    }
                    l.set_lookup(found.map_or(Lookup::Missing(r), Lookup::Found));
                }
                Err(e) => {
                    let message = self.describe(&e);
                    l.set_lookup(Lookup::Failed(message));
                }
            }
        }
        self.publish();
    }

    // MARK: Assign and lifecycle

    /// `assignToMe(_:)`: one call, then a refresh (ASG-1…4).
    pub(crate) async fn list_assign_to_me(&self, scope: ListScope, t: &KitTicket) {
        let i = scope.index();
        {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            if l.working.is_some() {
                return;
            }
            l.working = Some(t.id());
            l.notice = None;
        }
        self.trace(|| format!("{} list: assign to me on {}", scope.raw(), t.id()));
        self.publish();
        match self.0.writes.assign_to_me(t, self.me()).await {
            Ok(()) => {
                self.lock().lists[i].notice =
                    Some(Notice::ok(format!("{} is assigned to you.", t.r#ref)));
                self.publish();
                self.list_refresh(scope).await;
            }
            Err(e) => {
                let taken = matches!(e, WriteError::Failure(Failure::AlreadyTaken { .. }));
                self.lock().lists[i].notice = Some(Notice::error(self.action_error(
                    &e,
                    t,
                    &format!("assign {}", t.r#ref),
                )));
                self.publish();
                if taken {
                    self.list_refresh(scope).await;
                }
            }
        }
        self.lock().lists[i].working = None;
        self.publish();
    }

    /// `assignNew(_:)`: New ticket with "Assign to me" (NEW-20).
    pub(crate) async fn list_assign_new(&self, scope: ListScope, t: &KitTicket) {
        self.list_assign_to_me(scope, t).await;
        {
            let mut st = self.lock();
            let l = &mut st.lists[scope.index()];
            match &l.notice {
                Some(n) if !n.is_error => {
                    l.notice = Some(Notice::ok(format!(
                        "{} is created and assigned to you.",
                        t.r#ref
                    )))
                }
                Some(n) => {
                    let why = n.text.clone();
                    l.notice = Some(Notice::error(format!(
                        "{} is created, but not assigned to you. {why}",
                        t.r#ref
                    )));
                }
                None => {}
            }
        }
        self.publish();
    }

    /// `assign(_:team:agent:)`: the error text, or `None` when done (ASG-13).
    pub(crate) async fn list_assign(
        &self,
        scope: ListScope,
        t: &KitTicket,
        team: &Team,
        agent: &itop_kit::Agent,
    ) -> Option<String> {
        let i = scope.index();
        {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            if l.working.is_some() {
                return Some("Another assignment is still running.".to_owned());
            }
            l.working = Some(t.id());
            l.notice = None;
        }
        self.publish();
        let result = match self.0.writes.assign(t, &team.id, &agent.id).await {
            Ok(()) => {
                let who = if agent.id == self.me().contact_id {
                    "you".to_owned()
                } else {
                    agent.name.clone()
                };
                self.lock().lists[i].notice = Some(Notice::ok(format!(
                    "{} is assigned to {who} in {}.",
                    t.r#ref, team.name
                )));
                self.publish();
                self.list_refresh(scope).await;
                None
            }
            Err(e) => Some(self.action_error(&e, t, &format!("assign {}", t.r#ref))),
        };
        self.lock().lists[i].working = None;
        self.publish();
        result
    }

    /// `apply(_:to:)`: pending, resolve, reopen or close, then a refresh (LC-1…6, LIST-30).
    pub(crate) async fn list_apply(
        &self,
        scope: ListScope,
        change: &TicketChange,
        t: &KitTicket,
    ) -> Option<String> {
        let i = scope.index();
        {
            let mut st = self.lock();
            let l = &mut st.lists[i];
            if l.working.is_some() {
                return Some("Another change is still running.".to_owned());
            }
            l.working = Some(t.id());
            l.notice = None;
        }
        self.trace(|| {
            format!(
                "{} list: {} on {}",
                scope.raw(),
                change.action().raw_value(),
                t.id()
            )
        });
        self.publish();
        match change {
            TicketChange::Pending { .. } => {
                self.timer_stop_if_running(
                    t,
                    &format!("Setting {} to pending stopped its timer.", t.r#ref),
                )
                .await;
            }
            TicketChange::Resolve(_) => {
                self.timer_stop_if_running(t, &format!("Resolving {} stopped its timer.", t.r#ref))
                    .await;
            }
            _ => {}
        }
        let result = match self.0.writes.apply(change, t).await {
            Ok(()) => {
                let done = match change {
                    TicketChange::Pending { .. } => "set to pending",
                    TicketChange::Resolve(_) => "resolved",
                    TicketChange::Reopen => "reopened",
                    TicketChange::Close => "closed",
                };
                {
                    let mut st = self.lock();
                    let l = &mut st.lists[i];
                    l.notice = Some(Notice::ok(format!("{} is {done}.", t.r#ref)));
                    if l.selection.as_deref() == Some(t.id().as_str())
                        && matches!(change, TicketChange::Resolve(_) | TicketChange::Close)
                    {
                        l.set_lookup(Lookup::Found(t.clone()));
                        l.just_changed = Some(t.id());
                    }
                }
                self.publish();
                self.list_refresh(scope).await;
                None
            }
            Err(e) => {
                let verb = match change {
                    TicketChange::Pending { .. } => format!("set {} to pending", t.r#ref),
                    TicketChange::Resolve(_) => format!("resolve {}", t.r#ref),
                    TicketChange::Reopen => format!("reopen {}", t.r#ref),
                    TicketChange::Close => format!("close {}", t.r#ref),
                };
                Some(self.action_error(&e, t, &verb))
            }
        };
        self.lock().lists[i].working = None;
        self.publish();
        result
    }

    /// `applyFromHeader(_:to:)`: Reopen and Close have no sheet, so errors go to the notice.
    pub(crate) async fn list_apply_from_header(
        &self,
        scope: ListScope,
        change: &TicketChange,
        t: &KitTicket,
    ) {
        if let Some(message) = self.list_apply(scope, change, t).await {
            self.lock().lists[scope.index()].notice = Some(Notice::error(message));
            self.publish();
        }
    }

    /// `actionError(_:ticket:verb:)` (ERR-14).
    pub(crate) fn action_error(&self, e: &WriteError, t: &KitTicket, verb: &str) -> String {
        match e {
            WriteError::Failure(Failure::AlreadyTaken { by }) => {
                format!("{by} took {} a moment ago, so it stays with them.", t.r#ref)
            }
            WriteError::Failure(Failure::NoTeam) => {
                "You aren't in any team in iTop, so Assign to me can't pick one. Choose a team and agent instead.".to_owned()
            }
            WriteError::Failure(Failure::CannotAssign { status }) => {
                format!("{} tickets can't be assigned.", status_label(status))
            }
            WriteError::Failure(Failure::InvalidChoice) => "Pick a team and an agent first.".to_owned(),
            WriteError::Failure(Failure::Missing(field)) => format!("Fill in the {field} first."),
            WriteError::Failure(Failure::NotAllowed { status }) => {
                format!("{} tickets can't do that. Refresh to see the ticket as it is now.", status_label(status))
            }
            WriteError::ITop(itop_kit::ITopError::WritesDisabled) => {
                "Changes to iTop are switched off in this build, so nothing was sent.".to_owned()
            }
            other => format!("iTop didn't {verb}. {}", with_period(&self.describe_write(other))),
        }
    }

    /// `show(_:notice:)`: a ticket that may not be in the list yet, as the search result, selected.
    pub(crate) fn list_show(&self, scope: ListScope, t: &KitTicket, notice: Option<String>) {
        let changed = {
            let mut st = self.lock();
            let l = &mut st.lists[scope.index()];
            let changed = l.query != t.r#ref;
            if changed {
                l.query = t.r#ref.clone();
                l.page = 1;
            }
            l.set_lookup(Lookup::Found(t.clone()));
            l.selection = Some(t.id());
            if let Some(text) = notice {
                l.notice = Some(Notice::ok(text));
            }
            changed
        };
        if changed {
            self.restart_search(scope);
        }
        self.publish();
    }

    /// "Open in iTop on the web" for one ticket (DET-24).
    pub(crate) fn web_url(&self, t: &KitTicket) -> String {
        format!(
            "{}/pages/UI.php?operation=details&class={}&id={}",
            self.base_url(),
            t.ticket_class.raw_value(),
            t.key
        )
    }
}
