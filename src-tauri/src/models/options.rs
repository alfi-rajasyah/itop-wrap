//! Port of `ListOptions.swift`: the date range, resolved/closed toggle, sort and page size shared
//! by every ticket list, remembered between launches (PERS-3…6, PERS-9).

use itop_kit::{TicketScope, TicketSort as KitSort};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::prefs::Prefs;
use super::records::TicketSort;
use super::workspace::Workspace;

pub const RANGE_KEY: &str = "list.range";
pub const INCLUDE_CLOSED_KEY: &str = "list.includeClosed";
pub const SORT_KEY: &str = "list.sort";
pub const PAGE_SIZE_KEY: &str = "list.pageSize";

/// `ListOptions.Range`: 0 any time, 7, 30, 90 days.
pub const RANGES: [i64; 4] = [0, 7, 30, 90];
/// `ListOptions.pageSizes`.
pub const PAGE_SIZES: [Option<i64>; 6] = [Some(10), Some(20), Some(30), Some(50), Some(100), None];
pub const DEFAULT_PAGE_SIZE: Option<i64> = Some(20);

fn range_title(range: i64) -> &'static str {
    match range {
        7 => "Last 7 days",
        30 => "Last 30 days",
        90 => "Last 90 days",
        _ => "Any time",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsState {
    pub range: i64,
    pub include_closed: bool,
    pub sort: KitSort,
    pub page_size: Option<i64>,
}

impl OptionsState {
    /// `ListOptions.init(defaults:)`.
    pub fn load(prefs: &dyn Prefs) -> Self {
        let range = prefs.int(RANGE_KEY).unwrap_or(0);
        let page_size = prefs.int(PAGE_SIZE_KEY).unwrap_or(20);
        OptionsState {
            range: if RANGES.contains(&range) { range } else { 0 },
            include_closed: prefs.bool(INCLUDE_CLOSED_KEY).unwrap_or(false),
            sort: KitSort::from_raw(&prefs.string(SORT_KEY).unwrap_or_default()).unwrap_or(KitSort::Priority),
            page_size: if page_size == 0 { None } else { Some(page_size) },
        }
    }

    pub fn scope(&self) -> TicketScope {
        TicketScope::new(self.include_closed, if self.range == 0 { None } else { Some(self.range) })
    }

    pub fn is_default(&self) -> bool {
        self.scope() == TicketScope::OPEN && self.sort == KitSort::Priority && self.page_size == DEFAULT_PAGE_SIZE
    }

    /// "Open tickets", "Open, last 30 days", "All tickets, any time", "All, last 30 days".
    pub fn range_label(&self) -> String {
        match (self.include_closed, self.range) {
            (false, 0) => "Open tickets".to_owned(),
            (false, r) => format!("Open, {}", range_title(r).to_lowercase()),
            (true, 0) => "All tickets, any time".to_owned(),
            (true, r) => format!("All, {}", range_title(r).to_lowercase()),
        }
    }

    /// What differs from the default (LIST-42).
    pub fn summary(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if self.scope() != TicketScope::OPEN {
            parts.push(self.range_label());
        }
        if self.sort != KitSort::Priority {
            parts.push(self.sort.title().to_owned());
        }
        if self.page_size != DEFAULT_PAGE_SIZE {
            parts.push(self.page_size.map_or_else(|| "All on one page".to_owned(), |n| format!("{n} per page")));
        }
        if parts.is_empty() { None } else { Some(parts.join(" · ")) }
    }

    pub fn snapshot(&self) -> OptionsSnapshot {
        let summary = self.summary();
        OptionsSnapshot {
            range: self.range,
            include_closed: self.include_closed,
            sort: self.sort.into(),
            page_size: self.page_size,
            is_default: self.is_default(),
            help: summary.clone().unwrap_or_else(|| "Open tickets, sorted by priority, 20 per page".to_owned()),
            accessibility_label: format!("View options: {}", summary.as_deref().unwrap_or("default")),
            summary,
            range_choices: RANGES
                .iter()
                .map(|&r| Choice { value: r, title: range_title(r).to_owned(), selected: r == self.range })
                .collect(),
            sort_choices: KitSort::ALL
                .iter()
                .map(|&s| Choice { value: s.into(), title: s.title().to_owned(), selected: s == self.sort })
                .collect(),
            page_size_choices: PAGE_SIZES
                .iter()
                .map(|&p| Choice {
                    value: p,
                    title: p.map_or_else(|| "All".to_owned(), |n| format!("{n} per page")),
                    selected: p == self.page_size,
                })
                .collect(),
        }
    }

    fn save(&self, prefs: &dyn Prefs) {
        prefs.set_int(RANGE_KEY, self.range);
        prefs.set_bool(INCLUDE_CLOSED_KEY, self.include_closed);
        prefs.set_string(SORT_KEY, Some(self.sort.raw_value()));
        prefs.set_int(PAGE_SIZE_KEY, self.page_size.unwrap_or(0));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct Choice<T> {
    pub value: T,
    pub title: String,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OptionsSnapshot {
    #[ts(type = "0 | 7 | 30 | 90")]
    pub range: i64,
    pub include_closed: bool,
    pub sort: TicketSort,
    pub page_size: Option<i64>,
    pub is_default: bool,
    pub summary: Option<String>,
    pub help: String,
    pub accessibility_label: String,
    #[ts(type = "Array<Choice<0 | 7 | 30 | 90>>")]
    pub range_choices: Vec<Choice<i64>>,
    pub sort_choices: Vec<Choice<TicketSort>>,
    pub page_size_choices: Vec<Choice<Option<i64>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum OptionsIntent {
    SetRange {
        #[ts(type = "0 | 7 | 30 | 90")]
        range: i64,
    },
    SetIncludeClosed {
        #[serde(rename = "includeClosed")]
        #[ts(rename = "includeClosed")]
        include_closed: bool,
    },
    SetSort {
        sort: TicketSort,
    },
    SetPageSize {
        #[serde(rename = "pageSize")]
        #[ts(rename = "pageSize")]
        page_size: Option<i64>,
    },
    Reset,
}

impl Workspace {
    pub(crate) async fn dispatch_options(&self, intent: OptionsIntent) -> Result<serde_json::Value, crate::error::AppError> {
        let before = self.lock().options.clone();
        let mut after = before.clone();
        match intent {
            OptionsIntent::SetRange { range } => {
                if !RANGES.contains(&range) {
                    return Err(crate::error::AppError::invalid_intent(format!("No range of {range} days.")));
                }
                after.range = range;
            }
            OptionsIntent::SetIncludeClosed { include_closed } => after.include_closed = include_closed,
            OptionsIntent::SetSort { sort } => after.sort = sort.into(),
            OptionsIntent::SetPageSize { page_size } => {
                if matches!(page_size, Some(n) if n <= 0) {
                    return Err(crate::error::AppError::invalid_intent("A page size must be above zero."));
                }
                after.page_size = page_size;
            }
            // `reset()`: one render, so the shown view reacts once to the final values.
            OptionsIntent::Reset => {
                after.range = 0;
                after.include_closed = false;
                after.sort = KitSort::Priority;
                after.page_size = DEFAULT_PAGE_SIZE;
            }
        }
        self.options_changed(before, after).await;
        Ok(serde_json::Value::Null)
    }

    /// Every assignment writes the defaults (the `didSet`s); then only the view on screen reacts
    /// (`onChange` in `TicketListView`, `OrgDetailView`, `PersonDetailView`, `TicketTable`).
    async fn options_changed(&self, before: OptionsState, after: OptionsState) {
        after.save(self.0.prefs.as_ref());
        let (sort, page_size) = (after.sort, after.page_size);
        let scope_changed = before.scope() != after.scope();
        let page_reset = scope_changed || before.sort != after.sort || before.page_size != after.page_size;
        let shown = {
            let mut st = self.lock();
            st.options = after;
            st.shell.selection
        };
        self.trace(|| "view: options changed".to_owned());
        if let Some(scope) = shown.list() {
            if page_reset {
                self.lock().lists[scope.index()].page = 1;
            }
            self.publish();
            if scope_changed {
                let ws = self.clone();
                self.spawn(async move { ws.list_refresh(scope).await });
            }
        } else {
            self.publish();
        }
        // The org and person tables: the models reload only when their view is on screen.
        if let Some(s) = self.standalone() {
            if scope_changed {
                s.orgs.scope_changed();
                s.people.scope_changed();
            }
            if before.sort != sort || before.page_size != page_size {
                s.orgs.set_table_options(sort, page_size);
                s.people.set_table_options(sort, page_size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::prefs::MemoryPrefs;
    use itop_kit::SettingsStore;

    #[test]
    fn pers_3_to_6_defaults_and_unknown_values() {
        let prefs = MemoryPrefs::new();
        let o = OptionsState::load(&prefs);
        assert_eq!(o, OptionsState { range: 0, include_closed: false, sort: KitSort::Priority, page_size: Some(20) });
        prefs.set_int(RANGE_KEY, 14);
        prefs.set_string(SORT_KEY, Some("bogus"));
        prefs.set_int(PAGE_SIZE_KEY, 0);
        let o = OptionsState::load(&prefs);
        assert_eq!(o.range, 0);
        assert_eq!(o.sort, KitSort::Priority);
        assert_eq!(o.page_size, None, "0 = All");
        prefs.set_int(RANGE_KEY, 30);
        prefs.set_bool(INCLUDE_CLOSED_KEY, true);
        prefs.set_string(SORT_KEY, Some("lastUpdate"));
        prefs.set_int(PAGE_SIZE_KEY, 50);
        assert_eq!(
            OptionsState::load(&prefs),
            OptionsState { range: 30, include_closed: true, sort: KitSort::LastUpdate, page_size: Some(50) }
        );
    }

    #[test]
    fn list_41_default_view() {
        let o = OptionsState { range: 0, include_closed: false, sort: KitSort::Priority, page_size: Some(20) };
        assert!(o.is_default());
        assert_eq!(o.summary(), None);
        let s = o.snapshot();
        assert_eq!(s.help, "Open tickets, sorted by priority, 20 per page");
        assert_eq!(s.accessibility_label, "View options: default");
    }

    #[test]
    fn list_42_summary_parts() {
        let o = OptionsState { range: 0, include_closed: true, sort: KitSort::LastUpdate, page_size: Some(50) };
        assert_eq!(o.summary().as_deref(), Some("All tickets, any time · Last update, newest first · 50 per page"));
        let o = OptionsState { range: 30, include_closed: false, sort: KitSort::Priority, page_size: None };
        assert_eq!(o.summary().as_deref(), Some("Open, last 30 days · All on one page"));
        let o = OptionsState { range: 7, include_closed: true, sort: KitSort::Priority, page_size: Some(20) };
        assert_eq!(o.summary().as_deref(), Some("All, last 7 days"));
        assert_eq!(o.snapshot().accessibility_label, "View options: All, last 7 days");
    }

    #[test]
    fn list_40_choices() {
        let o = OptionsState { range: 7, include_closed: false, sort: KitSort::Newest, page_size: None };
        let v = serde_json::to_value(o.snapshot()).unwrap_or_default();
        assert_eq!(v["rangeChoices"][1], serde_json::json!({"value":7,"title":"Last 7 days","selected":true}));
        assert_eq!(v["sortChoices"][2], serde_json::json!({"value":"newest","title":"Opened, newest first","selected":true}));
        assert_eq!(v["pageSizeChoices"][5], serde_json::json!({"value":null,"title":"All","selected":true}));
        assert_eq!(v["pageSizeChoices"][0]["title"], "10 per page");
        assert_eq!(v["pageSize"], serde_json::Value::Null);
        assert_eq!(v["range"], 7);
    }
}
