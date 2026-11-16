//! Port of Tests/ITopKitTests/UpdateTests.swift (all 13 tests).

use std::collections::HashSet;
use std::sync::Arc;

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::ticket_writes::Failure;
use itop_kit::{
    Credentials, ITopClient, ITopError, JsonObject, MemorySecretStore, RestOperation,
    ServerAddress, SessionService, SessionSettings, Ticket, TicketClass, TicketWrites, WriteAccess,
    WriteError,
};

fn client(stub: &StubServer, writes: bool, allow: Option<&[&str]>) -> ITopClient {
    ITopClient::with_writes(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
        writes,
        allow.map(|keys| keys.iter().map(|k| k.to_string()).collect()),
        false,
    )
}

fn ok() -> StubReply {
    StubReply::json(
        r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"updated","fields":{"id":"7"}}}}"#,
    )
}

fn ticket_with(key: &str) -> Ticket {
    Ticket {
        key: key.into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: format!("R-00000{key}"),
        title: "t".into(),
        status: "assigned".into(),
        priority: 3,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: "0".into(),
        team_id: "0".into(),
        team_name: String::new(),
        agent_id: "2".into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

fn ticket() -> Ticket {
    ticket_with("7")
}

fn add_item<'a>(r: &'a serde_json::Value, field: &str) -> &'a serde_json::Value {
    &r["fields"][field]["add_item"]
}

mod log_entry_tests {
    use super::*;

    /// A public entry is appended with core/update, as text, with a comment for the history
    #[tokio::test]
    async fn public_entry() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, true, None))
            .add_log_entry(&ticket(), "Raised the timeout.\nPlease test.", false)
            .await
            .unwrap();

        let r = stub.requests()[0].json_data();
        assert_eq!(r["operation"], "core/update");
        assert_eq!(r["class"], "UserRequest");
        assert_eq!(r["key"], "7");
        assert_eq!(r["comment"], TicketWrites::COMMENT);
        let add = add_item(&r, "public_log");
        assert_eq!(add["message"], "Raised the timeout.\nPlease test.");
        assert_eq!(add["format"], "text");
    }

    /// A private entry goes to private_log
    #[tokio::test]
    async fn private_entry() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, true, None))
            .add_log_entry(&ticket(), "Internal note", true)
            .await
            .unwrap();
        let fields = &stub.requests()[0].json_data()["fields"];
        assert!(fields.get("private_log").is_some() && fields.get("public_log").is_none());
    }

    /// Blank text is refused before anything is sent (arguments: "", "   ", "\n\t")
    #[tokio::test]
    async fn blank() {
        for text in ["", "   ", "\n\t"] {
            let stub = StubServer::new(ok());
            assert_eq!(
                TicketWrites::new(client(&stub, true, None))
                    .add_log_entry(&ticket(), text, false)
                    .await,
                Err(WriteError::Failure(Failure::EmptyEntry)),
                "{text:?}"
            );
            assert!(stub.requests().is_empty());
        }
    }

    /// Surrounding blank lines are trimmed, inner line breaks kept
    #[tokio::test]
    async fn trimmed() {
        let stub = StubServer::new(ok());
        TicketWrites::new(client(&stub, true, None))
            .add_log_entry(&ticket(), "\n  Line one\nLine two  \n\n", false)
            .await
            .unwrap();
        let r = stub.requests()[0].json_data();
        assert_eq!(add_item(&r, "public_log")["message"], "Line one\nLine two");
    }

    /// iTop rejecting this one object is an error, even when the call itself succeeds
    #[tokio::test]
    async fn per_object_failure() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: public_log is read-only for this profile"}}}"#,
        ));
        assert_eq!(
            TicketWrites::new(client(&stub, true, None))
                .add_log_entry(&ticket(), "Hello", false)
                .await,
            Err(WriteError::ITop(ITopError::Server {
                code: 100,
                message: "Error: public_log is read-only for this profile".into()
            }))
        );
    }
}

mod write_safety_tests {
    use super::*;

