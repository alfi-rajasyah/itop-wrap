//! Port of Tests/ITopKitTests/OrgQueryTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.

use std::collections::HashSet;

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    CMDBService, Credentials, ITopClient, ITopObject, JsonValue, OrgQueries, OrgService, Person,
    ServerAddress, Team, Ticket, TicketClass, TicketScope, TicketSearch,
};

fn client(stub: &StubServer) -> ITopClient {
    ITopClient::new(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("agent", "pw"),
        stub.transport(),
    )
}

/// The Swift helper `objects(_:_:)`: rows of string fields as an iTop answer.
fn objects(cls: &str, rows: &[(&str, &[(&str, &str)])]) -> StubReply {
    let body = rows
        .iter()
        .map(|(key, fields)| {
            let f = fields
                .iter()
                .map(|(k, v)| format!(r#""{k}":"{v}""#))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#""{cls}::{key}":{{"class":"{cls}","key":"{key}","code":0,"message":"","fields":{{{f}}}}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    StubReply::json(&format!(
        r#"{{"code":0,"message":"","objects":{{{body}}}}}"#
    ))
}

// MARK: - Organizations

mod organization_tests {
    use super::*;

    /// OQL for the list, an org's open tickets, and its people and CIs
    #[test]
    fn oql() {
        assert_eq!(
            OrgQueries::ORGANIZATIONS,
            "SELECT Organization WHERE status = 'active'"
        );
        assert_eq!(
            OrgQueries::open_tickets(TicketClass::Incident, "7", &TicketScope::OPEN).as_deref(),
            Some("SELECT Incident WHERE status NOT IN ('resolved','closed') AND org_id = 7")
        );
        assert_eq!(
            OrgQueries::people("7").as_deref(),
            Some("SELECT Person WHERE org_id = 7")
        );
        assert_eq!(
            OrgQueries::config_items("7").as_deref(),
            Some("SELECT FunctionalCI WHERE org_id = 7")
        );
        assert_eq!(
            OrgQueries::open_tickets(TicketClass::UserRequest, "7 OR 1=1", &TicketScope::OPEN),
            None
        );
    }

    /// Organizations come with their open-ticket counts, sorted by name; counts only cover orgs iTop listed
    #[tokio::test]
    async fn counts() {
        let stub = StubServer::with_handler(|request| match request.class().as_deref() {
            Some("Organization") => objects(
                "Organization",
                &[
                    ("2", &[("name", "Org B"), ("code", "B")]),
                    ("1", &[("name", "Org A"), ("code", "A")]),
                ],
            ),
            Some("UserRequest") => objects(
                "UserRequest",
                &[
                    ("10", &[("org_id", "1")]),
                    ("11", &[("org_id", "1")]),
                    ("12", &[("org_id", "2")]),
                ],
            ),
            Some("Incident") => objects(
                "Incident",
                &[("20", &[("org_id", "1")]), ("21", &[("org_id", "99")])],
            ),
            _ => StubReply::json(r#"{"code":11,"message":"x"}"#),
        });

        let orgs = OrgService::new(client(&stub))
            .organizations()
            .await
            .unwrap();

        assert_eq!(
            orgs.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            ["Org A", "Org B"]
        );
        assert_eq!(
            orgs.iter().map(|o| o.open_count).collect::<Vec<_>>(),
            [3, 1]
        );
        let ticket_fields = stub
            .requests()
            .iter()
            .find(|r| r.class().as_deref() == Some("UserRequest"))
            .and_then(|r| r.json_data()["output_fields"].as_str().map(str::to_string));
        assert_eq!(
            ticket_fields.as_deref(),
            Some("org_id"),
            "counting asks for nothing but the org"
        );
    }
}

// MARK: - People

mod people_tests {
    use super::*;

    /// Each word has to match the first or last name
    #[test]
    fn search_oql() {
        assert_eq!(
            OrgQueries::people_search("Jane doe").as_deref(),
            Some(
                "SELECT Person WHERE (name LIKE '%Jane%' OR first_name LIKE '%Jane%') AND (name LIKE '%doe%' OR first_name LIKE '%doe%')"
            )
        );
        assert_eq!(
            OrgQueries::people_search("o'neil").as_deref(),
            Some(r"SELECT Person WHERE (name LIKE '%o\'neil%' OR first_name LIKE '%o\'neil%')")
        );
    }

    /// Fewer than 2 characters doesn't search
    #[test]
    fn too_short() {
        for q in ["", "j", "  "] {
            assert_eq!(OrgQueries::people_search(q), None, "{q:?}");
        }
    }

    /// Your real Person sample decodes: teams make an agent, and the phone stays cleaned
    #[tokio::test]
    async fn fixture() {
        let stub = StubServer::new(StubReply::fixture("person"));
        let people = OrgService::new(client(&stub))
            .people_matching("agent")
            .await
            .unwrap();

        let p = people.first().expect("a person");
        assert_eq!(p.id, "1002");
        assert!(p.is_agent());
        assert_eq!(p.teams.len(), 1);
        assert_eq!(p.display_name, "[text] [text]");
        assert_eq!(p.phone, "[number]");
    }

    /// Someone in no team is a caller
    #[test]
    fn caller() {
        let p = Person {
            id: "3".into(),
            display_name: "Caller One".into(),
            org_id: "1".into(),
            org_name: "Org A".into(),
            email: String::new(),
            phone: String::new(),
            function: String::new(),
            teams: vec![],
        };
        assert!(!p.is_agent());
        assert_eq!(p.subtitle(), "Caller · Org A");
        assert_eq!(p.initials(), "CO");
    }

    /// An agent's subtitle names their teams
    #[test]
    fn agent_subtitle() {
        let p = Person {
            id: "4".into(),
            display_name: "Agent Four".into(),
            org_id: "1".into(),
            org_name: "Org A".into(),
            email: String::new(),
            phone: String::new(),
            function: String::new(),
            teams: vec![Team::new("1", "Network"), Team::new("2", "Service desk")],
        };
        assert_eq!(p.subtitle(), "Agent · Network, Service desk");
    }

    /// Tickets assigned to someone and tickets they reported
    #[test]
    fn person_tickets() {
        assert_eq!(
            OrgQueries::assigned_to(TicketClass::UserRequest, "4", &TicketScope::OPEN).as_deref(),
            Some("SELECT UserRequest WHERE status NOT IN ('resolved','closed') AND agent_id = 4")
        );
        assert_eq!(
            OrgQueries::reported_by(TicketClass::Incident, "4", &TicketScope::OPEN).as_deref(),
            Some("SELECT Incident WHERE status NOT IN ('resolved','closed') AND caller_id = 4")
        );
    }
}

// MARK: - Org filter on My tickets

mod org_filter_tests {
    use super::*;

    fn t(r: &str, org: &str) -> Ticket {
        Ticket {
            key: r.into(),
            ticket_class: TicketClass::UserRequest,
            r#ref: r.into(),
            title: "t".into(),
            status: "assigned".into(),
            priority: 3,
            caller_name: String::new(),
            org_name: org.into(),
            org_id: "0".into(),
            team_id: "0".into(),
            team_name: String::new(),
            agent_id: "2".into(),
            agent_name: String::new(),
            start_date: None,
            last_update: None,
        }
    }

    fn refs(tickets: &[Ticket]) -> Vec<&str> {
        tickets.iter().map(|t| t.r#ref.as_str()).collect()
    }

    /// No orgs chosen means everything; otherwise only the chosen orgs
    #[test]
    fn filter() {
        let tickets = [t("R-1", "Org A"), t("R-2", "Org B"), t("R-3", "Org C")];
        assert_eq!(
            refs(&TicketSearch::filter_orgs(&tickets, &HashSet::new())),
            ["R-1", "R-2", "R-3"]
        );
        let chosen: HashSet<String> = ["Org A".to_string(), "Org C".to_string()].into();
        assert_eq!(
            refs(&TicketSearch::filter_orgs(&tickets, &chosen)),
            ["R-1", "R-3"]
        );
    }

    /// The menu offers the orgs your tickets are for, sorted
    #[test]
    fn choices() {
        assert_eq!(
            TicketSearch::orgs(&[
                t("R-1", "Org B"),
                t("R-2", "Org A"),
                t("R-3", "Org B"),
                t("R-4", "")
            ]),
            ["Org A", "Org B"]
        );
    }

    /// Tickets now carry their org id, for opening the org view
    #[test]
    fn org_id() {
        let json = JsonValue::parse_str(
            r#"{"class":"UserRequest","key":"5","fields":{"ref":"R-5","org_id":"14","org_id_friendlyname":"Org A"}}"#,
        )
        .unwrap();
        let object = ITopObject::from_json(&json).expect("an object");
        assert_eq!(
            Ticket::from_object(&object).map(|t| t.org_id),
            Some("14".to_string())
        );
        assert!(Ticket::LIST_FIELDS.contains(&"org_id"));
    }
}

// MARK: - CIs of an org

mod cmdb_by_org_tests {
    use super::*;

    /// An org's CIs use the same result shape as search
    #[tokio::test]
    async fn in_org() {
        let stub = StubServer::new(objects(
            "FunctionalCI",
            &[(
                "1",
                &[
                    ("name", "srv-01"),
                    ("finalclass", "Server"),
                    ("org_id_friendlyname", "Org A"),
                ],
            )],
        ));
        let items = CMDBService::new(client(&stub))
            .items_in_org("7")
            .await
            .unwrap();
        assert_eq!(
            items
                .iter()
                .map(|i| i.class_name.as_str())
                .collect::<Vec<_>>(),
            ["Server"]
        );
        assert_eq!(
            stub.requests()[0].json_data()["key"].as_str(),
            Some("SELECT FunctionalCI WHERE org_id = 7")
        );
    }
}
