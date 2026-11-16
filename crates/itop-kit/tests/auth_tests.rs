//! Port of Tests/ITopKitTests/AuthTests.swift.

use std::sync::Arc;

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    Credentials, CurrentUser, ITopClient, ITopError, MemorySecretStore, SecretStore, ServerAddress,
    SessionError, SessionService, SessionSettings, Team, WriteAccess,
};

/// A fake iTop that answers like your real server did in the Task 0 probe.
fn fake_itop(reject_password: bool, rest_message: Option<&str>) -> StubServer {
    let rest_message = rest_message.map(str::to_string);
    StubServer::with_handler(move |request| {
        if reject_password {
            return StubReply::json(r#"{"code":1,"message":"Error: Invalid login"}"#);
        }
        if let Some(m) = &rest_message {
            return StubReply::json(&format!(r#"{{"code":1,"message":"{m}"}}"#));
        }
        match (request.operation().as_deref(), request.class().as_deref()) {
            (Some("core/check_credentials"), _) => StubReply::fixture("check_credentials"),
            (Some("core/get"), Some("User")) => StubReply::fixture("user"),
            (Some("core/get"), Some("Person")) => StubReply::fixture("person"),
            _ => StubReply::json(r#"{"code":11,"message":"Error: Unknown operation"}"#),
        }
    })
}

fn fake() -> StubServer {
    fake_itop(false, None)
}

fn service(
    stub: &StubServer,
    secrets: Arc<MemorySecretStore>,
    settings: SessionSettings,
) -> SessionService {
    let transport = stub.transport();
    SessionService::with_make_client(
        secrets,
        settings,
        WriteAccess::Off,
        move |server, credentials| ITopClient::new(server, credentials, transport.clone()),
    )
}

fn fresh_service(stub: &StubServer) -> SessionService {
    service(
        stub,
        Arc::new(MemorySecretStore::new()),
        SessionSettings::in_memory(),
    )
}

// MARK: - Current user

mod current_user_tests {
    use super::*;

    /// Loads your User, then your Person and teams, from the real response shape
    #[tokio::test]
    async fn loads_from_fixtures() {
        let stub = fake();
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent", "pw"),
            stub.transport(),
        );

        let me = CurrentUser::load("agent", &client).await.unwrap();

        assert_eq!(me.user_id, "1001");
        assert_eq!(me.contact_id, "1002");
        assert_eq!(me.profiles.len(), 3);
        assert_eq!(me.teams, vec![Team::new("1009", "[text]")]);
        assert_eq!(me.display_name, "[text] [text]");
    }

    /// The login is quoted safely in the OQL query
    #[tokio::test]
    async fn login_is_quoted() {
        let stub = fake();
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("o'brien", "pw"),
            stub.transport(),
        );

        let _ = CurrentUser::load(r"o'brien\x", &client).await;

        let key = stub
            .requests()
            .into_iter()
            .find(|r| r.class().as_deref() == Some("User"))
            .and_then(|r| r.json_data()["key"].as_str().map(str::to_string));
        assert_eq!(
            key.as_deref(),
            Some(r"SELECT User WHERE login = 'o\'brien\\x'")
        );
    }

    /// Initials come from the display name
    #[test]
    fn initials() {
        let me = CurrentUser {
            user_id: "1".into(),
            contact_id: "2".into(),
            login: "jdoe".into(),
            display_name: "Jane van Doe".into(),
            profiles: vec![],
            teams: vec![],
        };
        assert_eq!(me.initials(), "JD");
    }
}

// MARK: - Session: sign in, relaunch, sign out

mod session_tests {
    use super::*;

    /// Signing in saves the password to the secret store and the server and username to settings
    #[tokio::test]
    async fn sign_in_saves() {
        let stub = fake();
        let secrets = Arc::new(MemorySecretStore::new());
        let settings = SessionSettings::in_memory();

        let session = service(&stub, secrets.clone(), settings.clone())
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();

        assert_eq!(session.user.contact_id, "1002");
        assert_eq!(
            settings.server().as_deref(),
            Some("https://itop.example.com")
        );
        assert_eq!(settings.user().as_deref(), Some("agent"));
        assert_eq!(
            secrets
                .password(&SessionService::account("agent", &session.client.server))
                .unwrap()
                .as_deref(),
            Some("pw")
        );
    }

    /// A wrong password shows as invalid credentials and saves nothing
    #[tokio::test]
    async fn wrong_password() {
        let stub = fake_itop(true, None);
        let secrets = Arc::new(MemorySecretStore::new());
        let settings = SessionSettings::in_memory();

        let result = service(&stub, secrets.clone(), settings.clone())
            .sign_in("itop.example.com", "agent", "wrong")
            .await;
        assert_eq!(
            result.err(),
            Some(SessionError::ITop(ITopError::InvalidCredentials))
        );
        assert_eq!(secrets.count(), 0);
        assert_eq!(settings.user(), None);
    }

    /// An account without REST rights gets its own error, not 'invalid credentials'
    #[tokio::test]
    async fn no_rest_profile() {
        let stub = fake_itop(
            false,
            Some(
                "Error: The current user does not have enough permissions for using the REST services",
            ),
        );
        let result = fresh_service(&stub)
            .sign_in("itop.example.com", "agent", "pw")
            .await;
        assert_eq!(
            result.err(),
            Some(SessionError::ITop(ITopError::RestNotAllowed))
        );
    }

    /// After a relaunch the saved session comes back without asking for anything
    #[tokio::test]
    async fn restore() {
        let stub = fake();
        let secrets = Arc::new(MemorySecretStore::new());
        let settings = SessionSettings::in_memory();
        service(&stub, secrets.clone(), settings.clone())
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();

        let restored = service(&stub, secrets.clone(), settings.clone())
            .restore()
            .await
            .unwrap();

        assert_eq!(
            restored.as_ref().map(|s| s.user.contact_id.as_str()),
            Some("1002")
        );
        assert_eq!(
            restored.map(|s| s.client.credentials),
            Some(Credentials::new("agent", "pw"))
        );
    }

    /// Nothing saved means nothing to restore
    #[tokio::test]
    async fn restore_empty() {
        assert!(fresh_service(&fake()).restore().await.unwrap().is_none());
    }

    /// If the saved password stops working, restore fails and keeps the server and username for the sign-in form
    #[tokio::test]
    async fn restore_rejected() {
        let secrets = Arc::new(MemorySecretStore::new());
        let settings = SessionSettings::in_memory();
        service(&fake(), secrets.clone(), settings.clone())
            .sign_in("itop.example.com", "agent", "old")
            .await
            .unwrap();

        let result = service(&fake_itop(true, None), secrets.clone(), settings.clone())
            .restore()
            .await;
        assert_eq!(
            result.err(),
            Some(SessionError::ITop(ITopError::InvalidCredentials))
        );
        assert_eq!(
            settings.server().as_deref(),
            Some("https://itop.example.com")
        );
        assert_eq!(settings.user().as_deref(), Some("agent"));
    }

    /// Signing out removes the password, so the next launch shows sign-in
    #[tokio::test]
    async fn sign_out() {
        let stub = fake();
        let secrets = Arc::new(MemorySecretStore::new());
        let settings = SessionSettings::in_memory();
        let svc = service(&stub, secrets.clone(), settings.clone());
        let session = svc
            .sign_in("itop.example.com", "agent", "pw")
            .await
            .unwrap();

        svc.sign_out(&session).unwrap();

        assert_eq!(secrets.count(), 0);
        assert!(svc.restore().await.unwrap().is_none());
        assert_eq!(
            settings.user().as_deref(),
            Some("agent"),
            "the username stays filled in for next time"
        );
    }
}

// MARK: - Keychain

// NOT PORTED: KeychainTests.roundTrip ("A password round-trips through the macOS Keychain and is
// gone after delete") talks to the real macOS Keychain, whose store lives in the app crate
// (src-tauri); port it there. The same set/set/read/delete sequence runs against
// MemorySecretStore in `secret_store::tests::round_trip`.