    /// With writes off, nothing is sent
    #[tokio::test]
    async fn off() {
        let stub = StubServer::new(ok());
        assert_eq!(
            TicketWrites::new(client(&stub, false, None))
                .add_log_entry(&ticket(), "Hello", false)
                .await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert!(stub.requests().is_empty());
    }

    /// With an allowlist, only those tickets can be changed
    #[tokio::test]
    async fn allowlist() {
        let stub = StubServer::new(ok());
        let writes = TicketWrites::new(client(&stub, true, Some(&["UserRequest::7"])));
        writes
            .add_log_entry(&ticket(), "Allowed", false)
            .await
            .unwrap();

        let other = ticket_with("8");
        assert_eq!(
            writes.add_log_entry(&other, "Not allowed", false).await,
            Err(WriteError::ITop(ITopError::WritesDisabled))
        );
        assert_eq!(stub.requests().len(), 1);
    }

    /// Allowlist entries can be refs, resolved to class and key by the app
    #[test]
    fn refs() {
        assert_eq!(TicketWrites::allowlist_key(&ticket()), "UserRequest::7");
    }

    /// With an allowlist, creating new objects is refused
    #[tokio::test]
    async fn allowlist_no_create() {
        let stub = StubServer::new(ok());
        let c = client(&stub, true, Some(&["UserRequest::7"]));
        assert_eq!(
            c.send(RestOperation::create("UserRequest", JsonObject::new(), "x"))
                .await,
            Err(ITopError::WritesDisabled)
        );
        assert!(stub.requests().is_empty());
    }

    /// canWrite reports what the client would allow, for the composer
    #[test]
    fn can_write() {
        let stub = StubServer::new(ok());
        assert!(!client(&stub, false, None).can_write("UserRequest", "7"));
        assert!(client(&stub, true, None).can_write("UserRequest", "7"));
        assert!(!client(&stub, true, Some(&["UserRequest::7"])).can_write("UserRequest", "8"));
    }
}

mod write_access_tests {
    use super::*;

    fn service(stub: &StubServer, write_access: WriteAccess) -> SessionService {
        let transport = stub.transport();
        SessionService::with_make_client(
            Arc::new(MemorySecretStore::new()),
            SessionSettings::in_memory(),
            write_access,
            move |server, credentials| ITopClient::new(server, credentials, transport.clone()),
        )
    }

    /// Refs given for testing become an allowlist of exact objects; unknown refs are left out
    #[tokio::test]
    async fn refs_resolve() {
        let stub = StubServer::with_handler(|request| {
            let key = request.json_data()["key"]
                .as_str()
                .unwrap_or("")
                .to_string();
            match (request.operation().as_deref(), request.class().as_deref()) {
                (Some("core/check_credentials"), _) => StubReply::fixture("check_credentials"),
                (Some("core/get"), Some("User")) => StubReply::fixture("user"),
                (Some("core/get"), Some("Person")) => StubReply::fixture("person"),
                (Some("core/get"), Some("UserRequest")) if key.contains("R-000007") => {
                    StubReply::json(
                        r#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","fields":{"ref":"R-000007"}}}}"#,
                    )
                }
                _ => StubReply::json(r#"{"code":0,"message":"","objects":null}"#),
            }
        });
        let service = service(
            &stub,
            WriteAccess::Tickets {
                refs: vec!["R-000007".into(), "R-999999".into()],
                new_tickets: false,
            },
        );

        let session = service
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();

        assert!(session.client.allows_writes);
        assert_eq!(
            session.client.write_allowlist(),
            Some(HashSet::from(["UserRequest::7".to_string()]))
        );
    }

    fn signed_in_stub() -> StubServer {
        StubServer::with_handler(|request| {
            match (request.operation().as_deref(), request.class().as_deref()) {
                (Some("core/check_credentials"), _) => StubReply::fixture("check_credentials"),
                (Some("core/get"), Some("User")) => StubReply::fixture("user"),
                _ => StubReply::fixture("person"),
            }
        })
    }

    /// By default the session can't write at all
    #[tokio::test]
    async fn off_by_default() {
        let stub = signed_in_stub();
        let session = service(&stub, WriteAccess::default())
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();
        assert!(!session.client.allows_writes);
    }

    /// Writes on everywhere (release builds, from 2026-11-11): no allowlist, creating tickets allowed; iTop's own rights still apply
    #[tokio::test]
    async fn all_on() {
        let stub = signed_in_stub();
        let session = service(&stub, WriteAccess::All)
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();
        assert!(session.client.allows_writes);
        assert_eq!(session.client.write_allowlist(), None);
        assert!(session.client.can_write("Incident", "12345"));
        assert!(session.client.can_create_tickets());
    }
}
