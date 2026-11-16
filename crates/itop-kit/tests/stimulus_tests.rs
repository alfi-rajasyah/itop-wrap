//! Port of Tests/ITopKitTests/StimulusTests.swift. Parametrised Swift tests loop over the same
//! arguments.

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::ticket_writes::Failure;
use itop_kit::{
    Agent, Credentials, CurrentUser, ITopClient, ITopError, ServerAddress, Team, TeamDirectory,
    TeamQueries, Ticket, TicketClass, TicketWrites, WriteError,
};

fn t(status: &str, agent: &str, team: &str) -> Ticket {
    Ticket {
        key: "7".into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: "R-000007".into(),
        title: "t".into(),
        status: status.into(),
        priority: 3,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: "0".into(),
        team_id: team.into(),
        team_name: String::new(),
        agent_id: agent.into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

/// `t(status:)` with the Swift defaults agent "0", team "10".
fn ts(status: &str) -> Ticket {
    t(status, "0", "10")
}

fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1".into(),
        contact_id: "2".into(),
        login: "a".into(),
        display_name: "Agent Me".into(),
        profiles: vec![],
        teams: vec![Team::new("10", "Network"), Team::new("11", "Service desk")],
    }
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

fn current(agent: &str, name: &str, status: &str) -> StubReply {
    StubReply::json(&format!(
        r#"{{"code":0,"message":"Found: 1","objects":{{"UserRequest::7":{{"class":"UserRequest","key":"7","code":0,"message":"","fields":{{"agent_id":"{agent}","agent_id_friendlyname":"{name}","status":"{status}"}}}}}}}}"#
    ))
}

fn is_get(request: &itop_kit::stub::RecordedRequest) -> bool {
    request.operation().as_deref() == Some("core/get")
}

mod stimulus_tests {
    use super::*;

    /// New and Escalated TTO tickets are assigned; tickets already owned are reassigned
    #[test]
    fn stimulus() {
        for (status, expected) in [
            ("new", "ev_assign"),
            ("escalated_tto", "ev_assign"),
            ("assigned", "ev_reassign"),
            ("escalated_ttr", "ev_reassign"),
            ("pending", "ev_reassign"),
        ] {
            assert_eq!(TicketWrites::assign_stimulus(status), Some(expected));
        }
    }

    /// Resolved, closed and unknown states can't be assigned
    #[test]
    fn not_assignable() {
        for status in ["resolved", "closed", "waiting_for_approval", ""] {
            assert_eq!(TicketWrites::assign_stimulus(status), None, "{status}");
        }
    }

    /// Assigning sends apply_stimulus with the team and agent, and a comment for the history
    #[tokio::test]
    async fn assign() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, None))
            .assign(&t("assigned", "2", "10"), "11", "5")
            .await
            .unwrap();

        let r = stub.requests()[0].json_data();
        assert_eq!(r["operation"], "core/apply_stimulus");
        assert_eq!(r["class"], "UserRequest");
        assert_eq!(r["key"], "7");
        assert_eq!(r["stimulus"], "ev_reassign");
        assert_eq!(r["comment"], TicketWrites::COMMENT);
        assert_eq!(r["fields"]["team_id"], "11");
        assert_eq!(r["fields"]["agent_id"], "5");
    }

    /// A ticket that can't be assigned is refused before anything is sent
    #[tokio::test]
    async fn refused_locally() {
        let stub = StubServer::new(ok());
        assert_eq!(
            TicketWrites::new(client(&stub, None))
                .assign(&ts("closed"), "10", "2")
                .await,
            Err(WriteError::Failure(Failure::CannotAssign {
                status: "closed".into()
            }))
        );
        assert!(stub.requests().is_empty());
    }

    /// Ids that aren't numbers never reach iTop
    #[tokio::test]
    async fn bad_ids() {
        for (team, agent) in [("1 OR 1=1", "5"), ("10", ""), ("10", "x")] {
            let stub = StubServer::new(ok());
            let result = TicketWrites::new(client(&stub, None))
                .assign(&ts("assigned"), team, agent)
                .await;
            assert!(
                matches!(result, Err(WriteError::Failure(_))),
                "{team:?} {agent:?}: {result:?}"
            );
            assert!(stub.requests().is_empty());
        }
    }

    /// iTop refusing the stimulus on the object is an error
    #[tokio::test]
    async fn per_object_failure() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: invalid stimulus"}}}"#,
        ));
        assert_eq!(
            TicketWrites::new(client(&stub, None))
                .assign(&ts("assigned"), "10", "2")
                .await,
            Err(WriteError::ITop(ITopError::Server {
                code: 100,
                message: "Error: invalid stimulus".into()
            }))
        );
    }

    /// A stimulus on a ticket outside the allowlist is never sent
    #[tokio::test]
    async fn allowlist() {
        let stub = StubServer::new(ok());
        assert_eq!(
            TicketWrites::new(client(&stub, Some(&["UserRequest::8"])))
                .assign(&ts("assigned"), "10", "2")
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(stub.requests().is_empty());
    }
}

