import Foundation
import Testing
@testable import ITopKit

private let me = CurrentUser(userID: "1001", contactID: "1002", login: "agent", displayName: "Agent One",
                             profiles: [], teams: [Team(id: "1009", name: "Network"), Team(id: "1010", name: "Service desk")])
private let loner = CurrentUser(userID: "1", contactID: "2", login: "solo", displayName: "Solo", profiles: [], teams: [])

private func ticket(_ ref: String, status: String, agent: String = "1002", team: String = "1009",
                    priority: Int = 3, started: String = "2026-11-10 09:00:00", title: String = "A ticket") -> Ticket {
    Ticket(key: String(ref.suffix(3)), ticketClass: ref.hasPrefix("I") ? .incident : .userRequest, ref: ref, title: title,
           status: status, priority: priority, callerName: "Caller", orgName: "Org A",
           teamID: team, teamName: "Network", agentID: agent, agentName: agent == "0" ? "" : "Agent",
           startDate: Ticket.parseDate(started), lastUpdate: nil)
}

// MARK: - OQL

@Suite("Ticket queries")
struct TicketQueryOQLTests {
    @Test("My tickets: open tickets assigned to me, plus unassigned Escalated TTO in my teams")
    func mine() {
        #expect(TicketQueries.mine(.userRequest, me: me) ==
            "SELECT UserRequest WHERE status NOT IN ('resolved','closed') AND (agent_id = 1002 OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (1009,1010)))")
    }

    @Test("Without teams, My tickets is only what's assigned to me")
    func mineNoTeams() {
        #expect(TicketQueries.mine(.incident, me: loner) ==
            "SELECT Incident WHERE status NOT IN ('resolved','closed') AND agent_id = 2")
    }

    @Test("Team tickets: everything open in my teams")
    func team() {
        #expect(TicketQueries.team(.incident, me: me) ==
            "SELECT Incident WHERE status NOT IN ('resolved','closed') AND team_id IN (1009,1010)")
    }

    @Test("No teams means no team query at all")
    func teamNone() {
        #expect(TicketQueries.team(.userRequest, me: loner) == nil)
    }

    @Test("A full ref is looked up in the right class", arguments: [
        ("R-000512", TicketClass.userRequest, "R-000512"),
        ("i-000088", TicketClass.incident, "I-000088"),
        ("  R-000512 ", TicketClass.userRequest, "R-000512"),
    ] as [(String, TicketClass, String)])
    func refLookup(input: String, cls: TicketClass, ref: String) {
        let lookup = TicketQueries.refLookup(input)
        #expect(lookup?.0 == cls)
        #expect(lookup?.1 == "SELECT \(cls.rawValue) WHERE ref = '\(ref)'")
    }

    @Test("Partial refs and words aren't looked up", arguments: ["412", "R-412", "VPN", "R-0005123", "X-000512", ""])
    func notARef(_ input: String) {
        #expect(TicketQueries.refLookup(input) == nil)
    }
}

// MARK: - Decoding

@Suite("Ticket decoding")
struct TicketDecodingTests {
    @Test("The real ticket sample decodes")
    func fixture() async throws {
        let stub = StubServer(.fixture("userrequest"))
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        let tickets = try await TicketService(client: client).loadMine(me: me)

        let t = try #require(tickets.first)
        #expect(t.status == "assigned")
        #expect(t.priority == 4)
        #expect(t.agentID == "1002")
        #expect(t.teamID == "1009")
        #expect(t.isUnassigned == false)
        #expect(t.startDate == Ticket.parseDate("2026-11-09 11:00:32"))
    }

    @Test("Both classes are loaded and combined, and the request asks only for the fields the list shows")
    func bothClasses() async throws {
        let stub = StubServer { request in
            let cls = request.jsonData["class"] as? String ?? ""
            let ref = cls == "Incident" ? "I-000088" : "R-000412"
            return .json(#"{"code":0,"message":"","objects":{"\#(cls)::7":{"class":"\#(cls)","key":"7","code":0,"message":"","fields":{"ref":"\#(ref)","title":"T","status":"assigned","priority":"2","agent_id":"1002","team_id":"1009","start_date":"2026-11-10 08:00:00"}}}}"#)
        }
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        let tickets = try await TicketService(client: client).loadMine(me: me)

        #expect(Set(tickets.map(\.ref)) == ["R-000412", "I-000088"])
        let fields = stub.requests.first?.jsonData["output_fields"] as? String ?? ""
        #expect(fields.contains("ref") && fields.contains("agent_id_friendlyname") && !fields.contains("*"))
    }

    @Test("Looking up a ref that iTop doesn't return gives nothing, not an error")
    func lookupMissing() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"Found: 0","objects":null}"#))
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        #expect(try await TicketService(client: client).lookup("R-000999") == nil)
    }
}

