//! Port of `ServerAddress.swift` (`ServerAddress` and `Credentials`).
//!
//! The base URL is kept as the exact string Swift's `baseURL.absoluteString` gives, because the
//! Keychain account is built from it ("user@https://itop.example.com"): a password saved by the
//! Swift app must still be found. So this does not go through the `url` crate, which would
//! lowercase the host and re-encode the path. Instead it parses the way Foundation's
//! `URLComponents(string:)` does (RFC 3986, strict about invalid characters), then rebuilds
//! "scheme://host[:port]path" from the decoded parts.
//!
//! Known edges where this may differ from Foundation: an IPv6 host keeps its brackets; a host
//! with non-ASCII letters is accepted as typed (Foundation's handling varies by macOS version);
//! characters a decoded path cannot hold in a URL are re-encoded, as macOS 14's
//! `URL(string:)` does.

use std::fmt;

use crate::error::ITopError;

/// The iTop server the user typed or pasted, normalised to its base URL.
///
/// Accepts a bare host ("itop.example.com", treated as https), a base URL, or a pasted
/// iTop page or REST URL, which is trimmed back to the folder iTop is installed in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServerAddress {
    base_url: String,
    scheme: String,
    host: String,
}

impl ServerAddress {
    pub const REST_VERSION: &'static str = "1.4";

    pub fn new(input: &str) -> Result<ServerAddress, ITopError> {
        // `.whitespacesAndNewlines` is exactly Unicode White_Space, which `trim` uses.
        let mut text = input.trim().to_string();
        if text.is_empty() {
            return Err(ITopError::InvalidServerAddress);
        }
        if !text.contains("://") {
            text = format!("https://{text}");
        }

        let parts = Components::parse(&text).ok_or(ITopError::InvalidServerAddress)?;
        let scheme = parts.scheme.to_lowercase();
        if scheme != "https" && scheme != "http" {
            return Err(ITopError::InvalidServerAddress);
        }
        let host = match parts.host {
            Some(h) if !h.is_empty() => h,
            _ => return Err(ITopError::InvalidServerAddress),
        };

        let mut path = parts.path;
        for marker in ["/pages/", "/webservices/"] {
            if let Some(i) = path.find(marker) {
                path.truncate(i);
            }
        }
        while path.ends_with('/') {
            path.pop();
        }

        let port = parts.port.map(|p| format!(":{p}")).unwrap_or_default();
        let base_url = format!(
            "{scheme}://{}{port}{}",
            encode_invalid(&host),
            encode_invalid(&path)
        );
        Ok(ServerAddress {
            base_url,
            scheme,
            host,
        })
    }

    /// `baseURL.absoluteString`, e.g. "https://itop.example.com/itop".
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The host as typed, e.g. "itop.example.com" (Swift's `baseURL.host()`).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// `false` for http://. The UI warns before sending a password over it.
    pub fn is_secure(&self) -> bool {
        self.scheme == "https"
    }

    pub fn rest_endpoint(&self) -> String {
        format!(
            "{}/webservices/rest.php?version={}",
            self.base_url,
            Self::REST_VERSION
        )
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.base_url)
    }
}

/// The parts of a URL string that `ServerAddress` uses, decoded like `URLComponents`.
struct Components {
    scheme: String,
    host: Option<String>,
    port: Option<u64>,
    path: String,
}

impl Components {
    fn parse(text: &str) -> Option<Components> {
        let colon = text.find(':')?;
        let scheme = &text[..colon];
        let mut chars = scheme.chars();
        if !chars.next()?.is_ascii_alphabetic()
            || !chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        {
            return None;
        }
        let rest = &text[colon + 1..];
        // Everything after the scheme must be made of valid URL characters.
        if !valid_chars(rest, "/?#[]@:") {
            return None;
        }
        let Some(after_slashes) = rest.strip_prefix("//") else {
            // No authority: no host.
            return Some(Components {
                scheme: scheme.into(),
                host: None,
                port: None,
                path: String::new(),
            });
        };
        let authority_end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        let authority = &after_slashes[..authority_end];
        let tail = &after_slashes[authority_end..];
        let path_end = tail.find(['?', '#']).unwrap_or(tail.len());
        let raw_path = &tail[..path_end];
        if raw_path.contains(['[', ']']) {
            return None;
        }

        let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let (raw_host, raw_port) = if hostport.starts_with('[') {
            let close = hostport.find(']')?;
            let after = &hostport[close + 1..];
            let port = match after {
                "" => None,
                p => Some(p.strip_prefix(':')?),
            };
            (&hostport[..=close], port)
        } else {
            match hostport.rsplit_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (hostport, None),
            }
        };
        if !raw_host.starts_with('[') && raw_host.contains(['[', ']', '@', ':']) {
            return None;
        }
        let port = match raw_port {
            None | Some("") => None,
            Some(p) if p.bytes().all(|b| b.is_ascii_digit()) => Some(p.parse().ok()?),
            Some(_) => return None,
        };
        Some(Components {
            scheme: scheme.into(),
            host: Some(percent_decode(raw_host)?),
            port,
            path: percent_decode(raw_path)?,
        })
    }
}