mod assign_to_me_tests {
    use super::*;

    /// Takes the ticket's own team when I'm in it, in one stimulus, after checking nobody took it
    #[tokio::test]
    async fn own_team() {
        let stub = StubServer::with_handler(|r| {
            if is_get(r) {
                current("0", "", "escalated_tto")
            } else {
                ok()
            }
        });
        TicketWrites::new(client(&stub, None))
            .assign_to_me(&t("escalated_tto", "0", "11"), &me())
            .await
            .unwrap();

        let requests = stub.requests();
        assert_eq!(
            requests.iter().map(|r| r.operation()).collect::<Vec<_>>(),
            [
                Some("core/get".to_string()),
                Some("core/apply_stimulus".to_string())
            ]
        );
        let r = requests.last().unwrap().json_data();
        assert_eq!(r["stimulus"], "ev_assign");
        assert_eq!(r["fields"]["team_id"], "11");
        assert_eq!(r["fields"]["agent_id"], "2", "my Person id, not my user id");
    }

    /// Otherwise takes my first team
    #[tokio::test]
    async fn first_team() {
        let stub = StubServer::with_handler(|r| {
            if is_get(r) {
                current("0", "", "escalated_tto")
            } else {
                ok()
            }
        });
        TicketWrites::new(client(&stub, None))
            .assign_to_me(&t("new", "0", "0"), &me())
            .await
            .unwrap();
        let r = stub.requests().last().unwrap().json_data();
        assert_eq!(r["fields"]["team_id"], "10");
    }

    /// If a colleague took it a moment ago, nothing is changed and their name is reported
    #[tokio::test]
    async fn race() {
        let stub = StubServer::with_handler(|r| {
            if is_get(r) {
                current("9", "Agent 2", "assigned")
            } else {
                ok()
            }
        });
        assert_eq!(
            TicketWrites::new(client(&stub, None))
                .assign_to_me(&ts("escalated_tto"), &me())
                .await,
            Err(WriteError::Failure(Failure::AlreadyTaken {
                by: "Agent 2".into()
            }))
        );
        assert_eq!(stub.requests().len(), 1, "no stimulus after the check");
    }

    /// Without a team, Assign to me explains instead of guessing
    #[tokio::test]
    async fn no_team() {
        let loner = CurrentUser {
            user_id: "1".into(),
            contact_id: "2".into(),
            login: "a".into(),
            display_name: "A".into(),
            profiles: vec![],
            teams: vec![],
        };
        let stub = StubServer::new(ok());
        assert_eq!(
            TicketWrites::new(client(&stub, None))
                .assign_to_me(&t("new", "0", "0"), &loner)
                .await,
            Err(WriteError::Failure(Failure::NoTeam))
        );
        assert!(stub.requests().is_empty());
    }
}

mod team_directory_tests {
    use super::*;

    /// OQL: active teams, and active people linked to one team
    #[test]
    fn oql() {
        assert_eq!(TeamQueries::TEAMS, "SELECT Team WHERE status = 'active'");
        assert_eq!(
            TeamQueries::agents("10").as_deref(),
            Some(
                "SELECT Person AS p JOIN lnkPersonToTeam AS l ON l.person_id = p.id WHERE l.team_id = 10 AND p.status = 'active'"
            )
        );
        assert_eq!(TeamQueries::agents("10 OR 1=1"), None);
    }

    /// Agents come back sorted by name, only for the chosen team
    #[tokio::test]
    async fn agents() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"Person::5":{"class":"Person","key":"5","fields":{"friendlyname":"Zoe Agent"}},"Person::3":{"class":"Person","key":"3","fields":{"friendlyname":"Ann Agent"}}}}"#,
        ));
        let agents = TeamDirectory::new(client(&stub, None))
            .agents("10")
            .await
            .unwrap();
        assert_eq!(
            agents,
            vec![Agent::new("3", "Ann Agent"), Agent::new("5", "Zoe Agent")]
        );
        assert!(
            stub.requests()[0].json_data()["key"]
                .as_str()
                .is_some_and(|k| k.contains("l.team_id = 10"))
        );
    }

    /// Teams come back sorted by name
    #[tokio::test]
    async fn teams() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"Team::2":{"class":"Team","key":"2","fields":{"friendlyname":"Service desk"}},"Team::1":{"class":"Team","key":"1","fields":{"friendlyname":"Network"}}}}"#,
        ));
        let teams = TeamDirectory::new(client(&stub, None))
            .teams()
            .await
            .unwrap();
        assert_eq!(
            teams.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["Network", "Service desk"]
        );
    }
}
