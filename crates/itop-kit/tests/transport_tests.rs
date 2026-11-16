//! Port of Tests/ITopKitTests/TransportTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.

use itop_kit::stub::{Fixture, StubReply, StubServer};
use itop_kit::transport::url_error;
use itop_kit::{
    Credentials, ITopClient, ITopError, ITopResponse, JsonValue, RestOperation, ServerAddress,
};

fn obj(pairs: &[(&str, JsonValue)]) -> itop_kit::JsonObject {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

// MARK: - Server address

mod server_address_tests {
    use super::*;

    /// A bare host name becomes https
    #[test]
    fn bare_host_is_https() {
        let address = ServerAddress::new("itop.example.com").unwrap();
        assert_eq!(address.base_url(), "https://itop.example.com");
        assert!(address.is_secure());
    }

    /// An http address is accepted but flagged as not secure
    #[test]
    fn http_is_flagged() {
        let address = ServerAddress::new("http://itop.example.com").unwrap();
        assert!(!address.is_secure());
    }

    /// Pasting an iTop page URL keeps only the server and its folder
    #[test]
    fn pasted_page_url_is_trimmed() {
        let address = ServerAddress::new(
            "https://itop.example.com/itop/pages/UI.php?operation=details&class=UserRequest&id=4",
        )
        .unwrap();
        assert_eq!(address.base_url(), "https://itop.example.com/itop");
    }

    /// Pasting the REST endpoint itself works too
    #[test]
    fn pasted_rest_url_is_trimmed() {
        let address =
            ServerAddress::new("https://itop.example.com/webservices/rest.php?version=1.3")
                .unwrap();
        assert_eq!(address.base_url(), "https://itop.example.com");
    }

    /// Spaces and a trailing slash are ignored
    #[test]
    fn whitespace_and_slash() {
        let address = ServerAddress::new("  https://itop.example.com/  ").unwrap();
        assert_eq!(address.base_url(), "https://itop.example.com");
    }

    /// The REST endpoint uses version 1.4
    #[test]
    fn endpoint() {
        let address = ServerAddress::new("https://itop.example.com/itop").unwrap();
        assert_eq!(
            address.rest_endpoint(),
            "https://itop.example.com/itop/webservices/rest.php?version=1.4"
        );
    }

    /// Other schemes and empty input are rejected
    #[test]
    fn rejected() {
        for input in ["ftp://itop.example.com", "", "   ", "https://"] {
            assert_eq!(
                ServerAddress::new(input),
                Err(ITopError::InvalidServerAddress),
                "{input:?}"
            );
        }
    }
}

// MARK: - Request encoding

mod request_encoding_tests {
    use super::*;

    /// send posts a form with auth_user, auth_pwd and json_data
    #[tokio::test]
    async fn form_body() {
        let stub = StubServer::new(StubReply::json(r#"{"code":0,"message":"","objects":null}"#));
        let client = ITopClient::new(
            ServerAddress::new("https://itop.example.com").unwrap(),
            Credentials::new("agent", "s3cret"),
            stub.transport(),
        );

        client
            .send(RestOperation::get(
                "UserRequest",
                "SELECT UserRequest WHERE agent_id = 7",
                &["ref", "title"],
            ))
            .await
            .unwrap();

        let request = stub.requests().into_iter().next().expect("a request");
        assert_eq!(request.request.method, "POST");
        assert_eq!(
            request.request.url,
            "https://itop.example.com/webservices/rest.php?version=1.4"
        );
        assert_eq!(
            request.request.header("Content-Type"),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(request.form()["auth_user"], "agent");
        assert_eq!(request.form()["auth_pwd"], "s3cret");
        let json = request.json_data();
        assert_eq!(json["operation"].as_str(), Some("core/get"));
        assert_eq!(json["class"].as_str(), Some("UserRequest"));
        assert_eq!(
            json["key"].as_str(),
            Some("SELECT UserRequest WHERE agent_id = 7")
        );
        assert_eq!(json["output_fields"].as_str(), Some("ref,title"));
    }

    /// Special characters in the password and the query survive form encoding
    #[tokio::test]
    async fn special_characters() {
        let stub = StubServer::new(StubReply::json(r#"{"code":0,"message":""}"#));
        let password = "p&ss=w+rd %20 é?#";
        let oql = "SELECT UserRequest WHERE title LIKE '%VPN & Wi-Fi%'";
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent@corp", password),
            stub.transport(),
        );

        client
            .send(RestOperation::get("UserRequest", oql, &["*"]))
            .await
            .unwrap();

        let request = stub.requests().into_iter().next().expect("a request");
        assert_eq!(request.form()["auth_user"], "agent@corp");
        assert_eq!(request.form()["auth_pwd"], password);
        assert_eq!(request.json_data()["key"].as_str(), Some(oql));
    }

    /// check_credentials sends the user and password in json_data
    #[tokio::test]
    async fn check_credentials_shape() {
        let stub = StubServer::new(StubReply::Response {
            status: 200,
            content_type: "application/json".into(),
            body: Fixture::data("check_credentials").unwrap(),
        });
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent", "pw"),
            stub.transport(),
        );

        let authorized = client.check_credentials().await.unwrap();

        assert!(authorized);
        let json = stub
            .requests()
            .into_iter()
            .next()
            .expect("a request")
            .json_data();
        assert_eq!(json["operation"].as_str(), Some("core/check_credentials"));
        assert_eq!(json["user"].as_str(), Some("agent"));
        assert_eq!(json["password"].as_str(), Some("pw"));
    }
}

// MARK: - Responses and errors

mod response_tests {
    use super::*;

    fn client(reply: StubReply) -> (ITopClient, StubServer) {
        let stub = StubServer::new(reply);
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent", "pw"),
            stub.transport(),
        );
        (client, stub)
    }

    /// A real ticket fixture decodes into objects with their fields
    #[tokio::test]
    async fn decodes_fixture() {
        let (client, _stub) = client(StubReply::Response {
            status: 200,
            content_type: "application/json".into(),
            body: Fixture::data("userrequest").unwrap(),
        });

        let response = client
            .send(RestOperation::get(
                "UserRequest",
                "SELECT UserRequest",
                &["*"],
            ))
            .await
            .unwrap();

        let objects = response.objects();
        let object = objects.first().expect("an object");
        assert_eq!(object.class_name, "UserRequest");
        assert_eq!(
            object.fields.get("status"),
            Some(&JsonValue::from("assigned"))
        );
        assert!(
            object
                .fields
                .get("public_log")
                .and_then(|v| v.get("entries"))
                .is_some()
        );
    }

    /// iTop code 1 means the username or password is wrong
    #[tokio::test]
    async fn unauthorized() {
        let (client, _stub) = client(StubReply::json(
            r#"{"code":1,"message":"Error: Invalid login"}"#,
        ));
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::InvalidCredentials)
        );
    }

    /// Code 1 about the REST services means the account has no REST profile
    #[tokio::test]
    async fn no_rest_profile() {
        let (client, _stub) = client(StubReply::json(
            r#"{"code":1,"message":"Error: The current user does not have enough permissions for using the REST services"}"#,
        ));
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::RestNotAllowed)
        );
    }

    /// Code 1 refusing one change keeps iTop's message, not 'no REST profile'
    #[tokio::test]
    async fn not_allowed() {
        for message in [
            "Error: You are not allowed to create objects of class TimeSpent",
            "Error: not enough permissions to modify this object",
        ] {
            let (client, _stub) = client(StubReply::json(&format!(
                r#"{{"code":1,"message":"{message}"}}"#
            )));
            assert_eq!(
                client.send(RestOperation::ListOperations).await,
                Err(ITopError::NotAllowed {
                    message: message.to_string()
                })
            );
        }
    }

    /// A refused change says what iTop said and doesn't blame the REST profile
    #[test]
    fn not_allowed_text() {
        let text = ITopError::NotAllowed {
            message: "Error: You are not allowed to create objects of class TimeSpent".into(),
        }
        .to_string();
        assert_eq!(
            text,
            "iTop didn't allow it: You are not allowed to create objects of class TimeSpent."
        );
        assert!(!text.contains("REST"));
    }

    /// Other iTop codes keep their number and message
    #[tokio::test]
    async fn server_errors() {
        for (code, message) in [
            (3, "Error: Missing json_data"),
            (11, "Error: Unknown operation"),
            (100, "Error: Internal error"),
        ] {
            let (client, _stub) = client(StubReply::json(&format!(
                r#"{{"code":{code},"message":"{message}"}}"#
            )));
            assert_eq!(
                client.send(RestOperation::ListOperations).await,
                Err(ITopError::Server {
                    code,
                    message: message.to_string()
                })
            );
        }
    }

    /// An HTML page instead of JSON (a login page or proxy) is reported as unexpected
    #[tokio::test]
    async fn html_is_unexpected() {
        let (client, _stub) = client(StubReply::Response {
            status: 200,
            content_type: "text/html".into(),
            body: b"<html><body>Login</body></html>".to_vec(),
        });
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::UnexpectedResponse)
        );
    }

    /// HTTP errors are reported with their status
    #[tokio::test]
    async fn http_status() {
        let (client, _stub) = client(StubReply::json_status("{}", 503));
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::Http { status: 503 })
        );
    }

    /// Network failures become transport errors
    #[tokio::test]
    async fn transport() {
        let (client, _stub) = client(StubReply::failure(url_error::NOT_CONNECTED_TO_INTERNET));
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::Transport(url_error::NOT_CONNECTED_TO_INTERNET))
        );
    }

    /// A certificate the Mac doesn't trust is reported as such, not as a generic network error
    #[tokio::test]
    async fn untrusted_certificate() {
        let (client, _stub) = client(StubReply::failure(url_error::SERVER_CERTIFICATE_UNTRUSTED));
        assert_eq!(
            client.send(RestOperation::ListOperations).await,
            Err(ITopError::UntrustedCertificate)
        );
    }
}

