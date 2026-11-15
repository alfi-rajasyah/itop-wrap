//! Port of Tests/ITopKitTests/LifecycleTests.swift. Parametrised Swift tests loop over the same
//! arguments.

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::ticket_writes::Failure;
use itop_kit::{
    Credentials, ITopClient, ITopError, Resolution, ResolutionCode, ServerAddress, ServiceCatalog,
    ServiceQueries, ServiceRef, Ticket, TicketAction, TicketChange, TicketClass, TicketLifecycle,
    TicketWrites, WriteError,
};

fn t(status: &str, cls: TicketClass) -> Ticket {
    Ticket {
        key: "7".into(),
        ticket_class: cls,
        r#ref: "R-000007".into(),
        title: "t".into(),
        status: status.into(),
        priority: 3,
        caller_name: String::new(),
        org_name: "Org A".into(),
        org_id: "4".into(),
        team_id: "10".into(),
        team_name: String::new(),
        agent_id: "2".into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

fn ur(status: &str) -> Ticket {
    t(status, TicketClass::UserRequest)
}

fn client(stub: &StubServer, allow: Option<&[&str]>) -> ITopClient {
    ITopClient::with_writes(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
        true,
        allow.map(|keys| keys.iter().map(|k| k.to_string()).collect()),
        false,
    )
}

fn ok() -> StubReply {
    StubReply::json(
        r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"","fields":{"id":"7"}}}}"#,
    )
}

fn resolution() -> Resolution {
    Resolution::new("3", "12", "assistance", "Raised the gateway timeout.")
}

mod lifecycle_tests {
    use super::*;
    use TicketAction::*;

    /// Actions per state, the same for requests and incidents
    #[test]
    fn actions() {
        let cases: [(&str, Vec<TicketAction>); 8] = [
            ("new", vec![Assign]),
            ("escalated_tto", vec![Assign]),
            ("assigned", vec![Reassign, Pending, Resolve]),
            ("escalated_ttr", vec![Reassign, Pending, Resolve]),
            ("pending", vec![Reassign, Resolve]),
            ("resolved", vec![Reopen, Close]),
            ("closed", vec![]),
            ("waiting_for_approval", vec![]),
        ];
        for (status, expected) in cases {
            assert_eq!(TicketLifecycle::actions(status), expected, "{status}");
        }
    }

    /// The main action: Assign when unowned, Resolve when being worked, Close when resolved
    #[test]
    fn primary() {
        let cases = [
            ("new", Some(Assign)),
            ("escalated_tto", Some(Assign)),
            ("assigned", Some(Resolve)),
            ("escalated_ttr", Some(Resolve)),
            ("pending", Some(Resolve)),
            ("resolved", Some(Close)),
            ("closed", None),
        ];
        for (status, expected) in cases {
            assert_eq!(TicketLifecycle::primary(status), expected, "{status}");
        }
    }

    /// Each action's iTop stimulus
    #[test]
    fn stimuli() {
        assert_eq!(
            TicketAction::ALL.map(TicketAction::stimulus),
            [
                "ev_assign",
                "ev_reassign",
                "ev_pending",
                "ev_resolve",
                "ev_reopen",
                "ev_close"
            ]
        );
    }

    /// Task 7's assign rule agrees with the lifecycle
    #[test]
    fn assign_agrees() {
        for status in [
            "new",
            "escalated_tto",
            "assigned",
            "escalated_ttr",
            "pending",
            "resolved",
            "closed",
        ] {
            let actions = TicketLifecycle::actions(status);
            let expected = if actions.contains(&Assign) {
                Some("ev_assign")
            } else if actions.contains(&Reassign) {
                Some("ev_reassign")
            } else {
                None
            };
            assert_eq!(TicketWrites::assign_stimulus(status), expected, "{status}");
        }
    }
}

mod lifecycle_write_tests {
    use super::*;

