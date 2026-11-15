//! The ticket table shared by an organization and a person (`TicketTable`, `PersonChip` and
//! `Pager`, W/OrgsView.swift:7-107 and W/ListOptions.swift:156-178): ORG-11, PPL-14.

use itop_kit::{
    CurrentUser, Ticket, TicketGroup, TicketList, TicketListView, TicketPage, TicketSort,
};
use serde::Serialize;
use ts_rs::TS;

use crate::models::cmdb::text::chip_initials;

pub use crate::models::records::{AgentChip, Pager, pager};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TableRow {
    pub ticket_id: String,
    pub r#ref: String,
    pub title: String,
    pub needs_you: bool,
    /// Only P1 is marked in tables.
    pub p1: bool,
    pub caller_name: String,
    pub assignee: AgentChip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TableSection {
    pub group: TicketGroup,
    pub title: String,
    pub total: i32,
    pub rows: Vec<TableRow>,
}

/// "assignee": Ref / Title / Caller / Assignee. "caller": Ref / Title / Caller (caller last).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum LastColumn {
    Assignee,
    Caller,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TicketTable {
    pub last_column: LastColumn,
    pub sections: Vec<TableSection>,
    pub pager: Pager,
}

/// The ticket area of an org or a person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TableState {
    Loading {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Failed {
        text: String,
        can_retry: bool,
    },
    Empty {
        text: String,
    },
    Loaded {
        table: TicketTable,
    },
}

/// The shared sort and page size (ListOptions) and the table's own page (`@State page`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableOptions {
    pub sort: TicketSort,
    /// `None` = All.
    pub page_size: Option<i64>,
    /// The page asked for; `TicketPage` clamps it.
    pub page: i64,
}

impl Default for TableOptions {
    /// ListOptions' defaults: priority, 20 per page.
    fn default() -> Self {
        Self {
            sort: TicketSort::Priority,
            page_size: Some(20),
            page: 1,
        }
    }
}

impl TableOptions {
    /// `onChange(of: sort / pageSize) { page = 1 }`. Returns whether anything changed.
    pub fn set_view(&mut self, sort: TicketSort, page_size: Option<i64>) -> bool {
        if self.sort == sort && self.page_size == page_size {
            return false;
        }
        self.sort = sort;
        self.page_size = page_size;
        self.page = 1;
        true
    }
}

/// `TicketTable.body`: grouped in Team order with the chosen sort, then paged.
pub fn table(
    tickets: &[Ticket],
    me: &CurrentUser,
    last_column: LastColumn,
    opts: &TableOptions,
) -> TicketTable {
    let paged = TicketPage::new(
        TicketList::groups(tickets, me, TicketListView::Team, opts.sort),
        opts.page_size,
        opts.page,
    );
    let sections = paged
        .sections
        .iter()
        .map(|s| TableSection {
            group: s.group,
            title: s.group.title().to_owned(),
            total: paged.group_total(s.group) as i32,
            rows: s.tickets.iter().map(|t| row(t, me)).collect(),
        })
        .collect();
    TicketTable {
        last_column,
        sections,
        pager: pager(&paged),
    }
}

fn row(t: &Ticket, me: &CurrentUser) -> TableRow {
    let mine = t.agent_id == me.contact_id;
    let assignee = if t.is_unassigned() {
        AgentChip::Unassigned
    } else {
        let name = if mine {
            "You".to_owned()
        } else {
            t.agent_name.clone()
        };
        let initials = if mine {
            me.initials()
        } else {
            chip_initials(&name)
        };
        AgentChip::Person { initials, name }
    };
    TableRow {
        ticket_id: t.id(),
        r#ref: t.r#ref.clone(),
        title: t.title.clone(),
        needs_you: TicketList::needs_you(t, me),
        p1: t.priority == 1,
        caller_name: t.caller_name.clone(),
        assignee,
    }
}
