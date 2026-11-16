//! Port of Tests/ITopKitTests/CMDBQueryTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.

use itop_kit::stub::{Fixture, StubReply, StubServer};
use itop_kit::{
    CIRef, CMDBQueries, CMDBService, ConfigItemDetail, Credentials, Direction, ITopClient,
    ITopResponse, JsonObject, JsonValue, RestOperation, ServerAddress, TicketClass,
};

fn client(stub: &StubServer) -> ITopClient {
    ITopClient::new(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("agent", "pw"),
        stub.transport(),
    )
}

fn json(text: &str) -> JsonValue {
    JsonValue::parse_str(text).unwrap()
}

// MARK: - Search

mod cmdb_search_tests {
    use super::*;

    /// Search matches names that contain the words, safely quoted
    #[test]
    fn oql() {
        assert_eq!(
            CMDBQueries::search("vpn").as_deref(),
            Some("SELECT FunctionalCI WHERE name LIKE '%vpn%'")
        );
        assert_eq!(
            CMDBQueries::search("  o'neil ").as_deref(),
            Some(r"SELECT FunctionalCI WHERE name LIKE '%o\'neil%'")
        );
    }

    /// Fewer than 2 characters doesn't search
    #[test]
    fn too_short() {
        for q in ["", " ", "v"] {
            assert_eq!(CMDBQueries::search(q), None, "{q:?}");
        }
    }

    /// Results show their real class and come sorted by name, with a cap on how many are asked for
    #[tokio::test]
    async fn decode() {
        let stub = StubServer::new(StubReply::json(
            r#"
        {"code":0,"message":"","objects":{
          "FunctionalCI::2":{"class":"FunctionalCI","key":"2","code":0,"message":"","fields":{"name":"vpn-gw-02","finalclass":"NetworkDevice","org_id_friendlyname":"Org A"}},
          "FunctionalCI::1":{"class":"FunctionalCI","key":"1","code":0,"message":"","fields":{"name":"Remote access VPN","finalclass":"ApplicationSolution","org_id_friendlyname":"Org A"}}}}
        "#,
        ));

        let items = CMDBService::new(client(&stub)).search("vpn").await.unwrap();

        assert_eq!(
            items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["Remote access VPN", "vpn-gw-02"]
        );
        assert_eq!(
            items.first().map(|i| i.class_name.as_str()),
            Some("ApplicationSolution")
        );
        assert_eq!(
            items.first().map(|i| i.class_label()).as_deref(),
            Some("Application solution")
        );
        let request = stub.requests()[0].json_data();
        assert_eq!(
            request["limit"].as_str(),
            Some(CMDBQueries::SEARCH_LIMIT.to_string().as_str())
        );
    }
}

// MARK: - Detail fields

mod ci_detail_tests {
    use super::*;

    /// The real Server sample gives its organization and criticality, and skips empty fields
    #[test]
    fn fields() {
        let data = Fixture::data("ci").unwrap();
        let objects = ITopResponse::new(JsonValue::parse(&data).unwrap()).objects();
        let object = objects.first().expect("an object");

        let fields = ConfigItemDetail::fields(&object.fields);
        let labels: Vec<&str> = fields.iter().map(|f| f.label.as_str()).collect();

        assert!(labels.contains(&"Organization"));
        assert!(labels.contains(&"Business criticality"));
        assert!(
            !labels.contains(&"In production since"),
            "move2production is empty in the sample"
        );
    }

    /// Codes read as words
    #[test]
    fn labels() {
        let cases = [
            ("production", "Production"),
            ("implementation", "Implementation"),
            ("high", "High"),
            ("not_applicable", "Not applicable"),
        ];
        for (code, label) in cases {
            assert_eq!(ConfigItemDetail::humanize(code), label);
        }
    }

    /// Contacts come from contacts_list
    #[test]
    fn contacts() {
        let mut f = JsonObject::new();
        f.insert(
            "contacts_list".into(),
            json(r#"[{"contact_id_friendlyname":"Team Network"},{"contact_id_friendlyname":"Agent 4"}]"#),
        );
        f.insert("status".into(), JsonValue::from("production"));
        let fields = ConfigItemDetail::fields(&f);
        let value = |label: &str| {
            fields
                .iter()
                .find(|x| x.label == label)
                .map(|x| x.value.clone())
        };
        assert_eq!(value("Contacts").as_deref(), Some("Team Network, Agent 4"));
        assert_eq!(value("Status").as_deref(), Some("Production"));
    }
}

// MARK: - Relationships

mod ci_relation_tests {
    use super::*;

    /// core/get_related asks for direct impacts in the given direction
    #[tokio::test]
    async fn operation() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{},"relations":{}}"#,
        ));
        CMDBService::new(client(&stub))
            .related("Server", "5", Direction::Up)
            .await
            .unwrap();

        let r = stub.requests()[0].json_data();
        assert_eq!(r["operation"].as_str(), Some("core/get_related"));
        assert_eq!(r["relation"].as_str(), Some("impacts"));
        assert_eq!(r["direction"].as_str(), Some("up"));
        assert_eq!(r["depth"].as_i64(), Some(1));
        assert!(
            !RestOperation::GetRelated {
                class_name: "Server".into(),
                key: "5".into(),
                relation: "impacts".into(),
                depth: 1,
                direction: "up".into(),
            }
            .is_write()
        );
    }

    /// Related objects exclude the CI itself and anything that isn't a CI, sorted by name
    #[test]
    fn parse() {
        let body = json(
            r#"
        {"code":0,"message":"","objects":{
          "Server::5":{"class":"Server","key":"5","fields":{"friendlyname":"srv-01"}},
          "ApplicationSolution::9":{"class":"ApplicationSolution","key":"9","fields":{"friendlyname":"Remote access VPN"}},
          "VirtualMachine::3":{"class":"VirtualMachine","key":"3","fields":{"name":"app-vm-03"}},
          "Person::2":{"class":"Person","key":"2","fields":{"friendlyname":"Agent 4"}}},
         "relations":{"Server::5":[{"key":"ApplicationSolution::9"},{"key":"VirtualMachine::3"}]}}
        "#,
        );

        let refs = CMDBService::related_items(&ITopResponse::new(body), "Server", "5");

        assert_eq!(
            refs,
            [
                CIRef::new("3", "app-vm-03", "VirtualMachine"),
                CIRef::new("9", "Remote access VPN", "ApplicationSolution"),
            ]
        );
    }
}

// MARK: - Open tickets on a CI

mod ci_ticket_tests {
    use super::*;

    /// Open tickets come through the ticket-to-CI link table, one query per ticket class
    #[test]
    fn oql() {
        assert_eq!(
            CMDBQueries::open_tickets(TicketClass::UserRequest, "55").as_deref(),
            Some(
                "SELECT UserRequest AS t JOIN lnkFunctionalCIToTicket AS l ON l.ticket_id = t.id WHERE l.functionalci_id = 55 AND t.status NOT IN ('resolved','closed')"
            )
        );
    }

    /// A non-numeric id never reaches the query
    #[test]
    fn unsafe_id() {
        assert_eq!(
            CMDBQueries::open_tickets(TicketClass::Incident, "5 OR 1=1"),
            None
        );
    }
}
