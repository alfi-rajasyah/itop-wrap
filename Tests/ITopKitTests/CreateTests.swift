import Foundation
import Testing
@testable import ITopKit

private func client(_ stub: StubServer, writes: Bool = true, allow: Set<String>? = nil, newTickets: Bool = false) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: writes, writeAllowlist: allow, allowsTicketCreation: newTickets)
}

private func draft(_ cls: TicketClass = .userRequest) -> NewTicket {
    NewTicket(ticketClass: cls, orgID: "4", callerID: "31", serviceID: "3", subcategoryID: "12",
              impact: "3", urgency: "2", title: "  VPN drops every hour  ", description: "Since Monday.\nBoth office & home <laptop>.")
}

private let createdReply = StubReply.json(#"{"code":0,"message":"","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"created","fields":{"id":"900"}}}}"#)
private let readBack = StubReply.json(#"{"code":0,"message":"Found: 1","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"","fields":{"ref":"R-000900","title":"VPN drops every hour","status":"new","org_id":"4","org_id_friendlyname":"Org A","caller_id_friendlyname":"Caller 1","agent_id":"0","team_id":"0","priority":"3"}}}}"#)

private func stub() -> StubServer {
    StubServer { r in r.jsonData["operation"] as? String == "core/create" ? createdReply : readBack }
}

@Suite("Create a ticket")
struct CreateTests {
    @Test("core/create sends every field, with the description as HTML that keeps line breaks and escapes markup")
    func create() async throws {
        let s = stub()
        let ticket = try await TicketWrites(client: client(s)).create(draft())

        let r = try #require(s.requests.first).jsonData
        #expect(r["operation"] as? String == "core/create")
        #expect(r["class"] as? String == "UserRequest")
        #expect(r["comment"] as? String == TicketWrites.comment)
        let f = try #require(r["fields"] as? [String: Any])
        #expect(f["org_id"] as? String == "4")
        #expect(f["caller_id"] as? String == "31")
        #expect(f["service_id"] as? String == "3")
        #expect(f["servicesubcategory_id"] as? String == "12")
        #expect(f["impact"] as? String == "3")
        #expect(f["urgency"] as? String == "2")
        #expect(f["title"] as? String == "VPN drops every hour")
        #expect(f["description"] as? String == "<p>Since Monday.<br>Both office &amp; home &lt;laptop&gt;.</p>")
        #expect(f["functionalcis_list"] == nil)
        #expect(ticket.ref == "R-000900", "the new ticket is read back so it can be opened")
        #expect(ticket.key == "900")
    }

    @Test("Created from a CI, the ticket is linked to it")
    func linkedCI() async throws {
        let s = stub()
        var d = draft(.incident)
        d.ciID = "55"
        _ = try await TicketWrites(client: client(s)).create(d)
        let r = try #require(s.requests.first).jsonData
        #expect(r["class"] as? String == "Incident")
        let list = (r["fields"] as? [String: Any])?["functionalcis_list"] as? [[String: Any]]
        #expect(list?.first?["functionalci_id"] as? String == "55")
    }

    @Test("Required fields are checked before anything is sent",
          arguments: ["organization", "caller", "service", "subcategory", "title", "description"])
    func required(name: String) async throws {
        let s = stub()
        var d = draft()
        switch name {
        case "organization": d.orgID = "  "
        case "caller": d.callerID = "  "
        case "service": d.serviceID = "  "
        case "subcategory": d.subcategoryID = "  "
        case "title": d.title = "  "
        default: d.description = "  "
        }
        await #expect(throws: TicketWrites.Failure.missing(name)) {
            _ = try await TicketWrites(client: client(s)).create(d)
        }
        #expect(s.requests.isEmpty)
    }

    @Test("Ids that aren't numbers never reach iTop")
    func badIDs() async throws {
        let s = stub()
        var d = draft()
        d.callerID = "31 OR 1=1"
        await #expect(throws: TicketWrites.Failure.missing("caller")) {
            _ = try await TicketWrites(client: client(s)).create(d)
        }
        #expect(s.requests.isEmpty)
    }

    @Test("With a per-ticket allowlist, new tickets are refused unless creating tickets is switched on too")
    func allowlist() async throws {
        let refused = stub()
        await #expect(throws: ITopError.writesDisabled) {
            _ = try await TicketWrites(client: client(refused, allow: ["UserRequest::7"])).create(draft())
        }
        #expect(refused.requests.isEmpty)

        let allowed = stub()
        _ = try await TicketWrites(client: client(allowed, allow: ["UserRequest::7"], newTickets: true)).create(draft())
        #expect(allowed.requests.first?.jsonData["operation"] as? String == "core/create")
    }

    @Test("A ticket created under the new-ticket switch joins the allowlist, so it can be worked on; others stay closed")
    func createdJoinsAllowlist() async throws {
        let s = StubServer { r in
            switch r.jsonData["operation"] as? String {
            case "core/create": return createdReply
            case "core/get": return readBack
            default: return .json(#"{"code":0,"message":"","objects":{"UserRequest::900":{"class":"UserRequest","key":"900","code":0,"message":"","fields":{"id":"900"}}}}"#)
            }
        }
        let c = try client(s, allow: [], newTickets: true)
        #expect(!c.canWrite(className: "UserRequest", key: "900"))
        let ticket = try await TicketWrites(client: c).create(draft())

        #expect(c.canWrite(className: "UserRequest", key: "900"))
        try await TicketWrites(client: c).addLogEntry(to: ticket, text: "Test entry", isPrivate: true)
        #expect(s.requests.last?.jsonData["operation"] as? String == "core/update")
        #expect(!c.canWrite(className: "UserRequest", key: "901"))
    }

    @Test("Creating tickets never works with writes off, whatever else is set")
    func writesOff() async throws {
        let s = stub()
        await #expect(throws: ITopError.writesDisabled) {
            _ = try await TicketWrites(client: client(s, writes: false, newTickets: true)).create(draft())
        }
        #expect(s.requests.isEmpty)
    }

    @Test("iTop refusing the ticket is an error with its message")
    func refused() async throws {
        let s = StubServer(.json(#"{"code":100,"message":"Error: servicesubcategory_id: not allowed for this request type","objects":null}"#))
        await #expect(throws: ITopError.self) {
            _ = try await TicketWrites(client: client(s)).create(draft())
        }
    }
}

