//! Not a Swift port: the real transport against local servers, checking that it gives the
//! URLError codes the Swift client got from URLSession (see `transport.rs`). Loopback only.

use std::sync::Arc;
use std::time::Duration;

use itop_kit::transport::url_error;
use itop_kit::{
    Credentials, HttpRequest, ITopClient, ITopError, ReqwestTransport, RestOperation,
    ServerAddress, Transport,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn transport() -> Arc<dyn Transport> {
    Arc::new(ReqwestTransport::new().expect("a transport"))
}

fn client(base: &str) -> ITopClient {
    ITopClient::new(
        ServerAddress::new(base).unwrap(),
        Credentials::new("agent", "p&ss=w+rd"),
        transport(),
    )
}

#[tokio::test]
async fn posts_the_form_and_reads_the_answer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/itop/webservices/rest.php"))
        .and(query_param("version", "1.4"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string_contains(
            "auth_user=agent&auth_pwd=p%26ss%3Dw%2Brd&json_data=",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"code":0,"message":"","authorized":true}"#),
        )
        .mount(&server)
        .await;

    let c = client(&format!("{}/itop", server.uri()));
    assert_eq!(c.check_credentials().await, Ok(true));
}

#[tokio::test]
async fn http_errors_keep_their_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert_eq!(
        client(&server.uri())
            .send(RestOperation::ListOperations)
            .await,
        Err(ITopError::Http { status: 503 })
    );
}

#[tokio::test]
async fn a_closed_port_cannot_connect() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert_eq!(
        client(&format!("http://127.0.0.1:{port}"))
            .send(RestOperation::ListOperations)
            .await,
        Err(ITopError::Transport(url_error::CANNOT_CONNECT_TO_HOST))
    );
}

#[tokio::test]
async fn a_server_that_stops_answering_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let quick = reqwest::Client::builder()
        .read_timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let t = ReqwestTransport::with_client(quick);
    let request = HttpRequest {
        method: "POST".into(),
        url: server.uri(),
        headers: vec![],
        body: vec![],
    };
    assert_eq!(
        t.send(request).await.map_err(|e| e.code),
        Err(url_error::TIMED_OUT)
    );
}

/// A TLS server on loopback with a self-signed certificate for "localhost", which the Mac
/// doesn't trust. It answers one connection, then stops.
async fn self_signed_server() -> u16 {
    use tokio_rustls::rustls::ServerConfig;
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let cert = CertificateDer::from(std::fs::read(dir.join("self-signed-cert.der")).unwrap());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        std::fs::read(dir.join("self-signed-key.der")).unwrap(),
    ));
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        if let Ok((socket, _)) = listener.accept().await
            && let Ok(mut tls) = acceptor.accept(socket).await
        {
            let mut buf = [0u8; 1024];
            let _ = tls.read(&mut buf).await;
            let _ = tls
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}")
                .await;
        }
    });
    port
}

#[tokio::test]
async fn a_certificate_the_mac_does_not_trust_is_reported_as_such() {
    let port = self_signed_server().await;
    assert_eq!(
        client(&format!("https://localhost:{port}"))
            .send(RestOperation::ListOperations)
            .await,
        Err(ITopError::UntrustedCertificate)
    );
}