    /// Set to pending sends ev_pending with the reason
    #[tokio::test]
    async fn pending() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, None))
            .apply(
                &TicketChange::Pending {
                    reason: "  Waiting for the caller's laptop.  ".into(),
                },
                &ur("assigned"),
            )
            .await
            .unwrap();
        let r = stub.requests()[0].json_data();
        assert_eq!(r["operation"], "core/apply_stimulus");
        assert_eq!(r["stimulus"], "ev_pending");
        assert_eq!(r["comment"], TicketWrites::COMMENT);
        assert_eq!(
            r["fields"]["pending_reason"],
            "Waiting for the caller's laptop."
        );
    }

    /// Resolve sends service, subcategory, resolution code and solution with ev_resolve
    #[tokio::test]
    async fn resolve() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, None))
            .apply(
                &TicketChange::Resolve(resolution()),
                &t("escalated_ttr", TicketClass::Incident),
            )
            .await
            .unwrap();
        let r = stub.requests()[0].json_data();
        assert_eq!(r["class"], "Incident");
        assert_eq!(r["stimulus"], "ev_resolve");
        let f = &r["fields"];
        assert_eq!(f["service_id"], "3");
        assert_eq!(f["servicesubcategory_id"], "12");
        assert_eq!(f["resolution_code"], "assistance");
        assert_eq!(
            f["solution"], "<p>Raised the gateway timeout.</p>",
            "SPEC answer 7: the solution is HTML"
        );
    }

    /// The solution keeps its paragraphs and line breaks, with markup escaped
    #[tokio::test]
    async fn solution_html() {
        let stub = StubServer::new(ok());
        let mut r = resolution();
        r.solution = "Raised the timeout to 8 h.\nAsked the caller to test.\n\nClose if quiet <24h> & stable.".into();
        TicketWrites::new(client(&stub, None))
            .apply(&TicketChange::Resolve(r), &ur("assigned"))
            .await
            .unwrap();
        let f = &stub.requests()[0].json_data()["fields"];
        assert_eq!(
            f["solution"],
            "<p>Raised the timeout to 8 h.<br>Asked the caller to test.</p><p>Close if quiet &lt;24h&gt; &amp; stable.</p>"
        );
    }

    /// Reopen and close send their stimulus with no fields
    #[tokio::test]
    async fn no_fields() {
        for (change, stimulus) in [
            (TicketChange::Reopen, "ev_reopen"),
            (TicketChange::Close, "ev_close"),
        ] {
            let stub = StubServer::new(ok());
            TicketWrites::new(client(&stub, None))
                .apply(&change, &ur("resolved"))
                .await
                .unwrap();
            let r = stub.requests()[0].json_data();
            assert_eq!(r["stimulus"], stimulus);
            assert_eq!(r["fields"].as_object().map(|o| o.is_empty()), Some(true));
        }
    }

    /// Required fields are checked before anything is sent
    #[tokio::test]
    async fn required() {
        let cases = [
            (
                TicketChange::Pending {
                    reason: " \n ".into(),
                },
                "reason",
            ),
            (
                TicketChange::Resolve(Resolution::new("3", "12", "assistance", "  ")),
                "solution",
            ),
            (
                TicketChange::Resolve(Resolution::new("", "12", "assistance", "Done")),
                "service",
            ),
            (
                TicketChange::Resolve(Resolution::new("3", "", "assistance", "Done")),
                "subcategory",
            ),
            (
                TicketChange::Resolve(Resolution::new("3 OR 1=1", "12", "assistance", "Done")),
                "service",
            ),
            (
                TicketChange::Resolve(Resolution::new("3", "12", "", "Done")),
                "resolution code",
            ),
        ];
        for (change, field) in cases {
            let stub = StubServer::new(ok());
            assert_eq!(
                TicketWrites::new(client(&stub, None))
                    .apply(&change, &ur("assigned"))
                    .await,
                Err(WriteError::Failure(Failure::Missing(field.into()))),
                "{field}"
            );
            assert!(stub.requests().is_empty());
        }
    }

    /// An action the state doesn't offer is refused before anything is sent
    #[tokio::test]
    async fn not_offered() {
        let cases = [
            (TicketChange::Pending { reason: "x".into() }, "pending"),
            (TicketChange::Resolve(resolution()), "resolved"),
            (TicketChange::Reopen, "assigned"),
            (TicketChange::Close, "closed"),
        ];
        for (change, status) in cases {
            let stub = StubServer::new(ok());
            assert_eq!(
                TicketWrites::new(client(&stub, None))
                    .apply(&change, &ur(status))
                    .await,
                Err(WriteError::Failure(Failure::NotAllowed {
                    status: status.into()
                }))
            );
            assert!(stub.requests().is_empty());
        }
    }

    /// iTop's own rejection comes back as its message
    #[tokio::test]
    async fn rejected() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: servicesubcategory_id: mandatory"}}}"#,
        ));
        assert_eq!(
            TicketWrites::new(client(&stub, None))
                .apply(&TicketChange::Resolve(resolution()), &ur("assigned"))
                .await,
            Err(WriteError::ITop(ITopError::Server {
                code: 100,
                message: "Error: servicesubcategory_id: mandatory".into()
            }))
        );
    }

    /// Lifecycle changes respect the write allowlist
    #[tokio::test]
    async fn allowlist() {
        let stub = StubServer::new(ok());
        assert_eq!(
            TicketWrites::new(client(&stub, Some(&["UserRequest::99"])))
                .apply(&TicketChange::Close, &ur("resolved"))
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(stub.requests().is_empty());
    }
}

