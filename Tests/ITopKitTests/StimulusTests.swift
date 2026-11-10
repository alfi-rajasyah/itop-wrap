import Foundation
import Testing
@testable import ITopKit

private func t(status: String, agent: String = "0", team: String = "10") -> Ticket {
    Ticket(key: "7", ticketClass: .userRequest, ref: "R-000007", title: "t", status: status, priority: 3,
           callerName: "", orgName: "", teamID: team, teamName: "", agentID: agent, agentName: "",
           startDate: nil, lastUpdate: nil)
}

private let me = CurrentUser(userID: "1", contactID: "2", login: "a", displayName: "Agent Me", profiles: [],
                             teams: [Team(id: "10", name: "Network"), Team(id: "11", name: "Service desk")])

private func client(_ stub: StubServer, allow: Set<String>? = nil) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: true, writeAllowlist: allow)
}

private let ok = StubReply.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"","fields":{"id":"7"}}}}"#)

private func current(agent: String, name: String = "", status: String = "escalated_tto") -> StubReply {
    .json(#"{"code":0,"message":"Found: 1","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"","fields":{"agent_id":"\#(agent)","agent_id_friendlyname":"\#(name)","status":"\#(status)"}}}}"#)
}

@Suite("Assign stimulus")
struct StimulusTests {
    @Test("New and Escalated TTO tickets are assigned; tickets already owned are reassigned", arguments: [
        ("new", "ev_assign"), ("escalated_tto", "ev_assign"),
        ("assigned", "ev_reassign"), ("escalated_ttr", "ev_reassign"), ("pending", "ev_reassign"),
    ] as [(String, String)])
    func stimulus(status: String, expected: String) {
        #expect(TicketWrites.assignStimulus(for: status) == expected)
    }

    @Test("Resolved, closed and unknown states can't be assigned", arguments: ["resolved", "closed", "waiting_for_approval", ""])
    func notAssignable(_ status: String) {
        #expect(TicketWrites.assignStimulus(for: status) == nil)
    }

    @Test("Assigning sends apply_stimulus with the team and agent, and a comment for the history")
    func assign() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).assign(t(status: "assigned", agent: "2"), teamID: "11", agentID: "5")

        let r = try #require(stub.requests.first).jsonData
        #expect(r["operation"] as? String == "core/apply_stimulus")
        #expect(r["class"] as? String == "UserRequest")
        #expect(r["key"] as? String == "7")
        #expect(r["stimulus"] as? String == "ev_reassign")
        #expect(r["comment"] as? String == TicketWrites.comment)
        let fields = r["fields"] as? [String: Any]
        #expect(fields?["team_id"] as? String == "11")
        #expect(fields?["agent_id"] as? String == "5")
    }

    @Test("A ticket that can't be assigned is refused before anything is sent")
    func refusedLocally() async throws {
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.cannotAssign(status: "closed")) {
            try await TicketWrites(client: client(stub)).assign(t(status: "closed"), teamID: "10", agentID: "2")
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("Ids that aren't numbers never reach iTop", arguments: [("1 OR 1=1", "5"), ("10", ""), ("10", "x")] as [(String, String)])
    func badIDs(team: String, agent: String) async throws {
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.self) {
            try await TicketWrites(client: client(stub)).assign(t(status: "assigned"), teamID: team, agentID: agent)
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("iTop refusing the stimulus on the object is an error")
    func perObjectFailure() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: invalid stimulus"}}}"#))
        await #expect(throws: ITopError.server(code: 100, message: "Error: invalid stimulus")) {
            try await TicketWrites(client: client(stub)).assign(t(status: "assigned"), teamID: "10", agentID: "2")
        }
    }

    @Test("A stimulus on a ticket outside the allowlist is never sent")
    func allowlist() async throws {
        let stub = StubServer(ok)
        await #expect(throws: ITopError.writesDisabled) {
            try await TicketWrites(client: client(stub, allow: ["UserRequest::8"])).assign(t(status: "assigned"), teamID: "10", agentID: "2")
        }
        #expect(stub.requests.isEmpty)
    }
}

