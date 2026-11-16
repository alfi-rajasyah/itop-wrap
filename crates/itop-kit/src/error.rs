//! Port of `ITopError.swift`.
//!
//! `Display` is Swift's `LocalizedError.errorDescription`, word for word: it is what the user sees.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum ITopError {
    /// The server address can't be used (empty, or not http/https).
    #[error("That server address can't be used. Enter it like https://itop.example.com.")]
    InvalidServerAddress,
    /// iTop code 1: wrong username or password.
    #[error("Invalid credentials. Check your username and password.")]
    InvalidCredentials,
    /// iTop code 1 about the REST services: the account lacks the "REST Services User" profile.
    #[error(
        "This account can't use the iTop REST API. Ask your iTop admin to add the \"REST Services User\" profile, then sign in again."
    )]
    RestNotAllowed,
    /// iTop code 1 refusing this one request (the account can use REST, but not for this), with
    /// iTop's own message.
    #[error("{}", not_allowed_text(message))]
    #[serde(rename_all = "camelCase")]
    NotAllowed { message: String },
    /// Any other non-zero iTop `code`, with iTop's own message.
    #[error("{}", server_text(*code, message))]
    #[serde(rename_all = "camelCase")]
    Server {
        #[ts(type = "number")]
        code: i64,
        message: String,
    },
    /// The web server answered with an HTTP error status.
    #[error("The server answered with HTTP {status}.")]
    #[serde(rename_all = "camelCase")]
    Http { status: u16 },
    /// The network request failed; the value is the `URLError.Code` raw value (see
    /// [`crate::transport::url_error`]).
    #[error("Can't reach the server. Check your connection and the server address.")]
    Transport(#[ts(type = "number")] i64),
    /// TLS failed because the Mac doesn't trust the server's certificate.
    #[error(
        "This Mac doesn't trust the server's certificate. Ask your iTop admin, or trust its certificate in Keychain Access."
    )]
    UntrustedCertificate,
    /// The answer wasn't iTop JSON (for example a login page or a proxy page).
    #[error("The server didn't answer like iTop. Check that the address points to your iTop.")]
    UnexpectedResponse,
    /// A write was attempted while writes are switched off (SPEC.md, Boundaries).
    #[error("Changes to iTop are switched off in this build.")]
    WritesDisabled,
}

impl ITopError {
    /// The text the user sees (Swift's `localizedDescription`).
    pub fn user_message(&self) -> String {
        self.to_string()
    }
}

fn not_allowed_text(message: &str) -> String {
    let stripped = message.strip_prefix("Error: ").unwrap_or(message);
    let mut reason = stripped.trim_matches(is_swift_whitespace).to_string();
    if reason.is_empty() {
        return "iTop didn't allow it.".to_string();
    }
    if let Some(last) = reason.chars().last()
        && !".!?".contains(last)
    {
        reason.push('.');
    }
    format!("iTop didn't allow it: {reason}")
}

/// Foundation's `CharacterSet.whitespaces`: tab and the Unicode space separators (Zs), not
/// newlines.
pub(crate) fn is_swift_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

fn server_text(code: i64, message: &str) -> String {
    if message.is_empty() {
        format!("iTop returned error {code}.")
    } else {
        format!("iTop returned error {code}: {message}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_allowed_variants() {
        let e = |m: &str| ITopError::NotAllowed { message: m.into() }.to_string();
        assert_eq!(e("Error: "), "iTop didn't allow it.");
        assert_eq!(e("  Nope!  "), "iTop didn't allow it: Nope!");
        assert_eq!(e("Error: no"), "iTop didn't allow it: no.");
    }

    #[test]
    fn server_text_with_and_without_message() {
        assert_eq!(
            ITopError::Server {
                code: 3,
                message: String::new()
            }
            .to_string(),
            "iTop returned error 3."
        );
        assert_eq!(
            ITopError::Server {
                code: 3,
                message: "x".into()
            }
            .to_string(),
            "iTop returned error 3: x"
        );
    }

    #[test]
    fn serialises_with_kind() {
        let json = serde_json::to_string(&ITopError::Http { status: 503 }).unwrap();
        assert_eq!(json, r#"{"kind":"http","detail":{"status":503}}"#);
        let json = serde_json::to_string(&ITopError::WritesDisabled).unwrap();
        assert_eq!(json, r#"{"kind":"writesDisabled"}"#);
    }
}
