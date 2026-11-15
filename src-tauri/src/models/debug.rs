//! The debug write switches (Swift `DebugLaunch.service`, checklist section 17).
//!
//! Release builds: writes on for everything the account may change, every `ITOPWRAP_*` switch
//! ignored (DBG-1). Debug builds: writes off unless `ITOPWRAP_WRITE_TICKETS` names tickets or
//! `ITOPWRAP_WRITE_NEW=1` allows creating them (DBG-2, DBG-3); `ITOPWRAP_TRACE=1` logs UI events
//! (DBG-9; the REST lines are `itop_kit::ITopClient`'s own).

use std::collections::HashMap;

use itop_kit::WriteAccess;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugSwitches {
    pub write_access: WriteAccess,
    /// One stderr line per UI event.
    pub trace: bool,
}

/// The switches for `env`. `release` is `!cfg!(debug_assertions)` in the app; a parameter so
/// both build kinds are testable.
pub fn switches(env: &HashMap<String, String>, release: bool) -> DebugSwitches {
    if release {
        return DebugSwitches { write_access: WriteAccess::All, trace: false };
    }
    let refs: Vec<String> = env
        .get("ITOPWRAP_WRITE_TICKETS")
        .map(String::as_str)
        .unwrap_or("")
        .split(',')
        .map(|r| r.trim().to_uppercase())
        .filter(|r| !r.is_empty())
        .collect();
    let new_tickets = env.get("ITOPWRAP_WRITE_NEW").map(String::as_str) == Some("1");
    let write_access = if refs.is_empty() && !new_tickets {
        WriteAccess::Off
    } else {
        WriteAccess::Tickets { refs, new_tickets }
    };
    DebugSwitches { write_access, trace: env.get("ITOPWRAP_TRACE").map(String::as_str) == Some("1") }
}

/// The switches for this process and this build.
pub fn from_process() -> DebugSwitches {
    switches(&std::env::vars().collect(), !cfg!(debug_assertions))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn dbg_1_release_allows_all_writes_and_ignores_switches() {
        let s = switches(&env(&[("ITOPWRAP_WRITE_TICKETS", "R-000001"), ("ITOPWRAP_TRACE", "1")]), true);
        assert_eq!(s, DebugSwitches { write_access: WriteAccess::All, trace: false });
    }

    #[test]
    fn dbg_1_debug_is_read_only_by_default() {
        assert_eq!(switches(&env(&[]), false).write_access, WriteAccess::Off);
    }

    #[test]
    fn dbg_2_write_tickets_are_trimmed_uppercased_and_empties_dropped() {
        let s = switches(&env(&[("ITOPWRAP_WRITE_TICKETS", " r-079999, ,I-000002,")]), false);
        assert_eq!(
            s.write_access,
            WriteAccess::Tickets { refs: vec!["R-079999".into(), "I-000002".into()], new_tickets: false }
        );
    }

    #[test]
    fn dbg_3_write_new_alone_gives_an_empty_allowlist_plus_creation() {
        let s = switches(&env(&[("ITOPWRAP_WRITE_NEW", "1")]), false);
        assert_eq!(s.write_access, WriteAccess::Tickets { refs: vec![], new_tickets: true });
        assert_eq!(switches(&env(&[("ITOPWRAP_WRITE_NEW", "yes")]), false).write_access, WriteAccess::Off);
    }

    #[test]
    fn dbg_9_trace_switch() {
        assert!(switches(&env(&[("ITOPWRAP_TRACE", "1")]), false).trace);
        assert!(!switches(&env(&[("ITOPWRAP_TRACE", "0")]), false).trace);
    }
}
