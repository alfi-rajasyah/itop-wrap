import Foundation
import Testing
@testable import ITopKit

private let me = CurrentUser(userID: "1001", contactID: "1002", login: "a", displayName: "Me", profiles: [], teams: [])

private func ticket(_ key: String = "7", status: String = "assigned", org: String = "4") -> Ticket {
    Ticket(key: key, ticketClass: .userRequest, ref: "R-00000\(key)", title: "VPN drops", status: status, priority: 3,
           callerName: "", orgName: "Org A", teamID: "10", teamName: "", agentID: "1002", agentName: "",
           startDate: nil, lastUpdate: nil, orgID: org)
}

private func client(_ stub: StubServer, allow: Set<String>? = nil) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: true, writeAllowlist: allow)
}

private let start = Ticket.parseDate("2026-11-11 09:30:00")!

private func found(_ cls: String, _ rows: [(String, [String: String])]) -> StubReply {
    let body = rows.map { key, fields in
        let f = fields.map { "\"\($0.key)\":\"\($0.value)\"" }.joined(separator: ",")
        return "\"\(cls)::\(key)\":{\"class\":\"\(cls)\",\"key\":\"\(key)\",\"code\":0,\"message\":\"\",\"fields\":{\(f)}}"
    }.joined(separator: ",")
    return .json(#"{"code":0,"message":"","objects":{\#(body)}}"#)
}

private let none = StubReply.json(#"{"code":0,"message":"Found: 0","objects":null}"#)

private func created(_ cls: String, _ key: String) -> StubReply {
    .json(#"{"code":0,"message":"","objects":{"\#(cls)::\#(key)":{"class":"\#(cls)","key":"\#(key)","code":0,"message":"created","fields":{"id":"\#(key)"}}}}"#)
}

@Suite("Timer maths")
struct TimerTests {
    @Test("Stopping gives start, end and whole seconds")
    func stop() {
        let r = RunningTimer(ticket: ticket(), start: start)
        let e = TimerEntry.stop(r, at: start.addingTimeInterval(42 * 60 + 9.7))
        #expect(e.start == start)
        #expect(e.duration == 2529)
        #expect(e.end == start.addingTimeInterval(2529))
        #expect(!e.capped)
    }

    @Test("Past the limit it stops at the limit, like iTop's own stopwatch")
    func capped() {
        let r = RunningTimer(ticket: ticket(), start: start)
        let e = TimerEntry.stop(r, at: start.addingTimeInterval(5 * 3600))
        #expect(e.capped)
        #expect(e.duration == 4 * 3600)
        #expect(e.end == start.addingTimeInterval(4 * 3600))
    }

    @Test("Under a minute counts as too short to save")
    func tooShort() {
        let r = RunningTimer(ticket: ticket(), start: start)
        #expect(TimerEntry.stop(r, at: start.addingTimeInterval(59)).isTooShort)
        #expect(!TimerEntry.stop(r, at: start.addingTimeInterval(60)).isTooShort)
    }

    @Test("A clock set backwards never gives negative time")
    func backwards() {
        let r = RunningTimer(ticket: ticket(), start: start)
        #expect(TimerEntry.stop(r, at: start.addingTimeInterval(-30)).duration == 0)
    }

    @Test("Times read as h:mm", arguments: [(0, "0:00"), (59, "0:00"), (60, "0:01"), (2529, "0:42"), (3600, "1:00"), (36000 + 300, "10:05")] as [(Int, String)])
    func format(seconds: Int, text: String) {
        #expect(TimeFormat.clock(seconds) == text)
    }

    @Test("A running timer survives a restart: it's saved and read back, and cleared")
    func persistence() throws {
        let defaults = try #require(UserDefaults(suiteName: "ITopWrapTests.timer.\(UUID().uuidString)"))
        let store = TimerStore(defaults: defaults)
        #expect(store.load() == nil)
        let r = RunningTimer(ticket: ticket(), start: start)
        store.save(r)
        #expect(TimerStore(defaults: defaults).load() == r)
        store.save(nil)
        #expect(store.load() == nil)
    }

    @Test("The timer knows which ticket it's on")
    func identity() {
        let r = RunningTimer(ticket: ticket("9"), start: start)
        #expect(r.ticketID == ticket("9").id)
        #expect(r.ref == "R-000009")
    }
}

@Suite("TimeSpent writes")
struct TimeSpentTests {
    @Test("Stopping saves one TimeSpent on the ticket's Activity, with your user, contact and the ticket's org")
    func save() async throws {
        let stub = StubServer { r in
            switch r.jsonData["class"] as? String {
            case "Activity": return found("Activity", [("1025", ["obj_id": "7"])])
            default: return created("TimeSpent", "77")
            }
        }
        let entry = TimerEntry(start: start, end: start.addingTimeInterval(2529), duration: 2529, capped: false)
        try await TimeTracking(client: client(stub)).save(entry, on: ticket(), me: me)

        #expect(stub.requests.map { $0.jsonData["operation"] as? String } == ["core/get", "core/create"])
        let get = stub.requests[0].jsonData
        #expect(get["key"] as? String == "SELECT Activity WHERE obj_class = 'UserRequest' AND obj_id = 7")
        let c = stub.requests[1].jsonData
        #expect(c["class"] as? String == "TimeSpent")
        #expect(c["comment"] as? String == TicketWrites.comment)
        let f = try #require(c["fields"] as? [String: Any])
        #expect(f["activity_id"] as? String == "1025")
        #expect(f["user_id"] as? String == "1001")
        #expect(f["contact_id"] as? String == "1002")
        #expect(f["org_id"] as? String == "4")
        #expect(f["start_date"] as? String == "2026-11-11 09:30:00")
        #expect(f["end_date"] as? String == "2026-11-11 10:12:09")
        #expect(f["duration"] as? String == "2529")
        #expect(f["title"] as? String == "R-000007", "a ref-only title, like iTop's web stopwatch")
        #expect(f["origin"] as? String == "stopwatch", "the same origin as the web stopwatch, so the calendar shows them alike")
    }

    @Test("A ticket without an Activity gets one first, then the TimeSpent on it")
    func createsActivity() async throws {
        let stub = StubServer { r in
            switch (r.jsonData["operation"] as? String, r.jsonData["class"] as? String) {
            case ("core/get", _): return none
            case ("core/create", "Activity"): return created("Activity", "2000")
            default: return created("TimeSpent", "78")
            }
        }
        let entry = TimerEntry(start: start, end: start.addingTimeInterval(600), duration: 600, capped: false)
        try await TimeTracking(client: client(stub)).save(entry, on: ticket(), me: me)

        #expect(stub.requests.map { $0.jsonData["class"] as? String } == ["Activity", "Activity", "TimeSpent"])
        let activity = try #require(stub.requests[1].jsonData["fields"] as? [String: Any])
        #expect(activity["obj_class"] as? String == "UserRequest")
        #expect(activity["obj_id"] as? String == "7")
        #expect(activity["label"] as? String == "R-000007 VPN drops")
        #expect((stub.requests[2].jsonData["fields"] as? [String: Any])?["activity_id"] as? String == "2000")
    }

    @Test("With a write allowlist, time on an allowed ticket is saved and on any other is refused before sending")
    func allowlist() async throws {
        let entry = TimerEntry(start: start, end: start.addingTimeInterval(600), duration: 600, capped: false)
        let allowed = StubServer { r in
            r.jsonData["class"] as? String == "Activity" ? found("Activity", [("1025", [:])]) : created("TimeSpent", "79")
        }
        try await TimeTracking(client: client(allowed, allow: ["UserRequest::7"])).save(entry, on: ticket(), me: me)
        #expect(allowed.requests.last?.jsonData["operation"] as? String == "core/create")

        let other = StubServer { r in
            r.jsonData["class"] as? String == "Activity" ? found("Activity", [("1025", [:])]) : created("TimeSpent", "80")
        }
        await #expect(throws: ITopError.writesDisabled) {
            try await TimeTracking(client: client(other, allow: ["UserRequest::8"])).save(entry, on: ticket(), me: me)
        }
        #expect(other.requests.allSatisfy { $0.jsonData["operation"] as? String == "core/get" })
    }

    @Test("Other creates stay refused under an allowlist")
    func otherCreates() async throws {
        let stub = StubServer(created("UserRequest", "1"))
        let c = try client(stub, allow: ["UserRequest::7"])
        await #expect(throws: ITopError.writesDisabled) {
            _ = try await c.send(.create(className: "UserRequest", fields: [:], comment: "x"))
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("iTop refusing the TimeSpent is an error with its message")
    func refused() async throws {
        let stub = StubServer { r in
            r.jsonData["class"] as? String == "Activity" ? found("Activity", [("1025", [:])])
                : .json(#"{"code":100,"message":"Error: You don't have the Time Tracker profile","objects":null}"#)
        }
        let entry = TimerEntry(start: start, end: start.addingTimeInterval(600), duration: 600, capped: false)
        await #expect(throws: ITopError.self) {
            try await TimeTracking(client: client(stub)).save(entry, on: ticket(), me: me)
        }
    }
}

@Suite("Web stopwatch and totals")
struct StopwatchTests {
    @Test("A TimeSpent of yours with no end date is a running web stopwatch, as your probe showed")
    func running() async throws {
        let stub = StubServer { r in
            r.jsonData["class"] as? String == "Activity"
                ? found("Activity", [("1025", [:])])
                : found("TimeSpent", [("1024", ["start_date": "2026-11-11 09:30:00", "end_date": "", "duration": "0", "origin": "stopwatch"]),
                                      ("1020", ["start_date": "2026-11-10 09:00:00", "end_date": "2026-11-10 10:00:00", "duration": "3600"])])
        }
        let since = try await TimeTracking(client: client(stub)).runningStopwatch(on: ticket(), me: me)
        #expect(since == start)
        #expect(stub.requests.last?.jsonData["key"] as? String == "SELECT TimeSpent WHERE activity_id = 1025 AND user_id = 1001")
    }

    @Test("No open TimeSpent, or no Activity, means no web stopwatch")
    func notRunning() async throws {
        let stub = StubServer(none)
        #expect(try await TimeTracking(client: client(stub)).runningStopwatch(on: ticket(), me: me) == nil)
    }

    @Test("Totals for a list come from two queries per ticket class, summed per ticket")
    func totals() async throws {
        let stub = StubServer { r in
            switch r.jsonData["class"] as? String {
            case "Activity": return found("Activity", [("1025", ["obj_id": "7"]), ("1026", ["obj_id": "8"])])
            default: return found("TimeSpent", [("1", ["activity_id": "1025", "duration": "600"]),
                                                 ("2", ["activity_id": "1025", "duration": "1200"]),
                                                 ("3", ["activity_id": "1026", "duration": "0"])])
            }
        }
        let totals = try await TimeTracking(client: client(stub)).totals(for: [ticket("7"), ticket("8"), ticket("9")])
        #expect(totals == [ticket("7").id: 1800, ticket("8").id: 0])
        #expect(stub.requests.count == 2)
        #expect(stub.requests[0].jsonData["key"] as? String == "SELECT Activity WHERE obj_class = 'UserRequest' AND obj_id IN (7,8,9)")
        #expect(stub.requests[1].jsonData["key"] as? String == "SELECT TimeSpent WHERE activity_id IN (1025,1026)")
    }

    @Test("No tickets means no queries")
    func empty() async throws {
        let stub = StubServer(none)
        #expect(try await TimeTracking(client: client(stub)).totals(for: []).isEmpty)
        #expect(stub.requests.isEmpty)
    }
}