mod service_catalog_tests {
    use super::*;

    /// OQL: services the org has a contract for, all live services as a fallback, and a service's subcategories
    #[test]
    fn oql() {
        assert_eq!(
            ServiceQueries::services("4").as_deref(),
            Some(
                "SELECT Service AS s JOIN lnkCustomerContractToService AS l ON l.service_id = s.id JOIN CustomerContract AS cc ON l.customercontract_id = cc.id WHERE cc.org_id = 4 AND s.status != 'obsolete'"
            )
        );
        assert_eq!(
            ServiceQueries::ALL_SERVICES,
            "SELECT Service WHERE status != 'obsolete'"
        );
        assert_eq!(
            ServiceQueries::subcategories("3").as_deref(),
            Some("SELECT ServiceSubcategory WHERE service_id = 3 AND status != 'obsolete'")
        );
        assert_eq!(ServiceQueries::services("4 OR 1=1"), None);
        assert_eq!(ServiceQueries::subcategories(""), None);
    }

    /// When the org has no contracted services, every live service is offered
    #[tokio::test]
    async fn fallback() {
        let stub = StubServer::with_handler(|request| {
            if request.json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("CustomerContract"))
            {
                StubReply::json(r#"{"code":0,"message":"Found: 0","objects":null}"#)
            } else {
                StubReply::json(
                    r#"{"code":0,"message":"","objects":{"Service::3":{"class":"Service","key":"3","fields":{"friendlyname":"Network access"}}}}"#,
                )
            }
        });
        let services = ServiceCatalog::new(client(&stub, None))
            .services(&ur("assigned"))
            .await
            .unwrap();
        assert_eq!(services, vec![ServiceRef::new("3", "Network access")]);
        assert_eq!(stub.requests().len(), 2);
    }

    /// Standard resolution codes, labelled in words
    #[test]
    fn codes() {
        assert_eq!(
            ResolutionCode::standard()
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            [
                "assistance",
                "bug fixed",
                "hardware repair",
                "other",
                "software patch",
                "system update",
                "training"
            ]
        );
        assert_eq!(
            ResolutionCode::standard().first().map(ResolutionCode::name),
            Some("Assistance".to_string())
        );
        assert_eq!(ResolutionCode::new("bug fixed").name(), "Bug fixed");
    }

    // NOT PORTED: ServiceCatalogTests.detailIDs ("The detail now carries the service and
    // subcategory ids, for prefilling Resolve"). It tests `TicketDetail` (TicketDetail.swift),
    // which is ported by the reads module and belongs with its tests.
}
