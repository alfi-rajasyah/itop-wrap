//! Port of Tests/ITopKitTests/MenuBarTests.swift.

use chrono::{DateTime, Duration, TimeZone, Utc};
use itop_kit::menu_bar_status::{GROUP_ORDER, pin_running};
use itop_kit::{MenuBarStatus, RunningTimer, Ticket, TicketClass, TicketGroup, TicketSection};

fn start() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 10, 9, 0, 0).unwrap()
}

fn timer() -> RunningTimer {
    let ticket = Ticket {
        key: "412".into(),
        ticket_class: TicketClass::UserRequest,
        r#ref: "R-000412".into(),
        title: "VPN drops".into(),
        status: "assigned".into(),
        priority: 3,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: "0".into(),
        team_id: "10".into(),
        team_name: String::new(),
        agent_id: "2".into(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    };
    RunningTimer::new(&ticket, start())
}

mod menu_bar_tests {
    use super::*;

    /// The item shows the running timer, then the needs-you count, each only when there is one
    #[test]
    fn title() {
        for (count, running, expected) in [
            (0, false, ""),
            (2, false, "2"),
            (0, true, "00:12:40"),
            (3, true, "00:12:40  3"),
        ] {
            let status = MenuBarStatus::new(count, running.then(timer));
            assert_eq!(
                status.title(start() + Duration::seconds(12 * 60 + 40)),
                expected
            );
        }
    }

    /// VoiceOver reads it as one sentence
    #[test]
    fn spoken() {
        for (count, running, expected) in [
            (
                2,
                true,
                "iTop Wrap: 2 tickets need you, timer 0:12 on R-000412",
            ),
            (1, false, "iTop Wrap: 1 ticket needs you"),
            (0, false, "iTop Wrap: nothing needs you"),
            (
                0,
                true,
                "iTop Wrap: nothing needs you, timer 0:12 on R-000412",
            ),
        ] {
            let status = MenuBarStatus::new(count, running.then(timer));
            assert_eq!(
                status.accessibility_label(start() + Duration::seconds(12 * 60 + 40)),
                expected
            );
        }
    }

    /// The panel's groups put escalated first, as DESIGN.md's menu bar table says
    #[test]
    fn order() {
        assert_eq!(
            GROUP_ORDER,
            [
                TicketGroup::EscalatedTtr,
                TicketGroup::EscalatedTto,
                TicketGroup::Assigned,
                TicketGroup::Pending
            ]
        );
    }

    /// The ticket with the running timer moves to the top, out of its group; a group it empties goes
    #[test]
    fn running_first() {
        fn t(id: &str, status: &str) -> Ticket {
            Ticket {
                key: id.into(),
                ticket_class: TicketClass::UserRequest,
                r#ref: format!("R-000{id}"),
                title: String::new(),
                status: status.into(),
                priority: 3,
                caller_name: String::new(),
                org_name: String::new(),
                org_id: "0".into(),
                team_id: "10".into(),
                team_name: String::new(),
                agent_id: "2".into(),
                agent_name: String::new(),
                start_date: None,
                last_update: None,
            }
        }
        let sections = vec![
            TicketSection::new(
                TicketGroup::Assigned,
                vec![t("405", "assigned"), t("412", "assigned")],
            ),
            TicketSection::new(TicketGroup::Pending, vec![t("398", "pending")]),
        ];
        let groups = |s: &[TicketSection]| s.iter().map(|x| x.group).collect::<Vec<_>>();

        let pinned = pin_running(&sections, Some("UserRequest::412"));
        assert_eq!(
            pinned.running.as_ref().map(|t| t.r#ref.as_str()),
            Some("R-000412")
        );
        assert_eq!(
            groups(&pinned.sections),
            [TicketGroup::Assigned, TicketGroup::Pending]
        );
        assert_eq!(
            pinned.sections[0]
                .tickets
                .iter()
                .map(|t| t.r#ref.as_str())
                .collect::<Vec<_>>(),
            ["R-000405"]
        );

        let alone = pin_running(&sections, Some("UserRequest::398"));
        assert_eq!(
            alone.running.as_ref().map(|t| t.r#ref.as_str()),
            Some("R-000398")
        );
        assert_eq!(
            groups(&alone.sections),
            [TicketGroup::Assigned],
            "Pending had only the running ticket"
        );

        let none = pin_running(&sections, None);
        assert!(none.running.is_none() && none.sections == sections);
        let elsewhere = pin_running(&sections, Some("UserRequest::999"));
        assert!(
            elsewhere.running.is_none() && elsewhere.sections == sections,
            "a timer on a ticket not listed changes nothing"
        );
    }
}
