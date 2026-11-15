//! Small rules shared by the CMDB, Organizations, People and New ticket models: ports of Swift
//! helpers that live in the views (`ErrorText`, `TicketGroup.statusLabel`, `Loadable`) or that
//! itop-kit keeps crate-private (Foundation's `.whitespaces`, `String.count`).
//!
//! Locale-dependent text follows en_US (the brief's choice): `join_list` is `ListFormatter` for
//! en_US.

use itop_kit::{ITopError, Session, WriteError};
use serde::Serialize;
use ts_rs::TS;

/// Swift's `Loadable` (W/OrgsModel.swift:31).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loadable<T> {
    Idle,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> Loadable<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Loadable::Loaded(v) => Some(v),
            _ => None,
        }
    }
}

/// `{orgId, orgName}`: an organization a list is scoped to, or a person's organization link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct OrgScope {
    pub org_id: String,
    pub org_name: String,
}

/// Foundation's `CharacterSet.whitespaces`: tab and the Unicode space separators, not newlines.
pub fn is_swift_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// `trimmingCharacters(in: .whitespaces)`.
pub fn trim_whitespaces(s: &str) -> &str {
    s.trim_matches(is_swift_whitespace)
}

/// Swift's `String.count` for the 2-character minimum: code points without combining marks
/// (agrees with grapheme clusters for everything but exotic clusters).
pub fn character_count(s: &str) -> usize {
    s.chars()
        .filter(|c| {
            !matches!(*c as u32,
                0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
                | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x200D)
        })
        .count()
}

/// `ErrorText.host`: the server's host, "the server" without one (W/Workspace.swift:31).
pub fn host(session: &Session) -> String {
    let h = session.client.server.host();
    if h.is_empty() {
        "the server".to_owned()
    } else {
        h.to_owned()
    }
}

/// `ErrorText.describe` (W/Theme.swift:85-92): network failures name the host, everything else
/// is iTop's own text.
pub fn describe(error: &ITopError, host: &str) -> String {
    match error {
        ITopError::Transport(_) => {
            format!("Can't reach {host}. Check your connection, then retry.")
        }
        other => other.user_message(),
    }
}

/// `ErrorText.describe` for a write: the iTop error as above, a `TicketWrites.Failure` as itself.
pub fn describe_write(error: &WriteError, host: &str) -> String {
    match error {
        WriteError::ITop(e) => describe(e, host),
        WriteError::Failure(f) => f.to_string(),
    }
}

/// `TicketGroup.statusLabel` (W/TicketListView.swift:386).
pub fn status_label(status: &str) -> String {
    match status {
        "escalated_ttr" => "Escalated TTR".to_owned(),
        "escalated_tto" => "Escalated TTO".to_owned(),
        "waiting_for_approval" => "Waiting for approval".to_owned(),
        _ => {
            let mut chars = status.chars();
            match chars.next() {
                Some(first) => {
                    let rest: String = chars.collect();
                    format!("{}{}", first.to_uppercase(), rest.replace('_', " "))
                }
                None => String::new(),
            }
        }
    }
}

/// `ListFormatter.localizedString(byJoining:)` for en_US: "A", "A and B", "A, B, and C".
pub fn join_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    }
}

/// Initials the way `PersonChip` draws them: first letters of the first two space-separated
/// words, uppercased (W/OrgsView.swift:96).
pub fn chip_initials(name: &str) -> String {
    name.split(' ')
        .filter(|w| !w.is_empty())
        .take(2)
        .filter_map(|w| w.chars().next())
        .map(|c| c.to_uppercase().collect::<String>())
        .collect()
}

/// The search `.task(id: query + scope)` of the CMDB and People lists: which run is current, and
/// what a run waiting out its debounce replaced.
pub struct SearchRun<R> {
    /// Bumped on every start and cancel; a run whose number is stale drops its answer.
    pub generation: u64,
    /// Set while a run sleeps through its 300 ms debounce: the results before it showed "Searching".
    pub previous: Option<R>,
    /// The task id the current run was started for (`query + scope id`), `None` with no run.
    pub task_id: Option<String>,
    pub handle: Option<tokio::task::AbortHandle>,
}

impl<R> Default for SearchRun<R> {
    fn default() -> Self {
        Self {
            generation: 0,
            previous: None,
            task_id: None,
            handle: None,
        }
    }
}

impl<R> SearchRun<R> {
    /// SwiftUI cancelling the `.task`: a run still in its debounce puts back the results it
    /// replaced (`guard !Task.isCancelled else { results = previous }`); a run already asking
    /// iTop is dropped.
    pub fn cancel(&mut self, results: &mut R) {
        self.generation += 1;
        if let Some(h) = self.handle.take() {
            h.abort();
        }
        if let Some(p) = self.previous.take() {
            *results = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_labels() {
        assert_eq!(status_label("escalated_ttr"), "Escalated TTR");
        assert_eq!(status_label("waiting_for_approval"), "Waiting for approval");
        assert_eq!(status_label("assigned"), "Assigned");
        assert_eq!(status_label("pending_customer"), "Pending customer");
        assert_eq!(status_label(""), "");
    }

    #[test]
    fn list_join_en_us() {
        let s = |v: &[&str]| join_list(&v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&["a"]), "a");
        assert_eq!(s(&["a", "b"]), "a and b");
        assert_eq!(s(&["a", "b", "c"]), "a, b, and c");
    }

    #[test]
    fn whitespace_and_count() {
        assert_eq!(trim_whitespaces("\t ab \u{A0}"), "ab");
        assert_eq!(trim_whitespaces("\nab\n"), "\nab\n");
        assert_eq!(character_count("e\u{301}"), 1);
        assert_eq!(chip_initials("Jane  van Doe"), "JV");
    }

    #[test]
    fn transport_errors_name_the_host() {
        assert_eq!(
            describe(&ITopError::Transport(-1009), "itop.example.com"),
            "Can't reach itop.example.com. Check your connection, then retry."
        );
        assert_eq!(
            describe(&ITopError::Http { status: 500 }, "h"),
            "The server answered with HTTP 500."
        );
    }
}
