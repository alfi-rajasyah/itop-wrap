//! Port of Tests/ITopKitTests/TimerTests.swift: only the test that needs nothing but the client.
//! The rest need TimeTracking (TimeTracking.swift) and are ported with that module.

use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{Credentials, ITopClient, ITopError, JsonObject, RestOperation, ServerAddress};

/// TimerTests' `created(_:_:)` helper.
fn created(class: &str, key: &str) -> StubReply {
    StubReply::json(&format!(
        r#"{{"code":0,"message":"","objects":{{"{class}::{key}":{{"class":"{class}","key":"{key}","code":0,"message":"created","fields":{{"id":"{key}"}}}}}}}}"#
    ))
}

mod time_spent_tests {
    use super::*;

    /// Other creates stay refused under an allowlist
    #[tokio::test]
    async fn other_creates() {
        let stub = StubServer::new(created("UserRequest", "1"));
        let c = ITopClient::with_writes(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("a", "p"),
            stub.transport(),
            true,
            Some(["UserRequest::7".to_string()].into()),
            false,
        );
        assert_eq!(
            c.send(RestOperation::create("UserRequest", JsonObject::new(), "x"))
                .await,
            Err(ITopError::WritesDisabled)
        );
        assert!(stub.requests().is_empty());
    }
}