@Suite("Assign to me")
struct AssignToMeTests {
    @Test("Takes the ticket's own team when I'm in it, in one stimulus, after checking nobody took it")
    func ownTeam() async throws {
        let stub = StubServer { request in
            request.jsonData["operation"] as? String == "core/get" ? current(agent: "0") : ok
        }
        try await TicketWrites(client: client(stub)).assignToMe(t(status: "escalated_tto", team: "11"), me: me)

        #expect(stub.requests.map { $0.jsonData["operation"] as? String } == ["core/get", "core/apply_stimulus"])
        let r = try #require(stub.requests.last).jsonData
        #expect(r["stimulus"] as? String == "ev_assign")
        let fields = r["fields"] as? [String: Any]
        #expect(fields?["team_id"] as? String == "11")
        #expect(fields?["agent_id"] as? String == "2", "my Person id, not my user id")
    }

    @Test("Otherwise takes my first team")
    func firstTeam() async throws {
        let stub = StubServer { request in
            request.jsonData["operation"] as? String == "core/get" ? current(agent: "0") : ok
        }
        try await TicketWrites(client: client(stub)).assignToMe(t(status: "new", team: "0"), me: me)
        let fields = try #require(stub.requests.last).jsonData["fields"] as? [String: Any]
        #expect(fields?["team_id"] as? String == "10")
    }

    @Test("If a colleague took it a moment ago, nothing is changed and their name is reported")
    func race() async throws {
        let stub = StubServer { request in
            request.jsonData["operation"] as? String == "core/get" ? current(agent: "9", name: "Agent 2", status: "assigned") : ok
        }
        await #expect(throws: TicketWrites.Failure.alreadyTaken(by: "Agent 2")) {
            try await TicketWrites(client: client(stub)).assignToMe(t(status: "escalated_tto"), me: me)
        }
        #expect(stub.requests.count == 1, "no stimulus after the check")
    }

    @Test("Without a team, Assign to me explains instead of guessing")
    func noTeam() async throws {
        let loner = CurrentUser(userID: "1", contactID: "2", login: "a", displayName: "A", profiles: [], teams: [])
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.noTeam) {
            try await TicketWrites(client: client(stub)).assignToMe(t(status: "new", team: "0"), me: loner)
        }
        #expect(stub.requests.isEmpty)
    }
}

@Suite("Team and agent pickers")
struct TeamDirectoryTests {
    @Test("OQL: active teams, and active people linked to one team")
    func oql() {
        #expect(TeamQueries.teams == "SELECT Team WHERE status = 'active'")
        #expect(TeamQueries.agents(inTeam: "10") ==
            "SELECT Person AS p JOIN lnkPersonToTeam AS l ON l.person_id = p.id WHERE l.team_id = 10 AND p.status = 'active'")
        #expect(TeamQueries.agents(inTeam: "10 OR 1=1") == nil)
    }

    @Test("Agents come back sorted by name, only for the chosen team")
    func agents() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"Person::5":{"class":"Person","key":"5","fields":{"friendlyname":"Zoe Agent"}},"Person::3":{"class":"Person","key":"3","fields":{"friendlyname":"Ann Agent"}}}}"#))
        let agents = try await TeamDirectory(client: client(stub)).agents(inTeam: "10")
        #expect(agents == [Agent(id: "3", name: "Ann Agent"), Agent(id: "5", name: "Zoe Agent")])
        #expect((try #require(stub.requests.first).jsonData["key"] as? String)?.contains("l.team_id = 10") == true)
    }

    @Test("Teams come back sorted by name")
    func teams() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"Team::2":{"class":"Team","key":"2","fields":{"friendlyname":"Service desk"}},"Team::1":{"class":"Team","key":"1","fields":{"friendlyname":"Network"}}}}"#))
        let teams = try await TeamDirectory(client: client(stub)).teams()
        #expect(teams.map(\.name) == ["Network", "Service desk"])
    }
}
