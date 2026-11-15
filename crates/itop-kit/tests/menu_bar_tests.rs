//! Port of Tests/ITopKitTests/MenuBarTests.swift.

use itop_kit::menu_bar_status::{GROUP_ORDER, pin_running};
use itop_kit::{Ticket, TicketClass, TicketGroup, TicketSection};

// NOT PORTED here: MenuBarTests.title ("The item shows the running timer, then the needs-you
// count, each only when there is one") and MenuBarTests.spoken ("VoiceOver reads it as one
// sentence"). Both build a `MenuBarStatus` with a `RunningTimer` and format it with
// `TimeFormat`, which come from TimeTracking.swift, ported in its own module. Port them with
// `MenuBarStatus` once that module is merged.

mod menu_bar_tests {
    use super::*;

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