/// RFC 3986 unreserved and sub-delims, plus `extra`; `%` only as a valid escape. Non-ASCII is
/// let through (see the module note).
fn valid_chars(s: &str, extra: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
            continue;
        }
        let ok = c >= 0x80
            || c.is_ascii_alphanumeric()
            || b"-._~!$&'()*+,;=".contains(&c)
            || extra.as_bytes().contains(&c);
        if !ok {
            return false;
        }
        i += 1;
    }
    true
}

/// Percent-decodes, failing (like `URLComponents`) when the bytes aren't UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Re-encodes what can't appear in a URL, like macOS 14's `URL(string:)` does for a decoded
/// path or host. ASCII only; non-ASCII is kept (see the module note).
fn encode_invalid(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let valid_escape = c == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit();
        let keep = c >= 0x80
            || valid_escape
            || c.is_ascii_alphanumeric()
            || b"-._~!$&'()*+,;=:@/[]".contains(&c);
        if keep {
            // Push whole UTF-8 sequences untouched.
            let len = utf8_len(c);
            out.push_str(&s[i..i + len]);
            i += len;
        } else {
            out.push_str(&format!("%{c:02X}"));
            i += 1;
        }
    }
    out
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// The username and password sent with every REST call. The password is never printed:
/// `Debug` and `Display` both show `***`, like Swift's `CustomStringConvertible`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Credentials {
    pub user: String,
    pub password: String,
}

impl Credentials {
    pub fn new(user: impl Into<String>, password: impl Into<String>) -> Self {
        Credentials {
            user: user.into(),
            password: password.into(),
        }
    }
}

impl fmt::Display for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Credentials(user: {}, password: ***)", self.user)
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(s: &str) -> Result<String, ITopError> {
        ServerAddress::new(s).map(|a| a.base_url().to_string())
    }

    #[test]
    fn keeps_host_case_and_port() {
        assert_eq!(
            base("HTTPS://ITop.Example.com:0443/Itop/").unwrap(),
            "https://ITop.Example.com:443/Itop"
        );
        assert_eq!(
            base("itop.example.com:").unwrap(),
            "https://itop.example.com"
        );
    }

    #[test]
    fn rejects_invalid_characters_and_ports() {
        assert_eq!(
            base("https://itop example.com"),
            Err(ITopError::InvalidServerAddress)
        );
        assert_eq!(
            base("https://itop.example.com:abc"),
            Err(ITopError::InvalidServerAddress)
        );
        assert_eq!(
            base("https://itop.example.com/%zz"),
            Err(ITopError::InvalidServerAddress)
        );
        assert_eq!(base("https://:80"), Err(ITopError::InvalidServerAddress));
    }

    #[test]
    fn drops_user_info_query_and_fragment() {
        assert_eq!(
            base("https://me:pw@itop.example.com/itop?x=1#y").unwrap(),
            "https://itop.example.com/itop"
        );
    }

    #[test]
    fn decoded_path_is_re_encoded() {
        assert_eq!(
            base("https://itop.example.com/my%20itop/").unwrap(),
            "https://itop.example.com/my%20itop"
        );
    }

    #[test]
    fn credentials_never_print_the_password() {
        let c = Credentials::new("agent", "s3cret");
        assert_eq!(format!("{c:?}"), "Credentials(user: agent, password: ***)");
        assert_eq!(c.to_string(), "Credentials(user: agent, password: ***)");
    }
}
