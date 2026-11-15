//! Port of `TicketQueries.swift`: `TicketScope`, `TicketSort`, `TicketQueries`, `TicketService`,
//! `TicketGroup`, `TicketSection`, `TicketList`, `TicketPage`, `TicketSearch` and `Age`.
//!
//! Also home of [`localized_standard_compare`], the port of Foundation's
//! `localizedStandardCompare`, which this file is the first to need (the org filter menu) and
//! which `config_item`, `organization` and `rich_text` sort with too.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;
use ts_rs::TS;

use crate::client::ITopClient;
use crate::current_user::CurrentUser;
use crate::error::{ITopError, is_swift_whitespace};
use crate::json::swift_int;
use crate::oql;
use crate::rest_operation::RestOperation;
use crate::ticket::{Ticket, TicketClass};

// MARK: - Scope

/// Which tickets a list covers over time: open ones by default, optionally only those opened
/// recently, and optionally resolved and closed ones too, for any time range. With resolved and
/// closed over any time, a busy org's whole history is loaded: nothing is cut, because iTop's
/// OQL can't order results and a cut couldn't keep the newest ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TicketScope {
    pub include_closed: bool,
    #[ts(type = "number | null")]
    pub opened_within_days: Option<i64>,
}

impl Default for TicketScope {
    fn default() -> Self {
        TicketScope::OPEN
    }
}

impl TicketScope {
    pub const OPEN: TicketScope = TicketScope {
        include_closed: false,
        opened_within_days: None,
    };

    pub fn new(include_closed: bool, opened_within_days: Option<i64>) -> Self {
        TicketScope {
            include_closed,
            opened_within_days,
        }
    }

    /// The OQL condition. The date test runs on the server's clock, so time zones don't matter.
    pub fn clause(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.include_closed {
            parts.push(TicketQueries::OPEN.to_string());
        }
        if let Some(days) = self.opened_within_days {
            parts.push(format!(
                "start_date >= DATE_SUB(NOW(), INTERVAL {days} DAY)"
            ));
        }
        // Every ticket: an always-true condition keeps "WHERE … AND …" valid OQL.
        if parts.is_empty() {
            "id > 0".to_string()
        } else {
            parts.join(" AND ")
        }
    }
}

// MARK: - Sort

/// The order inside each status group (the sort menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketSort {
    #[default]
    Priority,
    LastUpdate,
    Newest,
    Oldest,
}

/// Swift's `Date.distantPast`: 0001-01-01 00:00:00 UTC.
pub(crate) fn distant_past() -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(1, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc())
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// Swift's `Date.distantFuture`: 4001-01-01 00:00:00 UTC. The real value, not chrono's maximum,
/// so a (bogus) date after year 4001 sorts against a missing date exactly as in Swift.
fn distant_future() -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(4001, 1, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc())
        .unwrap_or(DateTime::<Utc>::MAX_UTC)
}

impl TicketSort {
    /// `CaseIterable.allCases`, in declaration order.
    pub const ALL: [TicketSort; 4] = [
        TicketSort::Priority,
        TicketSort::LastUpdate,
        TicketSort::Newest,
        TicketSort::Oldest,
    ];

