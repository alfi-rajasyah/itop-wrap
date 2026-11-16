//! Port of Tests/ITopKitTests/TicketQueryTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.

use chrono::{DateTime, Duration, Utc};
use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    Age, Credentials, CurrentUser, ITopClient, ServerAddress, Team, Ticket, TicketClass,
    TicketGroup, TicketList, TicketListView, TicketQueries, TicketScope, TicketSearch,
    TicketService, TicketSort,
};

fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1001".into(),
        contact_id: "1002".into(),
        login: "agent".into(),
        display_name: "Agent One".into(),
        profiles: vec![],
        teams: vec![
            Team::new("1009", "Network"),
            Team::new("1010", "Service desk"),
        ],
    }
}

fn loner() -> CurrentUser {
    CurrentUser {
        user_id: "1".into(),
        contact_id: "2".into(),
        login: "solo".into(),
        display_name: "Solo".into(),
        profiles: vec![],
        teams: vec![],
    }
}

/// The Swift helper `ticket(_:status:agent:team:priority:started:title:)`.
fn ticket(
    r: &str,
    status: &str,
    agent: &str,
    team: &str,
    priority: i64,
    started: &str,
    title: &str,
) -> Ticket {
    Ticket {
        key: r[r.len() - 3..].to_string(),
        ticket_class: if r.starts_with('I') {
            TicketClass::Incident
        } else {
            TicketClass::UserRequest
        },
        r#ref: r.into(),
        title: title.into(),
        status: status.into(),
        priority,
        caller_name: "Caller".into(),
        org_name: "Org A".into(),
        org_id: "0".into(),
        team_id: team.into(),
        team_name: "Network".into(),
        agent_id: agent.into(),
        agent_name: if agent == "0" { "" } else { "Agent" }.into(),
        start_date: Ticket::parse_date(started),
        last_update: None,
    }
}

/// `ticket(ref, status:)` with the Swift defaults.
fn t(r: &str, status: &str) -> Ticket {
    ticket(
        r,
        status,
        "1002",
        "1009",
        3,
        "2026-11-10 09:00:00",
        "A ticket",
    )
}

fn client(stub: &StubServer) -> ITopClient {
    ITopClient::new(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("agent", "pw"),
        stub.transport(),
    )
}

// MARK: - OQL

mod ticket_query_oql_tests {
    use super::*;

    /// My tickets: open tickets assigned to me, plus unassigned Escalated TTO in my teams
    #[test]
    fn mine() {
        assert_eq!(
            TicketQueries::mine(TicketClass::UserRequest, &me(), &TicketScope::OPEN),
            "SELECT UserRequest WHERE status NOT IN ('resolved','closed') AND (agent_id = 1002 OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (1009,1010)))"
        );
    }

    /// Without teams, My tickets is only what's assigned to me
    #[test]
    fn mine_no_teams() {
        assert_eq!(
            TicketQueries::mine(TicketClass::Incident, &loner(), &TicketScope::OPEN),
            "SELECT Incident WHERE status NOT IN ('resolved','closed') AND agent_id = 2"
        );
    }

    /// Team tickets: everything open in my teams
    #[test]
    fn team() {
        assert_eq!(
            TicketQueries::team(TicketClass::Incident, &me(), &TicketScope::OPEN).as_deref(),
            Some(
                "SELECT Incident WHERE status NOT IN ('resolved','closed') AND team_id IN (1009,1010)"
            )
        );
    }

    /// No teams means no team query at all
    #[test]
    fn team_none() {
        assert_eq!(
            TicketQueries::team(TicketClass::UserRequest, &loner(), &TicketScope::OPEN),
            None
        );
    }

    /// A full ref is looked up in the right class
    #[test]
    fn ref_lookup() {
        let cases = [
            ("R-000512", TicketClass::UserRequest, "R-000512"),
            ("i-000088", TicketClass::Incident, "I-000088"),
            ("  R-000512 ", TicketClass::UserRequest, "R-000512"),
        ];
        for (input, cls, r) in cases {
            let lookup = TicketQueries::ref_lookup(input);
            assert_eq!(lookup.as_ref().map(|l| l.0), Some(cls), "{input:?}");
            assert_eq!(
                lookup.map(|l| l.1),
                Some(format!("SELECT {} WHERE ref = '{r}'", cls.raw_value())),
                "{input:?}"
            );
        }
    }

