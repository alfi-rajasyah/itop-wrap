//! HTTP for `ITopClient`, standing in for the `URLSession` the Swift client was given.
//!
//! [`Transport`] is injectable so every module can be tested without a network (see
//! [`crate::stub`]). [`ReqwestTransport`] is the real one. Its failures carry the
//! `URLError.Code` raw value URLSession would have given, so `ITopError::Transport(code)` holds
//! the same numbers as before.
//!
//! URLSession behaviours mirrored:
//! - 60 s request timeout (URLSession's `timeoutIntervalForRequest`, an idle timeout): reqwest
//!   `connect_timeout` and `read_timeout` of 60 s, reported as `timedOut` (-1001).
//! - Redirects are followed (up to 10 here, 16 in CFNetwork), then `tooManyRedirects` (-1007),
//!   but only on the same server: same host, and the same scheme and port or an upgrade from
//!   http to https. The form body (with `auth_pwd`) is re-sent on 307/308, so a redirect to
//!   another host or down to http is refused and reported as `badServerResponse` (-1011), which
//!   the client shows as [`crate::ITopError::UnexpectedResponse`]. URLSession followed those.
//! - Certificates are checked against the macOS trust settings (rustls-platform-verifier), so a
//!   certificate trusted in Keychain Access works, and one the Mac doesn't trust fails with a
//!   `serverCertificate*` code, which the client maps to `ITopError::UntrustedCertificate`.
//!
//! Where reqwest can't tell what URLSession could, the closest code is used: an unreachable
//! network is `notConnectedToInternet` only when the OS says "network unreachable" (a failed DNS
//! lookup while offline is `cannotFindHost`, where URLSession may say `notConnectedToInternet`);
//! other connection failures are `cannotConnectToHost`; other TLS failures are
//! `secureConnectionFailed`; anything unrecognised is `unknown`. All of these become
//! `ITopError::Transport`, which the UI shows with one message, so the user sees no difference.
//! Not mirrored: URLSession.shared's cookie store (iTop REST authenticates every call, so no
//! cookie is needed).

use std::error::Error as StdError;
use std::time::Duration;

use async_trait::async_trait;

/// `URLError.Code` raw values (Foundation, NSURLError.h).
pub mod url_error {
    pub const UNKNOWN: i64 = -1;
    pub const CANCELLED: i64 = -999;
    pub const BAD_URL: i64 = -1000;
    pub const TIMED_OUT: i64 = -1001;
    pub const UNSUPPORTED_URL: i64 = -1002;
    pub const CANNOT_FIND_HOST: i64 = -1003;
    pub const CANNOT_CONNECT_TO_HOST: i64 = -1004;
    pub const NETWORK_CONNECTION_LOST: i64 = -1005;
    pub const DNS_LOOKUP_FAILED: i64 = -1006;
    pub const HTTP_TOO_MANY_REDIRECTS: i64 = -1007;
    pub const NOT_CONNECTED_TO_INTERNET: i64 = -1009;
    pub const BAD_SERVER_RESPONSE: i64 = -1011;
    pub const SECURE_CONNECTION_FAILED: i64 = -1200;
    pub const SERVER_CERTIFICATE_HAS_BAD_DATE: i64 = -1201;
    pub const SERVER_CERTIFICATE_UNTRUSTED: i64 = -1202;
    pub const SERVER_CERTIFICATE_HAS_UNKNOWN_ROOT: i64 = -1203;
    pub const SERVER_CERTIFICATE_NOT_YET_VALID: i64 = -1204;
    pub const CLIENT_CERTIFICATE_REJECTED: i64 = -1205;
    pub const CLIENT_CERTIFICATE_REQUIRED: i64 = -1206;
}

/// One HTTP request, as `URLRequest` held it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// The first header with this name, ignoring case (`value(forHTTPHeaderField:)`).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// One HTTP response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A failed request, like `URLError`: `code` is a [`url_error`] value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("transport error {code}: {message}")]
pub struct TransportError {
    pub code: i64,
    pub message: String,
}

impl TransportError {
    pub fn new(code: i64) -> Self {
        TransportError {
            code,
            message: String::new(),
        }
    }
}

/// Sends HTTP requests. Object-safe; use as `Arc<dyn Transport>`.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// The real transport, over reqwest with rustls and the macOS trust settings.
#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// URLSession's request timeout.
    pub const TIMEOUT: Duration = Duration::from_secs(60);

    pub fn new() -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .redirect(redirect_policy())
            .connect_timeout(Self::TIMEOUT)
            .read_timeout(Self::TIMEOUT)
            .user_agent(concat!("iTop Wrap/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| map_reqwest_error(&e))?;
        Ok(ReqwestTransport { client })
    }

    /// A transport around a reqwest client you configured yourself.
    pub fn with_client(client: reqwest::Client) -> Self {
        ReqwestTransport { client }
    }
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|_| TransportError {
                code: url_error::BAD_URL,
                message: "bad method".into(),
            })?;
        let mut builder = self.client.request(method, &request.url);
        for (k, v) in &request.headers {
            builder = builder.header(k, v);
        }
        let response = builder
            .body(request.body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&e))?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let body = response
            .bytes()
            .await
            .map_err(|e| map_reqwest_error(&e))?
            .to_vec();
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

