//! Port of Tests/ITopKitTests/UpdateTests.swift: the tests that need only the client and the
//! session. The rest need TicketWrites (TicketWrites.swift) and are ported with that module.

use std::collections::HashSet;
use std::sync::Arc;

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    Credentials, ITopClient, ITopError, JsonObject, MemorySecretStore, RestOperation,
    ServerAddress, SessionService, SessionSettings, WriteAccess,
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

// NOT PORTED here (need TicketWrites): LogEntryTests (all), WriteSafetyTests.off,
// WriteSafetyTests.allowlist, WriteSafetyTests.refs.

mod write_safety_tests {
    use super::*;

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