@Suite("Pick lists for New ticket")
struct PickListTests {
    @Test("OQL: active organizations, and active people in one organization")
    func oql() {
        #expect(PickLists.organizationsOQL == "SELECT Organization WHERE status = 'active'")
        #expect(PickLists.callersOQL(orgID: "4") == "SELECT Person WHERE org_id = 4 AND status = 'active'")
        #expect(PickLists.callersOQL(orgID: "4 OR 1=1") == nil)
    }

    @Test("Organizations and callers come from iTop, sorted by name")
    func lists() async throws {
        let s = StubServer { r in
            r.jsonData["class"] as? String == "Organization"
                ? .json(#"{"code":0,"message":"","objects":{"Organization::2":{"class":"Organization","key":"2","fields":{"friendlyname":"Org B"}},"Organization::1":{"class":"Organization","key":"1","fields":{"friendlyname":"Org A"}}}}"#)
                : .json(#"{"code":0,"message":"","objects":{"Person::9":{"class":"Person","key":"9","fields":{"friendlyname":"Zed Caller"}},"Person::8":{"class":"Person","key":"8","fields":{"friendlyname":"Ann Caller"}}}}"#)
        }
        let lists = PickLists(client: try client(s))
        #expect(try await lists.organizations().map(\.name) == ["Org A", "Org B"])
        #expect(try await lists.callers(inOrg: "1").map(\.name) == ["Ann Caller", "Zed Caller"])
    }

    @Test("Impact and urgency use iTop's standard values, with its defaults")
    func levels() {
        #expect(PickLists.impacts.map(\.id) == ["1", "2", "3"])
        #expect(PickLists.impacts.map(\.name) == ["A department", "A service", "A person"])
        #expect(PickLists.urgencies.map(\.name) == ["Critical", "High", "Medium", "Low"])
        #expect(NewTicket(ticketClass: .userRequest).impact == "1")
        #expect(NewTicket(ticketClass: .userRequest).urgency == "4")
    }
}