    /// Partial refs and words aren't looked up
    #[test]
    fn not_a_ref() {
        for input in ["412", "R-412", "VPN", "R-0005123", "X-000512", ""] {
            assert_eq!(TicketQueries::ref_lookup(input), None, "{input:?}");
        }
    }
}

// MARK: - Decoding

mod ticket_decoding_tests {
    use super::*;

    /// The real ticket sample decodes
    #[tokio::test]
    async fn fixture() {
        let stub = StubServer::new(StubReply::fixture("userrequest"));
        let tickets = TicketService::new(client(&stub))
            .load_mine(&me(), &TicketScope::OPEN)
            .await
            .unwrap();

        let t = tickets.first().expect("a ticket");
        assert_eq!(t.status, "assigned");
        assert_eq!(t.priority, 4);
        assert_eq!(t.agent_id, "1002");
        assert_eq!(t.team_id, "1009");
        assert!(!t.is_unassigned());
        assert_eq!(t.start_date, Ticket::parse_date("2026-11-09 11:00:32"));
    }

    /// Both classes are loaded and combined, and the request asks only for the fields the list shows
    #[tokio::test]
    async fn both_classes() {
        let stub = StubServer::with_handler(|request| {
            let cls = request.class().unwrap_or_default();
            let r = if cls == "Incident" {
                "I-000088"
            } else {
                "R-000412"
            };
            StubReply::json(&format!(
                r#"{{"code":0,"message":"","objects":{{"{cls}::7":{{"class":"{cls}","key":"7","code":0,"message":"","fields":{{"ref":"{r}","title":"T","status":"assigned","priority":"2","agent_id":"1002","team_id":"1009","start_date":"2026-11-10 08:00:00"}}}}}}}}"#
            ))
        });

        let tickets = TicketService::new(client(&stub))
            .load_mine(&me(), &TicketScope::OPEN)
            .await
            .unwrap();

        let refs: std::collections::HashSet<String> =
            tickets.iter().map(|t| t.r#ref.clone()).collect();
        assert_eq!(
            refs,
            ["R-000412".to_string(), "I-000088".to_string()].into()
        );
        let fields = stub.requests()[0].json_data()["output_fields"]
            .as_str()
            .unwrap_or("")
            .to_string();
        assert!(
            fields.contains("ref")
                && fields.contains("agent_id_friendlyname")
                && !fields.contains('*')
        );
    }

    /// Looking up a ref that iTop doesn't return gives nothing, not an error
    #[tokio::test]
    async fn lookup_missing() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"Found: 0","objects":null}"#,
        ));
        assert_eq!(
            TicketService::new(client(&stub))
                .lookup("R-000999")
                .await
                .unwrap(),
            None
        );
    }
}

// MARK: - Grouping and the "needs you" rule

mod ticket_grouping_tests {
    use super::*;

    /// Groups come escalated first, and empty groups are left out
    #[test]
    fn order() {
        let groups = TicketList::groups(
            &[
                t("R-000001", "pending"),
                t("R-000002", "assigned"),
                t("R-000003", "escalated_ttr"),
            ],
            &me(),
            TicketListView::Mine,
            TicketSort::Priority,
        );
        assert_eq!(
            groups.iter().map(|g| g.group).collect::<Vec<_>>(),
            [
                TicketGroup::EscalatedTtr,
                TicketGroup::Assigned,
                TicketGroup::Pending
            ]
        );
    }

    /// Team tickets also have a New group for unassigned tickets that haven't escalated
    #[test]
    fn team_new() {
        let groups = TicketList::groups(
            &[
                ticket(
                    "R-000004",
                    "new",
                    "0",
                    "1009",
                    3,
                    "2026-11-10 09:00:00",
                    "A ticket",
                ),
                t("R-000005", "assigned"),
            ],
            &me(),
            TicketListView::Team,
            TicketSort::Priority,
        );
        assert_eq!(
            groups.iter().map(|g| g.group).collect::<Vec<_>>(),
            [TicketGroup::New, TicketGroup::Assigned]
        );
    }

