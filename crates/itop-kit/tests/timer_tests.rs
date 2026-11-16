//! Port of Tests/ITopKitTests/TimerTests.swift (all 18 tests). Parametrised Swift tests loop
//! over the same arguments.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    Credentials, CurrentUser, ITopClient, ITopError, JsonObject, MemorySettingsStore,
    RestOperation, RunningTimer, ServerAddress, Ticket, TicketClass, TicketWrites, TimeFormat,
    TimeTracking, TimerEntry, TimerStore,
};

fn me() -> CurrentUser {
    CurrentUser {
        user_id: "1001".into(),
        contact_id: "1002".into(),
        login: "a".into(),
        display_name: "Me".into(),
        profiles: vec![],
        teams: vec![],
    }
}

fn ticket_full(key: &str, status: &str, org: &str) -> Ticket {
    Ticket {
        key: key.into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: format!("R-00000{key}"),
        title: "VPN drops".into(),
        status: status.into(),
        priority: 3,
        caller_name: String::new(),
        org_name: "Org A".into(),
        org_id: org.into(),
        team_id: "10".into(),
        team_name: String::new(),
        agent_id: "1002".into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

fn ticket_key(key: &str) -> Ticket {
    ticket_full(key, "assigned", "4")
}

fn ticket() -> Ticket {
    ticket_key("7")
}

fn client(stub: &StubServer, allow: Option<&[&str]>) -> ITopClient {
    ITopClient::with_writes(
        ServerAddress::new("itop.example.com").unwrap(),
        Credentials::new("a", "p"),
        stub.transport(),
        true,
        allow.map(|keys| keys.iter().map(|k| k.to_string()).collect()),
        false,
    )
}

fn start() -> DateTime<Utc> {
    Ticket::parse_date("2026-11-11 09:30:00").unwrap()
}

fn secs(s: i64) -> Duration {
    Duration::seconds(s)
}

fn found(cls: &str, rows: &[(&str, &[(&str, &str)])]) -> StubReply {
    let body = rows
        .iter()
        .map(|(key, fields)| {
            let f = fields
                .iter()
                .map(|(k, v)| format!(r#""{k}":"{v}""#))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                r#""{cls}::{key}":{{"class":"{cls}","key":"{key}","code":0,"message":"","fields":{{{f}}}}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    StubReply::json(&format!(
        r#"{{"code":0,"message":"","objects":{{{body}}}}}"#
    ))
}

fn none() -> StubReply {
    StubReply::json(r#"{"code":0,"message":"Found: 0","objects":null}"#)
}

/// TimerTests' `created(_:_:)` helper.
fn created(class: &str, key: &str) -> StubReply {
    StubReply::json(&format!(
        r#"{{"code":0,"message":"","objects":{{"{class}::{key}":{{"class":"{class}","key":"{key}","code":0,"message":"created","fields":{{"id":"{key}"}}}}}}}}"#
    ))
}

mod timer_tests {
    use super::*;

    /// Stopping gives start, end and whole seconds
    #[test]
    fn stop() {
        let r = RunningTimer::new(&ticket(), start());
        let e = TimerEntry::stop(&r, start() + Duration::milliseconds(42 * 60 * 1000 + 9700));
        assert_eq!(e.start, start());
        assert_eq!(e.duration, 2529);
        assert_eq!(e.end, start() + secs(2529));
        assert!(!e.capped);
    }

    /// Past the limit it stops at the limit, like iTop's own stopwatch
    #[test]
    fn capped() {
        let r = RunningTimer::new(&ticket(), start());
        let e = TimerEntry::stop(&r, start() + secs(5 * 3600));
        assert!(e.capped);
        assert_eq!(e.duration, 4 * 3600);
        assert_eq!(e.end, start() + secs(4 * 3600));
    }

    /// Under a minute counts as too short to save
    #[test]
    fn too_short() {
        let r = RunningTimer::new(&ticket(), start());
        assert!(TimerEntry::stop(&r, start() + secs(59)).is_too_short());
        assert!(!TimerEntry::stop(&r, start() + secs(60)).is_too_short());
    }

    /// A clock set backwards never gives negative time
    #[test]
    fn backwards() {
        let r = RunningTimer::new(&ticket(), start());
        assert_eq!(TimerEntry::stop(&r, start() - secs(30)).duration, 0);
    }

    /// Times read as h:mm
    #[test]
    fn format() {
        for (seconds, text) in [
            (0, "0:00"),
            (59, "0:00"),
            (60, "0:01"),
            (2529, "0:42"),
            (3600, "1:00"),
            (36000 + 300, "10:05"),
        ] {
            assert_eq!(TimeFormat::clock(seconds), text);
        }
    }

    /// Totals read like DESIGN.md's rows: 1h 20m
    #[test]
    fn total() {
        for (seconds, text) in [
            (0, "0m"),
            (59, "0m"),
            (60, "1m"),
            (2700, "45m"),
            (3600, "1h"),
            (4800, "1h 20m"),
            (36000 + 300, "10h 5m"),
        ] {
            assert_eq!(TimeFormat::total(seconds), text);
        }
    }

    /// The live counter reads 00:12:34
    #[test]
    fn counter() {
        for (seconds, text) in [(0, "00:00:00"), (754, "00:12:34"), (36000 + 61, "10:01:01")] {
            assert_eq!(TimeFormat::counter(seconds), text);
        }
    }

    /// A running timer survives a restart: it's saved and read back, and cleared
    #[test]
    fn persistence() {
        let defaults = Arc::new(MemorySettingsStore::new());
        let store = TimerStore::new(defaults.clone());
        assert_eq!(store.load(), None);
        let r = RunningTimer::new(&ticket(), start());
        store.save(Some(&r));
        assert_eq!(TimerStore::new(defaults.clone()).load(), Some(r));
        store.save(None);
        assert_eq!(store.load(), None);
    }

    /// The timer knows which ticket it's on
    #[test]
    fn identity() {
        let r = RunningTimer::new(&ticket_key("9"), start());
        assert_eq!(r.ticket_id(), ticket_key("9").id());
        assert_eq!(r.r#ref, "R-000009");
    }
}

mod time_spent_tests {
    use super::*;

    /// Stopping saves one TimeSpent on the ticket's Activity, with your user, contact and the ticket's org
    #[tokio::test]
    async fn save() {
        let stub = StubServer::with_handler(|r| match r.class().as_deref() {
            Some("Activity") => found("Activity", &[("1025", &[("obj_id", "7")])]),
            _ => created("TimeSpent", "77"),
        });
        let entry = TimerEntry::new(start(), start() + secs(2529), 2529, false);
        TimeTracking::new(client(&stub, None))
            .save(&entry, &ticket(), &me())
            .await
            .unwrap();

        let requests = stub.requests();
        assert_eq!(
            requests.iter().map(|r| r.operation()).collect::<Vec<_>>(),
            [
                Some("core/get".to_string()),
                Some("core/create".to_string())
            ]
        );
        let get = requests[0].json_data();
        assert_eq!(
            get["key"],
            "SELECT Activity WHERE obj_class = 'UserRequest' AND obj_id = 7"
        );
        let c = requests[1].json_data();
        assert_eq!(c["class"], "TimeSpent");
        assert_eq!(c["comment"], TicketWrites::COMMENT);
        let f = &c["fields"];
        assert_eq!(f["activity_id"], "1025");
        assert_eq!(f["user_id"], "1001");
        assert_eq!(f["contact_id"], "1002");
        assert_eq!(f["org_id"], "4");
        assert_eq!(f["start_date"], "2026-11-11 09:30:00");
        assert_eq!(f["end_date"], "2026-11-11 10:12:09");
        assert_eq!(f["duration"], "2529");
        assert_eq!(
            f["title"], "R-000007",
            "a ref-only title, like iTop's web stopwatch"
        );
        assert_eq!(
            f["origin"], "stopwatch",
            "the same origin as the web stopwatch, so the calendar shows them alike"
        );
    }

    /// A ticket without an Activity gets one first, then the TimeSpent on it
    #[tokio::test]
    async fn creates_activity() {
        let stub =
            StubServer::with_handler(|r| match (r.operation().as_deref(), r.class().as_deref()) {
                (Some("core/get"), _) => none(),
                (Some("core/create"), Some("Activity")) => created("Activity", "2000"),
                _ => created("TimeSpent", "78"),
            });
        let entry = TimerEntry::new(start(), start() + secs(600), 600, false);
        TimeTracking::new(client(&stub, None))
            .save(&entry, &ticket(), &me())
            .await
            .unwrap();

        let requests = stub.requests();
        assert_eq!(
            requests.iter().map(|r| r.class()).collect::<Vec<_>>(),
            [
                Some("Activity".to_string()),
                Some("Activity".to_string()),
                Some("TimeSpent".to_string())
            ]
        );
        let activity = &requests[1].json_data()["fields"];
        assert_eq!(activity["obj_class"], "UserRequest");
        assert_eq!(activity["obj_id"], "7");
        assert_eq!(activity["label"], "R-000007 VPN drops");
        assert_eq!(requests[2].json_data()["fields"]["activity_id"], "2000");
    }

    /// With a write allowlist, time on an allowed ticket is saved and on any other is refused before sending
    #[tokio::test]
    async fn allowlist() {
        let entry = TimerEntry::new(start(), start() + secs(600), 600, false);
        let allowed = StubServer::with_handler(|r| {
            if r.class().as_deref() == Some("Activity") {
                found("Activity", &[("1025", &[])])
            } else {
                created("TimeSpent", "79")
            }
        });
        TimeTracking::new(client(&allowed, Some(&["UserRequest::7"])))
            .save(&entry, &ticket(), &me())
            .await
            .unwrap();
        assert_eq!(
            allowed
                .requests()
                .last()
                .and_then(|r| r.operation())
                .as_deref(),
            Some("core/create")
        );

        let other = StubServer::with_handler(|r| {
            if r.class().as_deref() == Some("Activity") {
                found("Activity", &[("1025", &[])])
            } else {
                created("TimeSpent", "80")
            }
        });
        assert_eq!(
            TimeTracking::new(client(&other, Some(&["UserRequest::8"])))
                .save(&entry, &ticket(), &me())
                .await,
            Err(ITopError::WritesDisabled)
        );
        assert!(
            other
                .requests()
                .iter()
                .all(|r| r.operation().as_deref() == Some("core/get"))
        );
    }

    /// Other creates stay refused under an allowlist
    #[tokio::test]
    async fn other_creates() {
        let stub = StubServer::new(created("UserRequest", "1"));
        let c = client(&stub, Some(&["UserRequest::7"]));
        assert_eq!(
            c.send(RestOperation::create("UserRequest", JsonObject::new(), "x"))
                .await,
            Err(ITopError::WritesDisabled)
        );
        assert!(stub.requests().is_empty());
    }

    /// iTop refusing the TimeSpent is an error with its message
    #[tokio::test]
    async fn refused() {
        let stub = StubServer::with_handler(|r| {
            if r.class().as_deref() == Some("Activity") {
                found("Activity", &[("1025", &[])])
            } else {
                StubReply::json(
                    r#"{"code":100,"message":"Error: You don't have the Time Tracker profile","objects":null}"#,
                )
            }
        });
        let entry = TimerEntry::new(start(), start() + secs(600), 600, false);
        assert!(
            TimeTracking::new(client(&stub, None))
                .save(&entry, &ticket(), &me())
                .await
                .is_err()
        );
    }
}

mod stopwatch_tests {
    use super::*;

    /// A TimeSpent of yours with no end date is a running web stopwatch, as your probe showed
    #[tokio::test]
    async fn running() {
        let stub = StubServer::with_handler(|r| {
            if r.class().as_deref() == Some("Activity") {
                found("Activity", &[("1025", &[])])
            } else {
                found(
                    "TimeSpent",
                    &[
                        (
                            "1024",
                            &[
                                ("start_date", "2026-11-11 09:30:00"),
                                ("end_date", ""),
                                ("duration", "0"),
                                ("origin", "stopwatch"),
                            ],
                        ),
                        (
                            "1020",
                            &[
                                ("start_date", "2026-11-10 09:00:00"),
                                ("end_date", "2026-11-10 10:00:00"),
                                ("duration", "3600"),
                            ],
                        ),
                    ],
                )
            }
        });
        let since = TimeTracking::new(client(&stub, None))
            .running_stopwatch(&ticket(), &me())
            .await
            .unwrap();
        assert_eq!(since, Some(start()));
        assert_eq!(
            stub.requests().last().unwrap().json_data()["key"],
            "SELECT TimeSpent WHERE activity_id = 1025 AND user_id = 1001"
        );
    }

    /// No open TimeSpent, or no Activity, means no web stopwatch
    #[tokio::test]
    async fn not_running() {
        let stub = StubServer::new(none());
        assert_eq!(
            TimeTracking::new(client(&stub, None))
                .running_stopwatch(&ticket(), &me())
                .await,
            Ok(None)
        );
    }

    /// Totals for a list come from two queries per ticket class, summed per ticket
    #[tokio::test]
    async fn totals() {
        let stub = StubServer::with_handler(|r| match r.class().as_deref() {
            Some("Activity") => found(
                "Activity",
                &[("1025", &[("obj_id", "7")]), ("1026", &[("obj_id", "8")])],
            ),
            _ => found(
                "TimeSpent",
                &[
                    ("1", &[("activity_id", "1025"), ("duration", "600")]),
                    ("2", &[("activity_id", "1025"), ("duration", "1200")]),
                    ("3", &[("activity_id", "1026"), ("duration", "0")]),
                ],
            ),
        });
        let totals = TimeTracking::new(client(&stub, None))
            .totals(&[ticket_key("7"), ticket_key("8"), ticket_key("9")])
            .await
            .unwrap();
        assert_eq!(
            totals,
            HashMap::from([(ticket_key("7").id(), 1800), (ticket_key("8").id(), 0)])
        );
        let requests = stub.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].json_data()["key"],
            "SELECT Activity WHERE obj_class = 'UserRequest' AND obj_id IN (7,8,9)"
        );
        assert_eq!(
            requests[1].json_data()["key"],
            "SELECT TimeSpent WHERE activity_id IN (1025,1026)"
        );
    }

    /// No tickets means no queries
    #[tokio::test]
    async fn empty() {
        let stub = StubServer::new(none());
        assert!(
            TimeTracking::new(client(&stub, None))
                .totals(&[])
                .await
                .unwrap()
                .is_empty()
        );
        assert!(stub.requests().is_empty());
    }
}