// MARK: - Grouping and the "needs you" rule

@Suite("Ticket grouping")
struct TicketGroupingTests {
    @Test("Groups come escalated first, and empty groups are left out")
    func order() {
        let groups = TicketList.groups([
            ticket("R-000001", status: "pending"),
            ticket("R-000002", status: "assigned"),
            ticket("R-000003", status: "escalated_ttr"),
        ], me: me, view: .mine)
        #expect(groups.map(\.group) == [.escalatedTTR, .assigned, .pending])
    }

    @Test("Team tickets also have a New group for unassigned tickets that haven't escalated")
    func teamNew() {
        let groups = TicketList.groups([ticket("R-000004", status: "new", agent: "0"), ticket("R-000005", status: "assigned")],
                                       me: me, view: .team)
        #expect(groups.map(\.group) == [.new, .assigned])
    }

    @Test("Needs you: my Escalated TTR, and an unassigned Escalated TTO in my teams. Nothing else.", arguments: [
        ("escalated_ttr", "1002", "1009", true),    // my TTR
        ("escalated_ttr", "77", "1009", false),     // someone else's TTR
        ("escalated_tto", "0", "1009", true),       // unowned TTO in my team
        ("escalated_tto", "0", "555", false),       // unowned TTO in another team
        ("assigned", "1002", "1009", false),        // mine but not escalated
        ("new", "0", "1009", false),                // unassigned, not escalated
    ] as [(String, String, String, Bool)])
    func needsYou(status: String, agent: String, team: String, expected: Bool) {
        #expect(TicketList.needsYou(ticket("R-000010", status: status, agent: agent, team: team), me: me) == expected)
    }

    @Test("Within a group: higher priority first, then the oldest")
    func sortWithinGroup() {
        let groups = TicketList.groups([
            ticket("R-000020", status: "assigned", priority: 3, started: "2026-11-01 09:00:00"),
            ticket("R-000021", status: "assigned", priority: 1, started: "2026-11-09 09:00:00"),
            ticket("R-000022", status: "assigned", priority: 3, started: "2026-10-01 09:00:00"),
        ], me: me, view: .mine)
        #expect(groups.first?.tickets.map(\.ref) == ["R-000021", "R-000022", "R-000020"])
    }

    @Test("Approval states and anything unknown go to Other, so nothing is dropped")
    func other() {
        let groups = TicketList.groups([ticket("R-000030", status: "waiting_for_approval"), ticket("R-000031", status: "something_custom")],
                                       me: me, view: .mine)
        #expect(groups.map(\.group) == [.other])
        #expect(groups.first?.tickets.count == 2)
    }
}

// MARK: - Search and age

@Suite("Search and age")
struct SearchAndAgeTests {
    let tickets = [ticket("R-000412", status: "assigned", title: "VPN drops every hour"),
                   ticket("I-000088", status: "escalated_tto", agent: "0", title: "Mail delivery delayed")]

    @Test("Search matches ref digits, full refs and title words, ignoring case", arguments: [
        ("412", ["R-000412"]), ("r-000412", ["R-000412"]), ("vpn", ["R-000412"]),
        ("delivery mail", ["I-000088"]), ("", ["R-000412", "I-000088"]), ("printer", []),
    ] as [(String, [String])])
    func search(query: String, expected: [String]) {
        #expect(TicketSearch.filter(tickets, query: query).map(\.ref) == expected)
    }

    @Test("Age reads like the wireframe: minutes, hours, days", arguments: [
        (30, "now"), (2400, "40m"), (7200, "2h"), (108_000, "1d"), (777_600, "9d"),
    ] as [(Double, String)])
    func age(seconds: Double, expected: String) {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        #expect(Age.short(from: now.addingTimeInterval(-seconds), now: now) == expected)
    }
}