    /// Swift's `rawValue` (and `id`).
    pub fn raw_value(self) -> &'static str {
        match self {
            TicketSort::Priority => "priority",
            TicketSort::LastUpdate => "lastUpdate",
            TicketSort::Newest => "newest",
            TicketSort::Oldest => "oldest",
        }
    }

    /// `TicketSort(rawValue:)`.
    pub fn from_raw(raw: &str) -> Option<TicketSort> {
        TicketSort::ALL.into_iter().find(|s| s.raw_value() == raw)
    }

    /// `Identifiable.id`.
    pub fn id(self) -> &'static str {
        self.raw_value()
    }

    pub fn title(self) -> &'static str {
        match self {
            TicketSort::Priority => "Priority, then oldest",
            TicketSort::LastUpdate => "Last update, newest first",
            TicketSort::Newest => "Opened, newest first",
            TicketSort::Oldest => "Opened, oldest first",
        }
    }

    /// Swift's `before(_:_:)`, the `sorted(by:)` predicate. Refs compare like Swift's `String <`,
    /// which is by Unicode scalars; Rust's byte order is the same for the ASCII refs iTop makes.
    pub fn before(self, a: &Ticket, b: &Ticket) -> bool {
        let past = distant_past();
        let future = distant_future();
        match self {
            TicketSort::Priority => {
                (a.priority, a.start_date.unwrap_or(future), &a.r#ref)
                    < (b.priority, b.start_date.unwrap_or(future), &b.r#ref)
            }
            TicketSort::LastUpdate => {
                (a.last_update.unwrap_or(past), &b.r#ref)
                    > (b.last_update.unwrap_or(past), &a.r#ref)
            }
            TicketSort::Newest => {
                (a.start_date.unwrap_or(past), &b.r#ref) > (b.start_date.unwrap_or(past), &a.r#ref)
            }
            TicketSort::Oldest => {
                (a.start_date.unwrap_or(future), &a.r#ref)
                    < (b.start_date.unwrap_or(future), &b.r#ref)
            }
        }
    }

    /// `before` as an `Ordering`, for Rust's (stable, like Swift's) sort.
    fn ordering(self, a: &Ticket, b: &Ticket) -> Ordering {
        if self.before(a, b) {
            Ordering::Less
        } else if self.before(b, a) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

// MARK: - Queries

/// The OQL behind each list (PLAN.md Task 3; the "needs you" rule from DESIGN.md).
pub struct TicketQueries;

impl TicketQueries {
    pub(crate) const OPEN: &'static str = "status NOT IN ('resolved','closed')";

    /// Tickets assigned to me, plus unassigned Escalated TTO tickets in my teams.
    pub fn mine(cls: TicketClass, me: &CurrentUser, scope: &TicketScope) -> String {
        let assigned = format!("agent_id = {}", me.contact_id);
        if me.teams.is_empty() {
            return format!(
                "SELECT {} WHERE {} AND {assigned}",
                cls.raw_value(),
                scope.clause()
            );
        }
        format!(
            "SELECT {} WHERE {} AND ({assigned} OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN ({})))",
            cls.raw_value(),
            scope.clause(),
            Self::team_ids(me)
        )
    }

    /// Everything in my teams, or `None` when I'm in no team.
    pub fn team(cls: TicketClass, me: &CurrentUser, scope: &TicketScope) -> Option<String> {
        if me.teams.is_empty() {
            return None;
        }
        Some(format!(
            "SELECT {} WHERE {} AND team_id IN ({})",
            cls.raw_value(),
            scope.clause(),
            Self::team_ids(me)
        ))
    }

    /// For a full ref like "R-000512", the class and the query that finds it; otherwise `None`.
    pub fn ref_lookup(text: &str) -> Option<(TicketClass, String)> {
        let r = text.trim_matches(is_swift_whitespace).to_uppercase();
        // `/[RI]-\d{6}/`: Swift's `\d` is any Unicode decimal digit (Nd), not just ASCII.
        let chars: Vec<char> = r.chars().collect();
        let well_formed = chars.len() == 8
            && (chars[0] == 'R' || chars[0] == 'I')
            && chars[1] == '-'
            && chars[2..].iter().all(|c| is_unicode_decimal_digit(*c));
        if !well_formed {
            return None;
        }
        let cls = TicketClass::ALL
            .into_iter()
            .find(|c| r.starts_with(&format!("{}-", c.ref_prefix())))?;
        Some((
            cls,
            format!("SELECT {} WHERE ref = {}", cls.raw_value(), oql::quote(&r)),
        ))
    }

    /// Team ids that are numbers (Swift's `compactMap { Int($0.id) }`), comma-separated.
    fn team_ids(me: &CurrentUser) -> String {
        me.teams
            .iter()
            .filter_map(|t| swift_int(&t.id))
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Unicode general category Nd, which Swift Regex's `\d` matches. `char::is_numeric` is wider
/// (Nl, No), so the decimal-digit blocks are listed; each block holds ten digits from its start.
fn is_unicode_decimal_digit(c: char) -> bool {
    if c.is_ascii_digit() {
        return true;
    }
    if c.is_ascii() {
        return false;
    }
    // A decimal digit has a numeric value 0-9 and is in a run of ten; char::to_digit is ASCII
    // only, so check against the zeros of the Nd runs.
    const ZEROS: &[u32] = &[
        0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6,
        0x0D66, 0x0DE6, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0,
        0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0,
        0xAA50, 0xABF0, 0xFF10, 0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0,
        0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50, 0x11D50, 0x11DA0,
        0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140,
        0x1E2F0, 0x1E4F0, 0x1E950, 0x1FBF0,
    ];
    let v = c as u32;
    ZEROS.iter().any(|z| (*z..*z + 10).contains(&v))
}

// MARK: - Service

/// Loads the lists. Read-only.
#[derive(Clone, Debug)]
pub struct TicketService {
    pub(crate) client: ITopClient,
}

impl TicketService {
    pub fn new(client: ITopClient) -> Self {
        TicketService { client }
    }

    pub async fn load_mine(
        &self,
        me: &CurrentUser,
        scope: &TicketScope,
    ) -> Result<Vec<Ticket>, ITopError> {
        self.load(
            TicketClass::ALL
                .into_iter()
                .map(|cls| (cls, TicketQueries::mine(cls, me, scope)))
                .collect(),
        )
        .await
    }

    pub async fn load_team(
        &self,
        me: &CurrentUser,
        scope: &TicketScope,
    ) -> Result<Vec<Ticket>, ITopError> {
        self.load(
            TicketClass::ALL
                .into_iter()
                .filter_map(|cls| TicketQueries::team(cls, me, scope).map(|q| (cls, q)))
                .collect(),
        )
        .await
    }

    /// A ticket by full ref, for search; `None` if it doesn't exist or this account can't see it.
    pub async fn lookup(&self, r: &str) -> Result<Option<Ticket>, ITopError> {
        let Some((cls, oql)) = TicketQueries::ref_lookup(r) else {
            return Ok(None);
        };
        Ok(self.load(vec![(cls, oql)]).await?.into_iter().next())
    }

    /// The queries run concurrently, like Swift's task group. Swift concatenates the results in
    /// the order the queries finish, which is not fixed; here they come in query order, one
    /// of the orders Swift can produce. The first error (in finishing order) is returned and the
    /// other queries are cancelled, as in Swift.
    async fn load(&self, queries: Vec<(TicketClass, String)>) -> Result<Vec<Ticket>, ITopError> {
        let mut set: JoinSet<(usize, Result<Vec<Ticket>, ITopError>)> = JoinSet::new();
        let count = queries.len();
        for (index, (cls, oql)) in queries.into_iter().enumerate() {
            let client = self.client.clone();
            set.spawn(async move {
                let result = client
                    .send(RestOperation::get(
                        cls.raw_value(),
                        oql,
                        Ticket::LIST_FIELDS,
                    ))
                    .await
                    .map(|r| {
                        r.objects()
                            .iter()
                            .filter_map(Ticket::from_object)
                            .collect::<Vec<_>>()
                    });
                (index, result)
            });
        }
        let mut parts: Vec<Option<Vec<Ticket>>> = vec![None; count];
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((index, Ok(tickets))) => parts[index] = Some(tickets),
                Ok((_, Err(e))) => return Err(e),
                // A panicking request task is a bug; report it as an unusable answer rather
                // than unwinding through the caller.
                Err(_) => return Err(ITopError::UnexpectedResponse),
            }
        }
        Ok(parts.into_iter().flatten().flatten().collect())
    }
}

// MARK: - Grouping

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketGroup {
    #[serde(rename = "escalatedTTR")]
    EscalatedTtr,
    #[serde(rename = "escalatedTTO")]
    EscalatedTto,
    #[serde(rename = "new")]
    New,
    #[serde(rename = "assigned")]
    Assigned,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "other")]
    Other,
    #[serde(rename = "resolved")]
    Resolved,
    #[serde(rename = "closed")]
    Closed,
}

impl TicketGroup {
    /// `CaseIterable.allCases`, in declaration order.
    pub const ALL: [TicketGroup; 8] = [
        TicketGroup::EscalatedTtr,
        TicketGroup::EscalatedTto,
        TicketGroup::New,
        TicketGroup::Assigned,
        TicketGroup::Pending,
        TicketGroup::Other,
        TicketGroup::Resolved,
        TicketGroup::Closed,
    ];

    /// Swift's `rawValue`.
    pub fn raw_value(self) -> &'static str {
        match self {
            TicketGroup::EscalatedTtr => "escalatedTTR",
            TicketGroup::EscalatedTto => "escalatedTTO",
            TicketGroup::New => "new",
            TicketGroup::Assigned => "assigned",
            TicketGroup::Pending => "pending",
            TicketGroup::Other => "other",
            TicketGroup::Resolved => "resolved",
            TicketGroup::Closed => "closed",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            TicketGroup::EscalatedTtr => "Escalated TTR",
            TicketGroup::EscalatedTto => "Escalated TTO",
            TicketGroup::New => "New",
            TicketGroup::Assigned => "Assigned",
            TicketGroup::Pending => "Pending",
            TicketGroup::Other => "Other",
            TicketGroup::Resolved => "Resolved",
            TicketGroup::Closed => "Closed",
        }
    }

    /// The group of an iTop status code; anything unknown is Other.
    pub fn of(status: &str) -> TicketGroup {
        match status {
            "escalated_ttr" => TicketGroup::EscalatedTtr,
            "escalated_tto" => TicketGroup::EscalatedTto,
            "new" => TicketGroup::New,
            "assigned" => TicketGroup::Assigned,
            "pending" => TicketGroup::Pending,
            "resolved" => TicketGroup::Resolved,
            "closed" => TicketGroup::Closed,
            _ => TicketGroup::Other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TicketSection {
    pub group: TicketGroup,
    pub tickets: Vec<Ticket>,
}

impl TicketSection {
    pub fn new(group: TicketGroup, tickets: Vec<Ticket>) -> Self {
        TicketSection { group, tickets }
    }

    /// `Identifiable.id`.
    pub fn id(&self) -> TicketGroup {
        self.group
    }
}

/// Swift's `TicketList.View`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum TicketListView {
    Mine,
    Team,
}

pub struct TicketList;

impl TicketList {
    /// "Needs you" (DESIGN.md): my Escalated TTR, or an unassigned Escalated TTO in one of my teams.
    pub fn needs_you(t: &Ticket, me: &CurrentUser) -> bool {
        match t.status.as_str() {
            "escalated_ttr" => t.agent_id == me.contact_id,
            "escalated_tto" => {
                t.is_unassigned() && me.teams.iter().any(|team| team.id == t.team_id)
            }
            _ => false,
        }
    }

    /// Sections in DESIGN.md order, escalated first; empty ones are left out.
    /// Inside a section: priority 1 first, then the oldest (or the chosen sort).
    pub fn groups(
        tickets: &[Ticket],
        _me: &CurrentUser,
        view: TicketListView,
        sort: TicketSort,
    ) -> Vec<TicketSection> {
        use TicketGroup::*;
        let order: [TicketGroup; 8] = if view == TicketListView::Team {
            [
                EscalatedTtr,
                EscalatedTto,
                New,
                Assigned,
                Pending,
                Other,
                Resolved,
                Closed,
            ]
        } else {
            [
                EscalatedTtr,
                EscalatedTto,
                Assigned,
                New,
                Pending,
                Other,
                Resolved,
                Closed,
            ]
        };
        // `Dictionary(grouping:)` keeps the tickets' order inside each group.
        let mut by_group: HashMap<TicketGroup, Vec<Ticket>> = HashMap::new();
        for t in tickets {
            by_group
                .entry(TicketGroup::of(&t.status))
                .or_default()
                .push(t.clone());
        }
        order
            .into_iter()
            .filter_map(|g| {
                let mut list = by_group.remove(&g)?;
                if list.is_empty() {
                    return None;
                }
                list.sort_by(|a, b| sort.ordering(a, b));
                Some(TicketSection::new(g, list))
            })
            .collect()
    }
}

// MARK: - Paging

/// One page of grouped tickets (the "Show 20" menu). Paging happens in the app, after grouping
/// and sorting, so page 1 holds the most urgent tickets and the sort covers the whole list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TicketPage {
    pub sections: Vec<TicketSection>,
    #[ts(type = "number")]
    pub page: i64,
    #[ts(type = "number")]
    pub page_count: i64,
    #[ts(type = "number")]
    pub total: i64,
    /// Private in Swift (read through `group_total`); serialised so the UI can show headers.
    #[ts(type = "Partial<Record<TicketGroup, number>>")]
    totals: BTreeMap<TicketGroup, i64>,
    #[ts(type = "number")]
    first: i64,
    #[ts(type = "number")]
    last: i64,
}

impl TicketPage {
    /// `size` `None` means everything on one page.
    ///
    /// Swift builds `totals` with `Dictionary(uniqueKeysWithValues:)`, which traps on a repeated
    /// group; `TicketList::groups` never repeats one. Here a repeated group keeps the last count.
    pub fn new(all: Vec<TicketSection>, size: Option<i64>, requested: i64) -> TicketPage {
        let total: i64 = all.iter().map(|s| s.tickets.len() as i64).sum();
        let totals: BTreeMap<TicketGroup, i64> = all
            .iter()
            .map(|s| (s.group, s.tickets.len() as i64))
            .collect();
        let size = match size {
            Some(size) if size > 0 && total > 0 => size,
            _ => {
                return TicketPage {
                    sections: all,
                    page: 1,
                    page_count: 1,
                    total,
                    totals,
                    first: if total > 0 { 1 } else { 0 },
                    last: total,
                };
            }
        };
        let page_count = ((total + size - 1) / size).max(1);
        let page = requested.max(1).min(page_count);
        let start = (page - 1) * size;
        let end = (start + size).min(total);
        let mut out: Vec<TicketSection> = Vec::new();
        let mut index: i64 = 0;
        for section in &all {
            let count = section.tickets.len() as i64;
            // Check the bounds before slicing: a group that ends before this page starts
            // would give lo > hi.
            let lo = start.max(index);
            let hi = end.min(index + count);
            if lo < hi {
                out.push(TicketSection::new(
                    section.group,
                    section.tickets[(lo - index) as usize..(hi - index) as usize].to_vec(),
                ));
            }
            index += count;
        }
        TicketPage {
            sections: out,
            page,
            page_count,
            total,
            totals,
            first: start + 1,
            last: end,
        }
    }

    /// How many tickets the whole group has, for its header: a page may show only part of it.
    pub fn group_total(&self, group: TicketGroup) -> i64 {
        self.totals.get(&group).copied().unwrap_or(0)
    }

    /// "21–40 of 556" (en dash).
    pub fn label(&self) -> String {
        if self.total == 0 {
            "No tickets".to_string()
        } else {
            format!("{}–{} of {}", self.first, self.last, self.total)
        }
    }
}

// MARK: - Search

pub struct TicketSearch;

impl TicketSearch {
    /// The org filter: no orgs chosen means everything.
    pub fn filter_orgs(tickets: &[Ticket], orgs: &HashSet<String>) -> Vec<Ticket> {
        if orgs.is_empty() {
            return tickets.to_vec();
        }
        tickets
            .iter()
            .filter(|t| orgs.contains(&t.org_name))
            .cloned()
            .collect()
    }

    /// The orgs the filter menu offers: the ones these tickets are for.
    ///
    /// Duplicates are found by exact text; Swift's `Set<String>` uses canonical equivalence, so
    /// the same name in two Unicode normal forms would be one entry there and two here.
    pub fn orgs(tickets: &[Ticket]) -> Vec<String> {
        let mut set: Vec<String> = tickets
            .iter()
            .map(|t| t.org_name.clone())
            .filter(|n| !n.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        set.sort_by(|a, b| localized_standard_compare(a, b));
        set
    }

    /// Every word must appear in the ref or the title, ignoring case. Order is kept.
    ///
    /// Matching is by code points after Unicode lowercasing; Swift's `contains` also treats
    /// canonically equivalent text (é as e + ◌́) as equal, which only matters for text that mixes
    /// normal forms.
    pub fn filter_query(tickets: &[Ticket], query: &str) -> Vec<Ticket> {
        let lowered = query.to_lowercase();
        let words: Vec<&str> = lowered.split(' ').filter(|w| !w.is_empty()).collect();
        if words.is_empty() {
            return tickets.to_vec();
        }
        tickets
            .iter()
            .filter(|t| {
                let haystack = format!("{} {}", t.r#ref, t.title).to_lowercase();
                words.iter().all(|w| haystack.contains(w))
            })
            .cloned()
            .collect()
    }
}

// MARK: - Age

pub struct Age;

impl Age {
    /// "now", "40m", "2h", "9d", as in the wireframe's list rows (whole units, rounded down).
    pub fn short(from: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let s = ((now - from).num_milliseconds() as f64 / 1000.0).max(0.0);
        if s < 60.0 {
            "now".to_string()
        } else if s < 3600.0 {
            format!("{}m", (s / 60.0) as i64)
        } else if s < 86400.0 {
            format!("{}h", (s / 3600.0) as i64)
        } else {
            format!("{}d", (s / 86400.0) as i64)
        }
    }
}

// MARK: - localizedStandardCompare

/// Foundation's `a.localizedStandardCompare(b)`, the Finder order the app sorts names with.
///
/// Foundation compares with ICU collation in the current locale, case- and width-insensitive,
/// with digit runs compared by value, and falls back to a literal comparison so only identical
/// strings are equal. ICU isn't available here, so this reproduces the root (and English)
/// collation for what iTop names contain:
///
/// - primary level: whitespace < punctuation and symbols (ASCII in ICU's order) < other symbols
///   < digit runs (by numeric value) < letters (alphabetical, ignoring case and accents);
/// - secondary level: accents (a < á);
/// - tertiary level: case, lowercase first (a < A);
/// - then code points, as `.forcedOrdering` does.
///
/// Where it can differ from Swift: letters of different scripts follow code point order rather
/// than CLDR's script order; non-ASCII symbols sort by code point; locale-specific tailorings
/// (Swedish å after z, for example) are not applied; contractions and expansions (æ as "ae",
/// ß as "ss") are not applied.
pub fn localized_standard_compare(a: &str, b: &str) -> Ordering {
    let ka = collation_key(a);
    let kb = collation_key(b);
    let primary = |k: &[Element]| k.iter().map(|e| e.primary.clone()).collect::<Vec<_>>();
    primary(&ka)
        .cmp(&primary(&kb))
        .then_with(|| {
            let sec = |k: &[Element]| k.iter().map(|e| e.secondary.clone()).collect::<Vec<_>>();
            sec(&ka).cmp(&sec(&kb))
        })
        .then_with(|| {
            let ter = |k: &[Element]| k.iter().map(|e| e.tertiary).collect::<Vec<_>>();
            ter(&ka).cmp(&ter(&kb))
        })
        .then_with(|| a.cmp(b))
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Primary {
    Space(u32),
    AsciiPunct(u32),
    Symbol(u32),
    /// A digit run: its value as digits without leading zeros, compared by length then digits.
    Number(usize, String),
    Letter(u32),
}

#[derive(Clone, Debug)]
struct Element {
    primary: Primary,
    /// The combining marks on this element (accents), in order.
    secondary: Vec<u32>,
    /// 0 for lowercase or uncased, 1 for uppercase.
    tertiary: u8,
}

/// ICU's root order for ASCII punctuation and symbols.
const ASCII_PUNCT_ORDER: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn is_combining_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn nfd(s: &str) -> String {
    static NORMALIZER: OnceLock<icu_normalizer::DecomposingNormalizerBorrowed<'static>> =
        OnceLock::new();
    NORMALIZER
        .get_or_init(icu_normalizer::DecomposingNormalizerBorrowed::new_nfd)
        .normalize(s)
        .into_owned()
}

fn collation_key(s: &str) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::new();
    let mut digits = String::new();
    let flush_digits = |digits: &mut String, out: &mut Vec<Element>| {
        if digits.is_empty() {
            return;
        }
        let trimmed = digits.trim_start_matches('0');
        let value = if trimmed.is_empty() { "0" } else { trimmed };
        out.push(Element {
            primary: Primary::Number(value.len(), value.to_string()),
            secondary: Vec::new(),
            tertiary: 0,
        });
        digits.clear();
    };
    for raw in nfd(s).chars() {
        // Width-insensitive: fullwidth ASCII forms read as ASCII.
        let c = match raw as u32 {
            0xFF01..=0xFF5E => char::from_u32(raw as u32 - 0xFEE0).unwrap_or(raw),
            0x3000 => ' ',
            _ => raw,
        };
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if is_combining_mark(c) {
            if !digits.is_empty() {
                flush_digits(&mut digits, &mut out);
            }
            if let Some(last) = out.last_mut() {
                last.secondary.push(c as u32);
            }
            continue;
        }
        flush_digits(&mut digits, &mut out);
        let (primary, tertiary) = if c.is_whitespace() {
            (Primary::Space(c as u32), 0)
        } else if let Some(i) = ASCII_PUNCT_ORDER.find(c) {
            (Primary::AsciiPunct(i as u32), 0)
        } else if c.is_alphanumeric() {
            let lower = c.to_lowercase().next().unwrap_or(c);
            (Primary::Letter(lower as u32), u8::from(lower != c))
        } else {
            (Primary::Symbol(c as u32), 0)
        };
        out.push(Element {
            primary,
            secondary: Vec::new(),
            tertiary,
        });
    }
    flush_digits(&mut digits, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        v.sort_by(|a, b| localized_standard_compare(a, b));
        v
    }

    #[test]
    fn finder_order() {
        assert_eq!(
            sorted(&["vpn-gw-02", "Remote access VPN", "file10", "file2", "File1"]),
            ["File1", "file2", "file10", "Remote access VPN", "vpn-gw-02"]
        );
        // Case and accents only break ties; digit runs compare by value; spaces and
        // punctuation come before letters.
        assert_eq!(sorted(&["B", "b", "a"]), ["a", "b", "B"]);
        assert_eq!(sorted(&["cote", "côte", "cotf"]), ["cote", "côte", "cotf"]);
        assert_eq!(
            sorted(&["Org 10", "Org 9", "Org-1", "Org A"]),
            ["Org 9", "Org 10", "Org A", "Org-1"]
        );
        assert_eq!(localized_standard_compare("x", "x"), Ordering::Equal);
    }
}
