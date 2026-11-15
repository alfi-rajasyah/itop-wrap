//! Port of Tests/ITopKitTests/CaseLogTests.swift. One module per Swift suite; Swift
//! parameterised tests loop over the same arguments.
//!
//! Also LifecycleTests.detailIDs, the one LifecycleTests test that needs only `TicketDetail`
//! (the rest need TicketLifecycle.swift, ported in its own module).

use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use itop_kit::stub::{StubReply, StubServer};
use itop_kit::{
    CaseLog, Credentials, DateLocale, HtmlText, ITopClient, ITopObject, JsonValue, LinkedCI,
    LogDate, ServerAddress, Ticket, TicketClass, TicketDetail, TicketService,
};

fn log(json: &str) -> JsonValue {
    JsonValue::parse_str(json).unwrap()
}

// MARK: - Case log entries

mod case_log_tests {
    use super::*;

    /// Entries come out oldest first, whatever order iTop sends them in
    #[test]
    fn oldest_first() {
        let value = log(r#"
        {"entries":[
          {"date":"2026-11-10 11:05:00","user_login":"Caller 1","message":"It happened again at 11:00."},
          {"date":"2026-11-10 09:12:00","user_login":"Caller 1","message":"The VPN disconnects every hour."},
          {"date":"2026-11-10 10:40:00","user_login":"Agent One","message":"Checking the gateway timeout."}
        ]}
        "#);

        let entries = CaseLog::entries(Some(&value));

        assert_eq!(
            entries
                .iter()
                .map(|e| e.author.as_str())
                .collect::<Vec<_>>(),
            ["Caller 1", "Agent One", "Caller 1"]
        );
        assert_eq!(
            entries.first().map(|e| e.text.as_str()),
            Some("The VPN disconnects every hour.")
        );
    }

    /// Entries written in the same second keep iTop's meaning: the later one in its list is older
    #[test]
    fn same_second() {
        let value = log(
            r#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"B","message":"second"},{"date":"2026-11-10 09:00:00","user_login":"A","message":"first"}]}"#,
        );
        assert_eq!(
            CaseLog::entries(Some(&value))
                .iter()
                .map(|e| e.text.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    /// The HTML version is preferred and turned into plain text with its line breaks
    #[test]
    fn prefers_html() {
        let value = log(
            r#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"A","message":"flat","message_html":"<p>Hello<br>world</p><p>Second paragraph</p>"}]}"#,
        );
        assert_eq!(
            CaseLog::entries(Some(&value))
                .first()
                .map(|e| e.text.as_str()),
            Some("Hello\nworld\n\nSecond paragraph")
        );
    }

    /// Plain message is used when there's no HTML version
    #[test]
    fn falls_back() {
        let value = log(
            r#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"A","message":"Just text"}]}"#,
        );
        assert_eq!(
            CaseLog::entries(Some(&value))
                .first()
                .map(|e| e.text.as_str()),
            Some("Just text")
        );
    }

    /// A missing or odd log gives no entries instead of failing
    #[test]
    fn empty_logs() {
        for json in [
            r#"null"#,
            r#"{"entries":[]}"#,
            r#""text""#,
            r#"{"entries":[{"nothing":1}]}"#,
        ] {
            assert!(CaseLog::entries(Some(&log(json))).is_empty(), "{json}");
        }
    }

    /// Each entry keeps its date
    #[test]
    fn date() {
        let value =
            log(r#"{"entries":[{"date":"2026-11-10 09:12:00","user_login":"A","message":"x"}]}"#);
        assert_eq!(
            CaseLog::entries(Some(&value)).first().and_then(|e| e.date),
            Ticket::parse_date("2026-11-10 09:12:00")
        );
    }
}

// MARK: - HTML to text

mod html_text_tests {
    use super::*;

    /// Entities are decoded
    #[test]
    fn entities() {
        let cases = [
            ("Fish &amp; chips", "Fish & chips"),
            ("a &lt;b&gt; c", "a <b> c"),
            ("it&#39;s", "it's"),
            ("caf&eacute;", "café"),
            ("&quot;q&quot;", "\"q\""),
            ("x&nbsp;y", "x y"),
            ("&#x41;", "A"),
        ];
        for (html, text) in cases {
            assert_eq!(HtmlText::plain(html), text, "{html:?}");
        }
    }

    /// Your real description shape: a paragraph, a break and a non-breaking space
    #[test]
    fn real_description() {
        assert_eq!(HtmlText::plain("<p>[text]<br>\u{00A0}</p>"), "[text]");
    }

    /// Images become a placeholder and nothing is fetched
    #[test]
    fn images() {
        assert_eq!(
            HtmlText::plain(r#"<figure><img src="https://tracker.example.com/x.png"></figure>"#),
            "[Image]"
        );
    }

    /// Links keep their text only
    #[test]
    fn links() {
        assert_eq!(
            HtmlText::plain(r#"See <a href="https://example.com/kb/1">the KB article</a>."#),
            "See the KB article."
        );
    }

    /// List items get bullets, one per line
    #[test]
    fn lists() {
        assert_eq!(
            HtmlText::plain("<ul><li>Restart</li><li>Test again</li></ul>"),
            "• Restart\n• Test again"
        );
    }

    /// Script and style content is dropped
    #[test]
    fn scripts() {
        assert_eq!(
            HtmlText::plain("<style>p{color:red}</style><p>Kept</p><script>alert(1)</script>"),
            "Kept"
        );
    }

    /// Text without tags is left alone, apart from trimming
    #[test]
    fn plain() {
        assert_eq!(
            HtmlText::plain("  Line one\nLine two  "),
            "Line one\nLine two"
        );
    }
}

// MARK: - Detail decoding

mod ticket_detail_tests {
    use super::*;

    /// The real ticket sample decodes into a detail with a plain-text description
    #[tokio::test]
    async fn fixture() {
        let stub = StubServer::new(StubReply::fixture("userrequest"));
        let client = ITopClient::new(
            ServerAddress::new("itop.example.com").unwrap(),
            Credentials::new("agent", "pw"),
            stub.transport(),
        );
        let ticket = Ticket {
            key: "1016".into(),
            ticket_class: TicketClass::UserRequest,
            r#ref: "R-1".into(),
            title: "t".into(),
            status: "assigned".into(),
            priority: 4,
            caller_name: String::new(),
            org_name: String::new(),
            org_id: "0".into(),
            team_id: "0".into(),
            team_name: String::new(),
            agent_id: "0".into(),
            agent_name: String::new(),
            start_date: None,
            last_update: None,
        };

        let detail = TicketService::new(client).detail(&ticket).await.unwrap();

        assert_eq!(detail.description, "[text]");
        assert!(detail.public_log.is_empty() && detail.private_log.is_empty());
        assert!(detail.linked_cis.is_empty());
        let request = stub.requests()[0].json_data();
        assert_eq!(request["key"].as_str(), Some("1016"));
        assert_eq!(
            request["output_fields"]
                .as_str()
                .map(|f| f.contains("private_log")),
            Some(true)
        );
    }

    /// Linked CIs come from functionalcis_list
    #[test]
    fn linked_cis() {
        let object = ITopObject::from_json(&log(r#"
        {"class":"UserRequest","key":"7","fields":{"functionalcis_list":[
          {"functionalci_id":"55","functionalci_id_friendlyname":"vpn-gw-01","functionalci_id_finalclass_recall":"NetworkDevice"},
          {"functionalci_id":"56","functionalci_name":"Remote access VPN"}]}}
        "#))
        .expect("an object");

        let detail = TicketDetail::from_object(&object);

        assert_eq!(
            detail.linked_cis,
            [
                LinkedCI::new("55", "vpn-gw-01", "NetworkDevice"),
                LinkedCI::new("56", "Remote access VPN", "FunctionalCI"),
            ]
        );
    }
}

// MARK: - Log dates

mod log_date_tests {
    use super::*;

    /// Europe/Paris on these dates (November and December): UTC+1 throughout.
    fn paris() -> FixedOffset {
        FixedOffset::east_opt(3600).unwrap()
    }

    fn date(text: &str) -> DateTime<Utc> {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
            .unwrap()
            .and_local_timezone(paris())
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Like the wireframe: Today, Yesterday, a weekday this week, then the date
    #[test]
    fn short() {
        let cases = [
            ("2026-11-10 09:12", "Today 09:12"),
            ("2026-11-09 14:20", "Yesterday 14:20"),
            ("2026-11-06 16:10", "Fri 16:10"),
            ("2026-11-02 09:00", "2 Nov 09:00"),
            ("2025-12-24 10:00", "24 Dec 2025"),
        ];
        let now = date("2026-11-10 12:00");
        for (input, expected) in cases {
            // The Swift test's calendar uses the en_GB locale.
            assert_eq!(
                LogDate::short(date(input), now, &paris(), DateLocale::EnGb),
                expected,
                "{input}"
            );
        }
    }
}

// MARK: - From LifecycleTests

mod lifecycle_tests {
    use super::*;

    /// The detail now carries the service and subcategory ids, for prefilling Resolve
    #[test]
    fn detail_ids() {
        let object = ITopObject::from_json(&log(
            r#"{"class":"UserRequest","key":"5","fields":{"service_id":"3","service_id_friendlyname":"Network access","servicesubcategory_id":"12","servicesubcategory_id_friendlyname":"VPN"}}"#,
        ))
        .expect("an object");
        let detail = TicketDetail::from_object(&object);
        assert!(detail.service_id == "3" && detail.subcategory_id == "12");
        assert!(
            TicketDetail::FIELDS.contains(&"service_id")
                && TicketDetail::FIELDS.contains(&"servicesubcategory_id")
        );
    }
}
