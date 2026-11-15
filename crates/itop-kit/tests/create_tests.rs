//! Port of Tests/ITopKitTests/CreateTests.swift. Parametrised Swift tests loop over the same
//! arguments.

use std::collections::HashSet;

use itop_kit::stub::{RecordedRequest, StubReply, StubServer};
use itop_kit::ticket_writes::Failure;
use itop_kit::{
    Credentials, ITopClient, ITopError, NewTicket, PickLists, ServerAddress, TicketClass,
    TicketWrites, WriteError,
};

fn client(
    stub: &StubServer,
    writes: bool,
    allow: Option<&[&str]>,
    new_tickets: bool,
) -> ITopClient {
    ITopClient::with_writes(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
        writes,
        allow.map(|keys| keys.iter().map(|k| k.to_string()).collect::<HashSet<_>>()),
        new_tickets,
    )
}

fn writing(stub: &StubServer) -> ITopClient {
    client(stub, true, None, false)
}

fn draft(cls: TicketClass) -> NewTicket {
    NewTicket {
        org_id: "4".into(),
        caller_id: "31".into(),
        service_id: "3".into(),
        subcategory_id: "12".into(),
        impact: "3".into(),
        urgency: "2".into(),
        title: "  VPN drops every hour  ".into(),
        description: "Since Monday.\nBoth office & home <laptop>.".into(),
        ..NewTicket::new(cls)
    }
}

fn created_reply() -> StubReply {
    StubReply::json(
        r#"{"code":0,"message":"","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"created","fields":{"id":"900"}}}}"#,
    )
}

fn read_back() -> StubReply {
    StubReply::json(
        r#"{"code":0,"message":"Found: 1","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"","fields":{"ref":"R-000900","title":"VPN drops every hour","status":"new","org_id":"4","org_id_friendlyname":"Org A","caller_id_friendlyname":"Caller 1","agent_id":"0","team_id":"0","priority":"3"}}}}"#,
    )
}

fn op(r: &RecordedRequest) -> Option<String> {
    r.operation()
}

fn stub() -> StubServer {
    StubServer::with_handler(|r| {
        if op(r).as_deref() == Some("core/create") {
            created_reply()
        } else {
            read_back()
        }
    })
}

mod create_tests {
    use super::*;

    /// core/create sends every field, with the description as HTML that keeps line breaks and escapes markup
    #[tokio::test]
    async fn create() {
        let s = stub();
        let ticket = TicketWrites::new(writing(&s))
            .create(&draft(TicketClass::UserRequest))
            .await
            .unwrap();

        let r = s.requests()[0].json_data();
        assert_eq!(r["operation"], "core/create");
        assert_eq!(r["class"], "UserRequest");
        assert_eq!(r["comment"], TicketWrites::COMMENT);
        let f = r["fields"].as_object().unwrap();
        assert_eq!(f["org_id"], "4");
        assert_eq!(f["caller_id"], "31");
        assert_eq!(f["service_id"], "3");
        assert_eq!(f["servicesubcategory_id"], "12");
        assert_eq!(f["impact"], "3");
        assert_eq!(f["urgency"], "2");
        assert_eq!(f["title"], "VPN drops every hour");
        assert_eq!(
            f["description"],
            "<p>Since Monday.<br>Both office &amp; home &lt;laptop&gt;.</p>"
        );
        assert!(f.get("functionalcis_list").is_none());
        assert_eq!(
            ticket.r#ref, "R-000900",
            "the new ticket is read back so it can be opened"
        );
        assert_eq!(ticket.key, "900");
    }

    /// Created from a CI, the ticket is linked to it
    #[tokio::test]
    async fn linked_ci() {
        let s = stub();
        let mut d = draft(TicketClass::Incident);
        d.ci_id = Some("55".into());
        TicketWrites::new(writing(&s)).create(&d).await.unwrap();
        let r = s.requests()[0].json_data();
        assert_eq!(r["class"], "Incident");
        assert_eq!(
            r["fields"]["functionalcis_list"][0]["functionalci_id"],
            "55"
        );
    }

    /// Required fields are checked before anything is sent
    #[tokio::test]
    async fn required() {
        for name in [
            "organization",
            "caller",
            "service",
            "subcategory",
            "title",
            "description",
        ] {
            let s = stub();
            let mut d = draft(TicketClass::UserRequest);
            match name {
                "organization" => d.org_id = "  ".into(),
                "caller" => d.caller_id = "  ".into(),
                "service" => d.service_id = "  ".into(),
                "subcategory" => d.subcategory_id = "  ".into(),
                "title" => d.title = "  ".into(),
                _ => d.description = "  ".into(),
            }
            assert_eq!(
                TicketWrites::new(writing(&s)).create(&d).await,
                Err(WriteError::Failure(Failure::Missing(name.into()))),
                "{name}"
            );
            assert!(s.requests().is_empty());
        }
    }

    /// Ids that aren't numbers never reach iTop
    #[tokio::test]
    async fn bad_ids() {
        let s = stub();
        let mut d = draft(TicketClass::UserRequest);
        d.caller_id = "31 OR 1=1".into();
        assert_eq!(
            TicketWrites::new(writing(&s)).create(&d).await,
            Err(WriteError::Failure(Failure::Missing("caller".into())))
        );
        assert!(s.requests().is_empty());
    }