// MARK: - Read-only guard

mod read_only_guard_tests {
    use super::*;

    /// Write operations are refused before anything is sent while writes are off
    #[tokio::test]
    async fn writes_refused() {
        let operations = [
            RestOperation::Update {
                class_name: "UserRequest".into(),
                key: "1".into(),
                fields: obj(&[("title", "x".into())]),
                comment: "iTop Wrap".into(),
            },
            RestOperation::create("UserRequest", obj(&[("title", "x".into())]), "iTop Wrap"),
            RestOperation::ApplyStimulus {
                class_name: "UserRequest".into(),
                key: "1".into(),
                stimulus: "ev_resolve".into(),
                fields: obj(&[]),
                comment: "iTop Wrap".into(),
            },
        ];
        for operation in operations {
            let stub = StubServer::new(StubReply::json(r#"{"code":0,"message":""}"#));
            let client = ITopClient::new(
                ServerAddress::new("itop.example.com").unwrap(),
                Credentials::new("agent", "pw"),
                stub.transport(),
            );

            assert_eq!(
                client.send(operation.clone()).await,
                Err(ITopError::WritesDisabled),
                "{operation:?}"
            );
            assert!(stub.requests().is_empty());
        }
    }

    /// Read operations are allowed while writes are off
    #[test]
    fn reads_allowed() {
        for operation in [
            RestOperation::ListOperations,
            RestOperation::get("Person", "1", &["name"]),
        ] {
            assert!(!operation.is_write());
        }
    }

    /// Writes go through once they're switched on, with the comment attached
    #[tokio::test]
    async fn writes_when_enabled() {
        let stub = StubServer::new(StubReply::json(r#"{"code":0,"message":"","objects":{}}"#));
        let client = ITopClient::with_writes(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent", "pw"),
            stub.transport(),
            true,
            None,
            false,
        );

        client
            .send(RestOperation::Update {
                class_name: "UserRequest".into(),
                key: "1".into(),
                fields: obj(&[("title", "x".into())]),
                comment: "iTop Wrap".into(),
            })
            .await
            .unwrap();

        let json = stub
            .requests()
            .into_iter()
            .next()
            .expect("a request")
            .json_data();
        assert_eq!(json["operation"].as_str(), Some("core/update"));
        assert_eq!(json["comment"].as_str(), Some("iTop Wrap"));
        assert_eq!(json["fields"]["title"].as_str(), Some("x"));
    }
}

// MARK: - Debug trace

mod trace_tests {
    use super::*;

    /// A trace line shows the operation, class, query and what iTop found, never the credentials
    #[test]
    fn line() {
        let body = JsonValue::parse_str(
            r#"{"code":0,"message":"Found: 7","objects":{"UserRequest::1":{"class":"UserRequest","key":"1","fields":{"title":"Secret title"}}}}"#,
        )
        .unwrap();
        let line = ITopClient::trace_line(
            &RestOperation::get(
                "UserRequest",
                "SELECT UserRequest WHERE org_id = 5",
                &["ref"],
            ),
            &ITopResponse::new(body),
        );
        assert_eq!(
            line,
            "[iTop] core/get UserRequest | SELECT UserRequest WHERE org_id = 5 | code 0 | 1 objects | Found: 7"
        );
        assert!(!line.contains("Secret"));
    }

    /// check_credentials never shows the user or password
    #[test]
    fn credentials() {
        let body = JsonValue::parse_str(r#"{"code":0,"message":"","authorized":true}"#).unwrap();
        let line = ITopClient::trace_line(
            &RestOperation::CheckCredentials {
                user: "agent".into(),
                password: "s3cret".into(),
            },
            &ITopResponse::new(body),
        );
        assert!(!line.contains("s3cret") && !line.contains("agent"));
    }
}

mod found_count_tests {
    use super::*;

    /// iTop's "Found: N" message gives the total even when fewer objects came back
    #[test]
    fn found() {
        let cases: [(&str, Option<i64>); 4] = [
            ("Found: 256", Some(256)),
            ("Found: 0", Some(0)),
            ("", None),
            ("Scope: 1; Found: Server=4, VirtualMachine=3", None),
        ];
        for (message, expected) in cases {
            let body =
                JsonValue::parse_str(&format!(r#"{{"code":0,"message":"{message}"}}"#)).unwrap();
            assert_eq!(ITopResponse::new(body).found(), expected, "{message:?}");
        }
    }

    /// People search reports how many matched in total
    #[tokio::test]
    async fn people_total() {
        let stub = StubServer::new(StubReply::json(
            r#"{"code":0,"message":"Found: 256","objects":{"Person::1":{"class":"Person","key":"1","fields":{"name":"Rajasyah","first_name":"Alfi"}}}}"#,
        ));
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("a", "p"),
            stub.transport(),
        );
        let page = itop_kit::OrgService::new(client)
            .people_page_matching("alfi")
            .await
            .unwrap();
        assert_eq!(page.people.len(), 1);
        assert_eq!(page.total, 256);
    }
}
