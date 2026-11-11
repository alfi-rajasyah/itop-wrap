import Foundation
import Testing
@testable import ITopKit

private let ticket = Ticket(key: "7", ticketClass: .userRequest, ref: "R-000007", title: "t", status: "assigned", priority: 3,
                            callerName: "", orgName: "", teamID: "0", teamName: "", agentID: "2", agentName: "",
                            startDate: nil, lastUpdate: nil)

private func client(_ stub: StubServer, writes: Bool = true, allow: Set<String>? = nil) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: writes, writeAllowlist: allow)
}

private let ok = StubReply.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"updated","fields":{"id":"7"}}}}"#)

@Suite("Add a log entry")
struct LogEntryTests {
    @Test("A public entry is appended with core/update, as text, with a comment for the history")
    func publicEntry() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "Raised the timeout.\nPlease test.", isPrivate: false)

        let r = try #require(stub.requests.first).jsonData
        #expect(r["operation"] as? String == "core/update")
        #expect(r["class"] as? String == "UserRequest")
        #expect(r["key"] as? String == "7")
        #expect(r["comment"] as? String == TicketWrites.comment)
        let add = ((r["fields"] as? [String: Any])?["public_log"] as? [String: Any])?["add_item"] as? [String: Any]
        #expect(add?["message"] as? String == "Raised the timeout.\nPlease test.")
        #expect(add?["format"] as? String == "text")
    }

    @Test("A private entry goes to private_log")
    func privateEntry() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "Internal note", isPrivate: true)
        let fields = try #require(stub.requests.first).jsonData["fields"] as? [String: Any]
        #expect(fields?["private_log"] != nil && fields?["public_log"] == nil)
    }

    @Test("Blank text is refused before anything is sent", arguments: ["", "   ", "\n\t"])
    func blank(_ text: String) async throws {
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.emptyEntry) {
            try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: text, isPrivate: false)
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("Surrounding blank lines are trimmed, inner line breaks kept")
    func trimmed() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "\n  Line one\nLine two  \n\n", isPrivate: false)
        let add = (((try #require(stub.requests.first).jsonData["fields"] as? [String: Any])?["public_log"] as? [String: Any])?["add_item"] as? [String: Any])
        #expect(add?["message"] as? String == "Line one\nLine two")
    }

    @Test("iTop rejecting this one object is an error, even when the call itself succeeds")
    func perObjectFailure() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: public_log is read-only for this profile"}}}"#))
        await #expect(throws: ITopError.server(code: 100, message: "Error: public_log is read-only for this profile")) {
            try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "Hello", isPrivate: false)
        }
    }
}