/// Why a redirect wasn't followed: it leaves the server the password was meant for.
#[derive(Debug, thiserror::Error)]
#[error("refused a redirect to another server: {0}")]
pub struct RedirectRefused(String);

/// Whether a redirect from `from` to `to` stays on the same server: same host, and the same
/// scheme and port, or http upgraded to https (port unchanged or the scheme's default).
pub fn is_same_server_redirect(from: &url::Url, to: &url::Url) -> bool {
    if from.host_str() != to.host_str() {
        return false;
    }
    match (from.scheme(), to.scheme()) {
        (a, b) if a == b => from.port_or_known_default() == to.port_or_known_default(),
        ("http", "https") => to.port().is_none() || to.port() == from.port(),
        _ => false,
    }
}

/// Up to 10 redirects, each on the same server (see the module note).
fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects");
        }
        let same = attempt
            .previous()
            .last()
            .is_none_or(|from| is_same_server_redirect(from, attempt.url()));
        if same {
            attempt.follow()
        } else {
            let to = format!(
                "{}://{}",
                attempt.url().scheme(),
                attempt.url().host_str().unwrap_or("")
            );
            attempt.error(RedirectRefused(to))
        }
    })
}

/// The `URLError.Code` URLSession would have given for this failure (see the module note).
pub fn map_reqwest_error(error: &reqwest::Error) -> TransportError {
    let message = error_chain_text(error);
    let code = if let Some(code) = certificate_code(error) {
        code
    } else if error.is_timeout() {
        url_error::TIMED_OUT
    } else if chain(error)
        .iter()
        .any(|e| e.downcast_ref::<RedirectRefused>().is_some())
    {
        url_error::BAD_SERVER_RESPONSE
    } else if error.is_redirect() {
        url_error::HTTP_TOO_MANY_REDIRECTS
    } else if error.is_builder() {
        url_error::BAD_URL
    } else if error.is_connect() {
        connect_code(error)
    } else if error.is_body() || error.is_decode() {
        url_error::NETWORK_CONNECTION_LOST
    } else if error.is_request() {
        // Failures after connecting, while sending or reading the head.
        match io_kind(error) {
            Some(_) => url_error::NETWORK_CONNECTION_LOST,
            None => url_error::UNKNOWN,
        }
    } else {
        url_error::UNKNOWN
    };
    TransportError { code, message }
}

/// Every error in the chain, including the ones `io::Error` hides from `source()`.
fn chain<'a>(error: &'a (dyn StdError + 'static)) -> Vec<&'a (dyn StdError + 'static)> {
    let mut out = Vec::new();
    let mut next: Option<&'a (dyn StdError + 'static)> = Some(error);
    while let Some(e) = next {
        out.push(e);
        if let Some(io) = e.downcast_ref::<std::io::Error>()
            && let Some(inner) = io.get_ref()
        {
            out.extend(chain(inner));
        }
        next = e.source();
    }
    out
}

fn error_chain_text(error: &reqwest::Error) -> String {
    chain(error)
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(": ")
}

/// A certificate rustls (with the macOS verifier) refused, as the matching URLError code.
fn certificate_code(error: &reqwest::Error) -> Option<i64> {
    use rustls::CertificateError as C;
    let links = chain(error);
    for e in &links {
        if let Some(rustls::Error::InvalidCertificate(cert)) = e.downcast_ref::<rustls::Error>() {
            return Some(match cert {
                C::Expired | C::ExpiredContext { .. } => url_error::SERVER_CERTIFICATE_HAS_BAD_DATE,
                C::NotValidYet | C::NotValidYetContext { .. } => {
                    url_error::SERVER_CERTIFICATE_NOT_YET_VALID
                }
                C::UnknownIssuer => url_error::SERVER_CERTIFICATE_HAS_UNKNOWN_ROOT,
                _ => url_error::SERVER_CERTIFICATE_UNTRUSTED,
            });
        }
    }
    for e in &links {
        if let Some(tls) = e.downcast_ref::<rustls::Error>() {
            return Some(match tls {
                rustls::Error::NoCertificatesPresented => url_error::CLIENT_CERTIFICATE_REQUIRED,
                _ => url_error::SECURE_CONNECTION_FAILED,
            });
        }
    }
    None
}

fn io_kind(error: &reqwest::Error) -> Option<std::io::ErrorKind> {
    chain(error)
        .iter()
        .find_map(|e| e.downcast_ref::<std::io::Error>().map(std::io::Error::kind))
}

fn connect_code(error: &reqwest::Error) -> i64 {
    use std::io::ErrorKind as K;
    match io_kind(error) {
        Some(K::NetworkUnreachable) => return url_error::NOT_CONNECTED_TO_INTERNET,
        Some(K::TimedOut) => return url_error::TIMED_OUT,
        Some(K::ConnectionRefused | K::HostUnreachable | K::AddrNotAvailable) => {
            return url_error::CANNOT_CONNECT_TO_HOST;
        }
        _ => {}
    }
    // hyper-util reports a failed lookup as "dns error".
    if chain(error)
        .iter()
        .any(|e| e.to_string().contains("dns error"))
    {
        return url_error::CANNOT_FIND_HOST;
    }
    url_error::CANNOT_CONNECT_TO_HOST
}
