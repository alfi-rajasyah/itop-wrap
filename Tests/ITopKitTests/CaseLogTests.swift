import Foundation
import Testing
@testable import ITopKit

private func log(_ json: String) throws -> JSONValue {
    try JSONDecoder().decode(JSONValue.self, from: Data(json.utf8))
}

// MARK: - Case log entries

@Suite("Case log")
struct CaseLogTests {
    @Test("Entries come out oldest first, whatever order iTop sends them in")
    func oldestFirst() throws {
        let value = try log(#"""
        {"entries":[
          {"date":"2026-11-10 11:05:00","user_login":"Caller 1","message":"It happened again at 11:00."},
          {"date":"2026-11-10 09:12:00","user_login":"Caller 1","message":"The VPN disconnects every hour."},
          {"date":"2026-11-10 10:40:00","user_login":"Agent One","message":"Checking the gateway timeout."}
        ]}
        """#)

        let entries = CaseLog.entries(from: value)

        #expect(entries.map(\.author) == ["Caller 1", "Agent One", "Caller 1"])
        #expect(entries.first?.text == "The VPN disconnects every hour.")
    }

    @Test("Entries written in the same second keep iTop's meaning: the later one in its list is older")
    func sameSecond() throws {
        let value = try log(#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"B","message":"second"},{"date":"2026-11-10 09:00:00","user_login":"A","message":"first"}]}"#)
        #expect(CaseLog.entries(from: value).map(\.text) == ["first", "second"])
    }

    @Test("The HTML version is preferred and turned into plain text with its line breaks")
    func prefersHTML() throws {
        let value = try log(#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"A","message":"flat","message_html":"<p>Hello<br>world</p><p>Second paragraph</p>"}]}"#)
        #expect(CaseLog.entries(from: value).first?.text == "Hello\nworld\n\nSecond paragraph")
    }

    @Test("Plain message is used when there's no HTML version")
    func fallsBack() throws {
        let value = try log(#"{"entries":[{"date":"2026-11-10 09:00:00","user_login":"A","message":"Just text"}]}"#)
        #expect(CaseLog.entries(from: value).first?.text == "Just text")
    }

    @Test("A missing or odd log gives no entries instead of failing", arguments: [#"null"#, #"{"entries":[]}"#, #""text""#, #"{"entries":[{"nothing":1}]}"#])
    func emptyLogs(_ json: String) throws {
        #expect(CaseLog.entries(from: try log(json)).isEmpty)
    }

    @Test("Each entry keeps its date")
    func date() throws {
        let value = try log(#"{"entries":[{"date":"2026-11-10 09:12:00","user_login":"A","message":"x"}]}"#)
        #expect(CaseLog.entries(from: value).first?.date == Ticket.parseDate("2026-11-10 09:12:00"))
    }
}

// MARK: - HTML to text

@Suite("HTML to text")
struct HTMLTextTests {
    @Test("Entities are decoded", arguments: [
        ("Fish &amp; chips", "Fish & chips"), ("a &lt;b&gt; c", "a <b> c"), ("it&#39;s", "it's"),
        ("caf&eacute;", "café"), ("&quot;q&quot;", "\"q\""), ("x&nbsp;y", "x y"), ("&#x41;", "A"),
    ] as [(String, String)])
    func entities(html: String, text: String) {
        #expect(HTMLText.plain(html) == text)
    }

    @Test("Your real description shape: a paragraph, a break and a non-breaking space")
    func realDescription() {
        #expect(HTMLText.plain("<p>[text]<br>\u{00A0}</p>") == "[text]")
    }

    @Test("Images become a placeholder and nothing is fetched")
    func images() {
        #expect(HTMLText.plain(#"<figure><img src="https://tracker.example.com/x.png"></figure>"#) == "[Image]")
    }

    @Test("Links keep their text only")
    func links() {
        #expect(HTMLText.plain(#"See <a href="https://example.com/kb/1">the KB article</a>."#) == "See the KB article.")
    }

    @Test("List items get bullets, one per line")
    func lists() {
        #expect(HTMLText.plain("<ul><li>Restart</li><li>Test again</li></ul>") == "• Restart\n• Test again")
    }

    @Test("Script and style content is dropped")
    func scripts() {
        #expect(HTMLText.plain("<style>p{color:red}</style><p>Kept</p><script>alert(1)</script>") == "Kept")
    }

    @Test("Text without tags is left alone, apart from trimming")
    func plain() {
        #expect(HTMLText.plain("  Line one\nLine two  ") == "Line one\nLine two")
    }
}

// MARK: - Detail decoding

@Suite("Ticket detail")
struct TicketDetailTests {
    @Test("The real ticket sample decodes into a detail with a plain-text description")
    func fixture() async throws {
        let stub = StubServer(.fixture("userrequest"))
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "agent", password: "pw"), session: stub.session)
        let ticket = Ticket(key: "1016", ticketClass: .userRequest, ref: "R-1", title: "t", status: "assigned", priority: 4,
                            callerName: "", orgName: "", teamID: "0", teamName: "", agentID: "0", agentName: "",
                            startDate: nil, lastUpdate: nil)

        let detail = try await TicketService(client: client).detail(for: ticket)

        #expect(detail.description == "[text]")
        #expect(detail.publicLog.isEmpty && detail.privateLog.isEmpty)
        #expect(detail.linkedCIs.isEmpty)
        let request = try #require(stub.requests.first).jsonData
        #expect(request["key"] as? String == "1016")
        #expect((request["output_fields"] as? String)?.contains("private_log") == true)
    }

    @Test("Linked CIs come from functionalcis_list")
    func linkedCIs() throws {
        let object = try #require(ITopObject(json: try log(#"""
        {"class":"UserRequest","key":"7","fields":{"functionalcis_list":[
          {"functionalci_id":"55","functionalci_id_friendlyname":"vpn-gw-01","functionalci_id_finalclass_recall":"NetworkDevice"},
          {"functionalci_id":"56","functionalci_name":"Remote access VPN"}]}}
        """#)))

        let detail = TicketDetail(object: object)

        #expect(detail.linkedCIs == [LinkedCI(id: "55", name: "vpn-gw-01", className: "NetworkDevice"),
                                     LinkedCI(id: "56", name: "Remote access VPN", className: "FunctionalCI")])
    }
}

// MARK: - Log dates

@Suite("Log dates")
struct LogDateTests {
    let calendar: Calendar = {
        var c = Calendar(identifier: .gregorian)
        c.timeZone = TimeZone(identifier: "Europe/Paris")!
        c.locale = Locale(identifier: "en_GB")
        return c
    }()

    func date(_ text: String) -> Date {
        let f = DateFormatter()
        f.calendar = calendar
        f.timeZone = calendar.timeZone
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm"
        return f.date(from: text)!
    }

    @Test("Like the wireframe: Today, Yesterday, a weekday this week, then the date", arguments: [
        ("2026-11-10 09:12", "Today 09:12"),
        ("2026-11-09 14:20", "Yesterday 14:20"),
        ("2026-11-06 16:10", "Fri 16:10"),
        ("2026-11-02 09:00", "2 Nov 09:00"),
        ("2025-12-24 10:00", "24 Dec 2025"),
    ] as [(String, String)])
    func short(input: String, expected: String) {
        let now = date("2026-11-10 12:00")
        #expect(LogDate.short(date(input), now: now, calendar: calendar) == expected)
    }
}