@Suite("Write safety")
struct WriteSafetyTests {
    @Test("With writes off, nothing is sent")
    func off() async throws {
        let stub = StubServer(ok)
        await #expect(throws: ITopError.writesDisabled) {
            try await TicketWrites(client: client(stub, writes: false)).addLogEntry(to: ticket, text: "Hello", isPrivate: false)
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("With an allowlist, only those tickets can be changed")
    func allowlist() async throws {
        let stub = StubServer(ok)
        let writes = TicketWrites(client: try client(stub, allow: ["UserRequest::7"]))
        try await writes.addLogEntry(to: ticket, text: "Allowed", isPrivate: false)

        let other = Ticket(key: "8", ticketClass: .userRequest, ref: "R-000008", title: "t", status: "assigned", priority: 3,
                           callerName: "", orgName: "", teamID: "0", teamName: "", agentID: "2", agentName: "", startDate: nil, lastUpdate: nil)
        await #expect(throws: ITopError.writesDisabled) { try await writes.addLogEntry(to: other, text: "Not allowed", isPrivate: false) }
        #expect(stub.requests.count == 1)
    }

    @Test("With an allowlist, creating new objects is refused")
    func allowlistNoCreate() async throws {
        let stub = StubServer(ok)
        let c = try client(stub, allow: ["UserRequest::7"])
        await #expect(throws: ITopError.writesDisabled) {
            _ = try await c.send(.create(className: "UserRequest", fields: [:], comment: "x"))
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("canWrite reports what the client would allow, for the composer")
    func canWrite() throws {
        let stub = StubServer(ok)
        #expect(try client(stub, writes: false).canWrite(className: "UserRequest", key: "7") == false)
        #expect(try client(stub).canWrite(className: "UserRequest", key: "7"))
        #expect(try client(stub, allow: ["UserRequest::7"]).canWrite(className: "UserRequest", key: "8") == false)
    }

    @Test("Allowlist entries can be refs, resolved to class and key by the app")
    func refs() {
        #expect(TicketWrites.allowlistKey(for: ticket) == "UserRequest::7")
    }
}

@Suite("Write access at sign-in")
struct WriteAccessTests {
    @Test("Refs given for testing become an allowlist of exact objects; unknown refs are left out")
    func refsResolve() async throws {
        let stub = StubServer { request in
            switch (request.jsonData["operation"] as? String, request.jsonData["class"] as? String) {
            case ("core/check_credentials", _): return .fixture("check_credentials")
            case ("core/get", "User"): return .fixture("user")
            case ("core/get", "Person"): return .fixture("person")
            case ("core/get", "UserRequest") where (request.jsonData["key"] as? String)?.contains("R-000007") == true:
                return .json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","fields":{"ref":"R-000007"}}}}"#)
            default: return .json(#"{"code":0,"message":"","objects":null}"#)
            }
        }
        let service = SessionService(secrets: MemoryStore(), settings: SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!),
                                     writeAccess: .tickets(["R-000007", "R-999999"])) { server, credentials in
            ITopClient(server: server, credentials: credentials, session: stub.session)
        }

        let session = try await service.signIn(server: "itop.example.com", user: "agent", password: "pw")

        #expect(session.client.allowsWrites)
        #expect(session.client.writeAllowlist == ["UserRequest::7"])
    }

    @Test("By default the session can't write at all")
    func offByDefault() async throws {
        let stub = StubServer { request in
            switch (request.jsonData["operation"] as? String, request.jsonData["class"] as? String) {
            case ("core/check_credentials", _): return .fixture("check_credentials")
            case ("core/get", "User"): return .fixture("user")
            default: return .fixture("person")
            }
        }
        let service = SessionService(secrets: MemoryStore(), settings: SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)) {
            ITopClient(server: $0, credentials: $1, session: stub.session)
        }
        let session = try await service.signIn(server: "itop.example.com", user: "agent", password: "pw")
        #expect(session.client.allowsWrites == false)
    }

    @Test("Writes on everywhere (release builds, from 2026-11-11): no allowlist, creating tickets allowed; iTop's own rights still apply")
    func allOn() async throws {
        let stub = StubServer { request in
            switch (request.jsonData["operation"] as? String, request.jsonData["class"] as? String) {
            case ("core/check_credentials", _): return .fixture("check_credentials")
            case ("core/get", "User"): return .fixture("user")
            default: return .fixture("person")
            }
        }
        let service = SessionService(secrets: MemoryStore(), settings: SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!),
                                     writeAccess: .all) {
            ITopClient(server: $0, credentials: $1, session: stub.session)
        }
        let session = try await service.signIn(server: "itop.example.com", user: "agent", password: "pw")
        #expect(session.client.allowsWrites)
        #expect(session.client.writeAllowlist == nil)
        #expect(session.client.canWrite(className: "Incident", key: "12345"))
        #expect(session.client.canCreateTickets)
    }
}

private final class MemoryStore: SecretStore, @unchecked Sendable {
    private let lock = NSLock()
    private var items: [String: String] = [:]
    func password(for account: String) throws -> String? { lock.withLock { items[account] } }
    func setPassword(_ password: String, for account: String) throws { lock.withLock { items[account] = password } }
    func deletePassword(for account: String) throws { lock.withLock { _ = items.removeValue(forKey: account) } }
}