    /// Needs you: my Escalated TTR, and an unassigned Escalated TTO in my teams. Nothing else.
    #[test]
    fn needs_you() {
        let cases = [
            ("escalated_ttr", "1002", "1009", true), // my TTR
            ("escalated_ttr", "77", "1009", false),  // someone else's TTR
            ("escalated_tto", "0", "1009", true),    // unowned TTO in my team
            ("escalated_tto", "0", "555", false),    // unowned TTO in another team
            ("assigned", "1002", "1009", false),     // mine but not escalated
            ("new", "0", "1009", false),             // unassigned, not escalated
        ];
        for (status, agent, team, expected) in cases {
            let tk = ticket(
                "R-000010",
                status,
                agent,
                team,
                3,
                "2026-11-10 09:00:00",
                "A ticket",
            );
            assert_eq!(
                TicketList::needs_you(&tk, &me()),
                expected,
                "{status} {agent} {team}"
            );
        }
    }

    /// Within a group: higher priority first, then the oldest
    #[test]
    fn sort_within_group() {
        let groups = TicketList::groups(
            &[
                ticket(
                    "R-000020",
                    "assigned",
                    "1002",
                    "1009",
                    3,
                    "2026-11-01 09:00:00",
                    "A ticket",
                ),
                ticket(
                    "R-000021",
                    "assigned",
                    "1002",
                    "1009",
                    1,
                    "2026-11-09 09:00:00",
                    "A ticket",
                ),
                ticket(
                    "R-000022",
                    "assigned",
                    "1002",
                    "1009",
                    3,
                    "2026-10-01 09:00:00",
                    "A ticket",
                ),
            ],
            &me(),
            TicketListView::Mine,
            TicketSort::Priority,
        );
        assert_eq!(
            groups[0]
                .tickets
                .iter()
                .map(|t| t.r#ref.as_str())
                .collect::<Vec<_>>(),
            ["R-000021", "R-000022", "R-000020"]
        );
    }

    /// Approval states and anything unknown go to Other, so nothing is dropped
    #[test]
    fn other() {
        let groups = TicketList::groups(
            &[
                t("R-000030", "waiting_for_approval"),
                t("R-000031", "something_custom"),
            ],
            &me(),
            TicketListView::Mine,
            TicketSort::Priority,
        );
        assert_eq!(
            groups.iter().map(|g| g.group).collect::<Vec<_>>(),
            [TicketGroup::Other]
        );
        assert_eq!(groups.first().map(|g| g.tickets.len()), Some(2));
    }
}

// MARK: - Search and age

mod search_and_age_tests {
    use super::*;

    fn tickets() -> Vec<Ticket> {
        vec![
            ticket(
                "R-000412",
                "assigned",
                "1002",
                "1009",
                3,
                "2026-11-10 09:00:00",
                "VPN drops every hour",
            ),
            ticket(
                "I-000088",
                "escalated_tto",
                "0",
                "1009",
                3,
                "2026-11-10 09:00:00",
                "Mail delivery delayed",
            ),
        ]
    }

    /// Search matches ref digits, full refs and title words, ignoring case
    #[test]
    fn search() {
        let cases: [(&str, &[&str]); 6] = [
            ("412", &["R-000412"]),
            ("r-000412", &["R-000412"]),
            ("vpn", &["R-000412"]),
            ("delivery mail", &["I-000088"]),
            ("", &["R-000412", "I-000088"]),
            ("printer", &[]),
        ];
        for (query, expected) in cases {
            let found = TicketSearch::filter_query(&tickets(), query);
            assert_eq!(
                found.iter().map(|t| t.r#ref.as_str()).collect::<Vec<_>>(),
                expected,
                "{query:?}"
            );
        }
    }

    /// Age reads like the wireframe: minutes, hours, days
    #[test]
    fn age() {
        let cases = [
            (30, "now"),
            (2400, "40m"),
            (7200, "2h"),
            (108_000, "1d"),
            (777_600, "9d"),
        ];
        let now: DateTime<Utc> = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        for (seconds, expected) in cases {
            assert_eq!(
                Age::short(now - Duration::seconds(seconds), now),
                expected,
                "{seconds}"
            );
        }
    }
}