    /// With a per-ticket allowlist, new tickets are refused unless creating tickets is switched on too
    #[tokio::test]
    async fn allowlist() {
        let refused = stub();
        assert_eq!(
            TicketWrites::new(client(&refused, true, Some(&["UserRequest::7"]), false))
                .create(&draft(TicketClass::UserRequest))
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(refused.requests().is_empty());

        let allowed = stub();
        TicketWrites::new(client(&allowed, true, Some(&["UserRequest::7"]), true))
            .create(&draft(TicketClass::UserRequest))
            .await
            .unwrap();
        assert_eq!(
            allowed.requests().first().and_then(op).as_deref(),
            Some("core/create")
        );
    }

    /// A ticket created under the new-ticket switch joins the allowlist, so it can be worked on; others stay closed
    #[tokio::test]
    async fn created_joins_allowlist() {
        let s = StubServer::with_handler(|r| match op(r).as_deref() {
            Some("core/create") => created_reply(),
            Some("core/get") => read_back(),
            _ => StubReply::json(
                r#"{"code":0,"message":"","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"","fields":{"id":"900"}}}}"#,
            ),
        });
        let c = client(&s, true, Some(&[]), true);
        assert!(!c.can_write("UserRequest", "900"));
        let ticket = TicketWrites::new(c.clone())
            .create(&draft(TicketClass::UserRequest))
            .await
            .unwrap();

        assert!(c.can_write("UserRequest", "900"));
        TicketWrites::new(c.clone())
            .add_log_entry(&ticket, "Test entry", true)
            .await
            .unwrap();
        assert_eq!(
            s.requests().last().and_then(op).as_deref(),
            Some("core/update")
        );
        assert!(!c.can_write("UserRequest", "901"));
    }

    /// Creating tickets never works with writes off, whatever else is set
    #[tokio::test]
    async fn writes_off() {
        let s = stub();
        assert_eq!(
            TicketWrites::new(client(&s, false, None, true))
                .create(&draft(TicketClass::UserRequest))
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(s.requests().is_empty());
    }

    /// iTop refusing the ticket is an error with its message
    #[tokio::test]
    async fn refused() {
        let s = StubServer::new(StubReply::json(
            r#"{"code":100,"message":"Error: servicesubcategory_id: not allowed for this request type","objects":null}"#,
        ));
        let result = TicketWrites::new(writing(&s))
            .create(&draft(TicketClass::UserRequest))
            .await;
        assert!(matches!(result, Err(WriteError::ITop(_))), "{result:?}");
    }
}

mod pick_list_tests {
    use super::*;

    /// OQL: active organizations, and active people in one organization
    #[test]
    fn oql() {
        assert_eq!(
            PickLists::ORGANIZATIONS_OQL,
            "SELECT Organization WHERE status = 'active'"
        );
        assert_eq!(
            PickLists::callers_oql("4").as_deref(),
            Some("SELECT Person WHERE org_id = 4 AND status = 'active'")
        );
        assert_eq!(PickLists::callers_oql("4 OR 1=1"), None);
    }

    /// Organizations and callers come from iTop, sorted by name
    #[tokio::test]
    async fn lists() {
        let s = StubServer::with_handler(|r| {
            if r.class().as_deref() == Some("Organization") {
                StubReply::json(
                    r#"{"code":0,"message":"","objects":{"Organization::2":{"class":"Organization","key":"2","fields":{"friendlyname":"Org B"}},"Organization::1":{"class":"Organization","key":"1","fields":{"friendlyname":"Org A"}}}}"#,
                )
            } else {
                StubReply::json(
                    r#"{"code":0,"message":"","objects":{"Person::9":{"class":"Person","key":"9","fields":{"friendlyname":"Zed Caller"}},"Person::8":{"class":"Person","key":"8","fields":{"friendlyname":"Ann Caller"}}}}"#,
                )
            }
        });
        let lists = PickLists::new(writing(&s));
        let names =
            |v: Vec<itop_kit::ServiceRef>| v.into_iter().map(|r| r.name).collect::<Vec<_>>();
        assert_eq!(
            names(lists.organizations().await.unwrap()),
            ["Org A", "Org B"]
        );
        assert_eq!(
            names(lists.callers("1").await.unwrap()),
            ["Ann Caller", "Zed Caller"]
        );
    }

    /// Impact and urgency use iTop's standard values, with its defaults
    #[test]
    fn levels() {
        let ids = |v: Vec<itop_kit::ServiceRef>| v.into_iter().map(|r| r.id).collect::<Vec<_>>();
        let names =
            |v: Vec<itop_kit::ServiceRef>| v.into_iter().map(|r| r.name).collect::<Vec<_>>();
        assert_eq!(ids(PickLists::impacts()), ["1", "2", "3"]);
        assert_eq!(
            names(PickLists::impacts()),
            ["A department", "A service", "A person"]
        );
        assert_eq!(
            names(PickLists::urgencies()),
            ["Critical", "High", "Medium", "Low"]
        );
        assert_eq!(NewTicket::new(TicketClass::UserRequest).impact, "1");
        assert_eq!(NewTicket::new(TicketClass::UserRequest).urgency, "4");
    }
}
