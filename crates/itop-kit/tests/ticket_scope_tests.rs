//! Port of Tests/ITopKitTests/TicketScopeTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.

use itop_kit::{
    CurrentUser, OrgQueries, Team, Ticket, TicketClass, TicketGroup, TicketList, TicketListView,
    TicketPage, TicketQueries, TicketScope, TicketSection, TicketSort,
};

fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1".into(),
        contact_id: "2".into(),
        login: "a".into(),
        display_name: "A".into(),
        profiles: vec![],
        teams: vec![Team::new("10", "N")],
    }
}

/// The Swift helper `t(_:status:prio:opened:updated:)`.
fn t(r: &str, status: &str, prio: i64, opened: &str, updated: &str) -> Ticket {
    Ticket {
        key: r.into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: r.into(),
        title: r.into(),
        status: status.into(),
        priority: prio,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: "0".into(),
        team_id: "10".into(),
        team_name: String::new(),
        agent_id: "2".into(),
        agent_name: String::new(),
        start_date: Ticket::parse_date(opened),
        last_update: Ticket::parse_date(updated),
    }
}

fn groups_of(sections: &[TicketSection]) -> Vec<TicketGroup> {
    sections.iter().map(|s| s.group).collect()
}

fn refs(section: Option<&TicketSection>) -> Vec<String> {
    section
        .map(|s| s.tickets.iter().map(|t| t.r#ref.clone()).collect())
        .unwrap_or_default()
}

mod ticket_scope_tests {
    use super::*;

    /// Default: open tickets, any time
    #[test]
    fn open() {
        assert_eq!(
            TicketScope::OPEN.clause(),
            "status NOT IN ('resolved','closed')"
        );
    }

    /// Open tickets opened in the last 30 days
    #[test]
    fn open_recent() {
        assert_eq!(
            TicketScope::new(false, Some(30)).clause(),
            "status NOT IN ('resolved','closed') AND start_date >= DATE_SUB(NOW(), INTERVAL 30 DAY)"
        );
    }

    /// Including resolved and closed drops the status test
    #[test]
    fn with_closed() {
        assert_eq!(
            TicketScope::new(true, Some(7)).clause(),
            "start_date >= DATE_SUB(NOW(), INTERVAL 7 DAY)"
        );
    }

    /// All tickets, any time: a condition that's always true, so the queries stay valid OQL
    #[test]
    fn everything() {
        let all = TicketScope::new(true, None);
        assert_eq!(all.opened_within_days, None);
        assert_eq!(all.clause(), "id > 0");
        assert_eq!(
            OrgQueries::open_tickets(TicketClass::UserRequest, "5", &all).as_deref(),
            Some("SELECT UserRequest WHERE id > 0 AND org_id = 5")
        );
    }

    /// The scope reaches every list query
    #[test]
    fn reaches_queries() {
        for closed in [true, false] {
            let scope = TicketScope::new(closed, Some(7));
            for oql in [
                TicketQueries::mine(TicketClass::UserRequest, &me(), &scope),
                TicketQueries::team(TicketClass::UserRequest, &me(), &scope).unwrap(),
                OrgQueries::open_tickets(TicketClass::UserRequest, "5", &scope).unwrap(),
                OrgQueries::assigned_to(TicketClass::Incident, "4", &scope).unwrap(),
                OrgQueries::reported_by(TicketClass::Incident, "4", &scope).unwrap(),
            ] {
                assert!(oql.contains("INTERVAL 7 DAY"), "{oql}");
                assert_eq!(oql.contains("status NOT IN"), !closed, "{oql}");
            }
        }
    }

    /// My tickets keeps the unassigned-TTO-in-my-teams part with any scope
    #[test]
    fn mine_keeps_tto() {
        assert_eq!(
            TicketQueries::mine(
                TicketClass::UserRequest,
                &me(),
                &TicketScope::new(true, Some(7))
            ),
            "SELECT UserRequest WHERE start_date >= DATE_SUB(NOW(), INTERVAL 7 DAY) AND (agent_id = 2 OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (10)))"
        );
    }

    /// Resolved and closed tickets get their own groups, after the open ones
    #[test]
    fn closed_groups() {
        let groups = TicketList::groups(
            &[
                t(
                    "R-1",
                    "closed",
                    3,
                    "2026-11-01 09:00:00",
                    "2026-11-02 09:00:00",
                ),
                t(
                    "R-2",
                    "resolved",
                    3,
                    "2026-11-01 09:00:00",
                    "2026-11-02 09:00:00",
                ),
                t(
                    "R-3",
                    "pending",
                    3,
                    "2026-11-01 09:00:00",
                    "2026-11-02 09:00:00",
                ),
            ],
            &me(),
            TicketListView::Mine,
            TicketSort::Priority,
        );
        assert_eq!(
            groups_of(&groups),
            [
                TicketGroup::Pending,
                TicketGroup::Resolved,
                TicketGroup::Closed
            ]
        );
    }
}

mod ticket_sort_tests {
    use super::*;

    fn tickets() -> Vec<Ticket> {
        vec![
            t(
                "R-1",
                "assigned",
                3,
                "2026-11-01 09:00:00",
                "2026-11-09 09:00:00",
            ),
            t(
                "R-2",
                "assigned",
                1,
                "2026-11-05 09:00:00",
                "2026-11-06 09:00:00",
            ),
            t(
                "R-3",
                "assigned",
                3,
                "2026-10-20 09:00:00",
                "2026-11-10 09:00:00",
            ),
        ]
    }

    /// Each sort orders tickets inside a group
    #[test]
    fn orders() {
        let cases = [
            (TicketSort::Priority, ["R-2", "R-3", "R-1"]),
            (TicketSort::LastUpdate, ["R-3", "R-1", "R-2"]),
            (TicketSort::Newest, ["R-2", "R-1", "R-3"]),
            (TicketSort::Oldest, ["R-3", "R-1", "R-2"]),
        ];
        for (sort, expected) in cases {
            let groups = TicketList::groups(&tickets(), &me(), TicketListView::Mine, sort);
            assert_eq!(refs(groups.first()), expected, "{sort:?}");
        }
    }

    /// Sorting never changes the status grouping
    #[test]
    fn keeps_groups() {
        let mut mixed = tickets();
        mixed.push(t(
            "R-4",
            "escalated_ttr",
            4,
            "2026-11-10 08:00:00",
            "2026-11-10 08:00:00",
        ));
        assert_eq!(
            groups_of(&TicketList::groups(
                &mixed,
                &me(),
                TicketListView::Mine,
                TicketSort::LastUpdate
            )),
            [TicketGroup::EscalatedTtr, TicketGroup::Assigned]
        );
    }
}

mod ticket_paging_tests {
    use super::*;

    fn me() -> CurrentUser {
        CurrentUser {
            teams: vec![],
            ..super::me()
        }
    }

    fn tickets(ttr: usize, assigned: usize) -> Vec<Ticket> {
        let at = "2026-11-01 09:00:00";
        (0..ttr)
            .map(|i| t(&format!("R-{i:06}"), "escalated_ttr", 3, at, at))
            .chain((0..assigned).map(|i| t(&format!("R-{:06}", 100 + i), "assigned", 3, at, at)))
            .collect()
    }

    fn groups() -> Vec<TicketSection> {
        TicketList::groups(
            &tickets(3, 30),
            &me(),
            TicketListView::Mine,
            TicketSort::Priority,
        )
    }

    /// Page 1 takes the first tickets in group order, so the most urgent come first
    #[test]
    fn first_page() {
        let page = TicketPage::new(groups(), Some(10), 1);
        assert_eq!(
            groups_of(&page.sections),
            [TicketGroup::EscalatedTtr, TicketGroup::Assigned]
        );
        assert_eq!(
            page.sections
                .iter()
                .map(|s| s.tickets.len())
                .collect::<Vec<_>>(),
            [3, 7]
        );
        assert_eq!(page.total, 33);
        assert_eq!(page.page_count, 4);
        assert_eq!(page.label(), "1–10 of 33");
        assert_eq!(
            page.group_total(TicketGroup::Assigned),
            30,
            "headers count the whole group, not just this page"
        );
    }

    /// A later page keeps the group headers of what it shows
    #[test]
    fn later_page() {
        let page = TicketPage::new(groups(), Some(10), 4);
        assert_eq!(groups_of(&page.sections), [TicketGroup::Assigned]);
        assert_eq!(page.sections.first().map(|s| s.tickets.len()), Some(3));
        assert_eq!(page.label(), "31–33 of 33");
    }

    /// Pages out of range are clamped, and All shows everything on one page
    #[test]
    fn clamp_and_all() {
        assert_eq!(TicketPage::new(groups(), Some(10), 99).page, 4);
        assert_eq!(TicketPage::new(groups(), Some(10), 0).page, 1);
        let all = TicketPage::new(groups(), None, 3);
        assert!(
            all.page == 1
                && all.page_count == 1
                && all.sections.iter().map(|s| s.tickets.len()).sum::<usize>() == 33
        );
    }

    /// No tickets: one empty page
    #[test]
    fn empty() {
        let page = TicketPage::new(vec![], Some(20), 1);
        assert!(page.page_count == 1 && page.total == 0 && page.sections.is_empty());
    }
}
