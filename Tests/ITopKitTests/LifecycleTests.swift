import Foundation
import Testing
@testable import ITopKit

private func t(_ status: String, cls: TicketClass = .userRequest) -> Ticket {
    Ticket(key: "7", ticketClass: cls, ref: "R-000007", title: "t", status: status, priority: 3,
           callerName: "", orgName: "Org A", teamID: "10", teamName: "", agentID: "2", agentName: "",
           startDate: nil, lastUpdate: nil, orgID: "4")
}

private func client(_ stub: StubServer, allow: Set<String>? = nil) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: true, writeAllowlist: allow)
}

private let ok = StubReply.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"","fields":{"id":"7"}}}}"#)

private let resolution = Resolution(serviceID: "3", subcategoryID: "12", code: "assistance", solution: "Raised the gateway timeout.")

@Suite("Lifecycle: which actions each state offers")
struct LifecycleTests {
    @Test("Actions per state, the same for requests and incidents", arguments: [
        ("new", [TicketAction.assign]),
        ("escalated_tto", [.assign]),
        ("assigned", [.reassign, .pending, .resolve]),
        ("escalated_ttr", [.reassign, .pending, .resolve]),
        ("pending", [.reassign, .resolve]),
        ("resolved", [.reopen, .close]),
        ("closed", []),
        ("waiting_for_approval", []),
    ] as [(String, [TicketAction])])
    func actions(status: String, expected: [TicketAction]) {
        #expect(TicketLifecycle.actions(for: status) == expected)
    }

    @Test("The main action: Assign when unowned, Resolve when being worked, Close when resolved", arguments: [
        ("new", TicketAction?.some(.assign)), ("escalated_tto", .assign), ("assigned", .resolve), ("escalated_ttr", .resolve),
        ("pending", .resolve), ("resolved", .close), ("closed", nil),
    ] as [(String, TicketAction?)])
    func primary(status: String, expected: TicketAction?) {
        #expect(TicketLifecycle.primary(for: status) == expected)
    }

    @Test("Each action's iTop stimulus")
    func stimuli() {
        #expect(TicketAction.allCases.map(\.stimulus) == ["ev_assign", "ev_reassign", "ev_pending", "ev_resolve", "ev_reopen", "ev_close"])
    }

    @Test("Task 7's assign rule agrees with the lifecycle", arguments: ["new", "escalated_tto", "assigned", "escalated_ttr", "pending", "resolved", "closed"])
    func assignAgrees(_ status: String) {
        let actions = TicketLifecycle.actions(for: status)
        let expected = actions.contains(.assign) ? "ev_assign" : actions.contains(.reassign) ? "ev_reassign" : nil
        #expect(TicketWrites.assignStimulus(for: status) == expected)
    }
}

@Suite("Lifecycle writes")
struct LifecycleWriteTests {
    @Test("Set to pending sends ev_pending with the reason")
    func pending() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).apply(.pending(reason: "  Waiting for the caller's laptop.  "), to: t("assigned"))
        let r = try #require(stub.requests.first).jsonData
        #expect(r["operation"] as? String == "core/apply_stimulus")
        #expect(r["stimulus"] as? String == "ev_pending")
        #expect(r["comment"] as? String == TicketWrites.comment)
        #expect((r["fields"] as? [String: Any])?["pending_reason"] as? String == "Waiting for the caller's laptop.")
    }

    @Test("Resolve sends service, subcategory, resolution code and solution with ev_resolve")
    func resolve() async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).apply(.resolve(resolution), to: t("escalated_ttr", cls: .incident))
        let r = try #require(stub.requests.first).jsonData
        #expect(r["class"] as? String == "Incident")
        #expect(r["stimulus"] as? String == "ev_resolve")
        let f = r["fields"] as? [String: Any]
        #expect(f?["service_id"] as? String == "3")
        #expect(f?["servicesubcategory_id"] as? String == "12")
        #expect(f?["resolution_code"] as? String == "assistance")
        #expect(f?["solution"] as? String == "<p>Raised the gateway timeout.</p>", "SPEC answer 7: the solution is HTML")
    }

    @Test("The solution keeps its paragraphs and line breaks, with markup escaped")
    func solutionHTML() async throws {
        let stub = StubServer(ok)
        var r = resolution
        r.solution = "Raised the timeout to 8 h.\nAsked the caller to test.\n\nClose if quiet <24h> & stable."
        try await TicketWrites(client: client(stub)).apply(.resolve(r), to: t("assigned"))
        let f = try #require(stub.requests.first).jsonData["fields"] as? [String: Any]
        #expect(f?["solution"] as? String == "<p>Raised the timeout to 8 h.<br>Asked the caller to test.</p><p>Close if quiet &lt;24h&gt; &amp; stable.</p>")
    }

    @Test("Reopen and close send their stimulus with no fields", arguments: [
        (TicketChange.reopen, "ev_reopen"), (.close, "ev_close"),
    ] as [(TicketChange, String)])
    func noFields(change: TicketChange, stimulus: String) async throws {
        let stub = StubServer(ok)
        try await TicketWrites(client: client(stub)).apply(change, to: t("resolved"))
        let r = try #require(stub.requests.first).jsonData
        #expect(r["stimulus"] as? String == stimulus)
        #expect((r["fields"] as? [String: Any])?.isEmpty == true)
    }

    @Test("Required fields are checked before anything is sent", arguments: [
        (TicketChange.pending(reason: " \n "), "reason"),
        (.resolve(Resolution(serviceID: "3", subcategoryID: "12", code: "assistance", solution: "  ")), "solution"),
        (.resolve(Resolution(serviceID: "", subcategoryID: "12", code: "assistance", solution: "Done")), "service"),
        (.resolve(Resolution(serviceID: "3", subcategoryID: "", code: "assistance", solution: "Done")), "subcategory"),
        (.resolve(Resolution(serviceID: "3 OR 1=1", subcategoryID: "12", code: "assistance", solution: "Done")), "service"),
        (.resolve(Resolution(serviceID: "3", subcategoryID: "12", code: "", solution: "Done")), "resolution code"),
    ] as [(TicketChange, String)])
    func required(change: TicketChange, field: String) async throws {
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.missing(field)) {
            try await TicketWrites(client: client(stub)).apply(change, to: t("assigned"))
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("An action the state doesn't offer is refused before anything is sent", arguments: [
        (TicketChange.pending(reason: "x"), "pending"), (.resolve(resolution), "resolved"), (.reopen, "assigned"), (.close, "closed"),
    ] as [(TicketChange, String)])
    func notOffered(change: TicketChange, status: String) async throws {
        let stub = StubServer(ok)
        await #expect(throws: TicketWrites.Failure.notAllowed(status: status)) {
            try await TicketWrites(client: client(stub)).apply(change, to: t(status))
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("iTop's own rejection comes back as its message")
    func rejected() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":100,"message":"Error: servicesubcategory_id: mandatory"}}}"#))
        await #expect(throws: ITopError.server(code: 100, message: "Error: servicesubcategory_id: mandatory")) {
            try await TicketWrites(client: client(stub)).apply(.resolve(resolution), to: t("assigned"))
        }
    }

    @Test("Lifecycle changes respect the write allowlist")
    func allowlist() async throws {
        let stub = StubServer(ok)
        await #expect(throws: ITopError.writesDisabled) {
            try await TicketWrites(client: client(stub, allow: ["UserRequest::99"])).apply(.close, to: t("resolved"))
        }
        #expect(stub.requests.isEmpty)
    }
}

@Suite("Service catalog for the Resolve sheet")
struct ServiceCatalogTests {
    @Test("OQL: services the org has a contract for, all live services as a fallback, and a service's subcategories")
    func oql() {
        #expect(ServiceQueries.services(forOrg: "4") ==
            "SELECT Service AS s JOIN lnkCustomerContractToService AS l ON l.service_id = s.id JOIN CustomerContract AS cc ON l.customercontract_id = cc.id WHERE cc.org_id = 4 AND s.status != 'obsolete'")
        #expect(ServiceQueries.allServices == "SELECT Service WHERE status != 'obsolete'")
        #expect(ServiceQueries.subcategories(ofService: "3") == "SELECT ServiceSubcategory WHERE service_id = 3 AND status != 'obsolete'")
        #expect(ServiceQueries.services(forOrg: "4 OR 1=1") == nil)
        #expect(ServiceQueries.subcategories(ofService: "") == nil)
    }

    @Test("When the org has no contracted services, every live service is offered")
    func fallback() async throws {
        let stub = StubServer { request in
            (request.jsonData["key"] as? String)?.contains("CustomerContract") == true
                ? .json(#"{"code":0,"message":"Found: 0","objects":null}"#)
                : .json(#"{"code":0,"message":"","objects":{"Service::3":{"class":"Service","key":"3","fields":{"friendlyname":"Network access"}}}}"#)
        }
        let services = try await ServiceCatalog(client: client(stub)).services(for: t("assigned"))
        #expect(services == [ServiceRef(id: "3", name: "Network access")])
        #expect(stub.requests.count == 2)
    }

    @Test("Standard resolution codes, labelled in words")
    func codes() {
        #expect(ResolutionCode.standard.map(\.id) == ["assistance", "bug fixed", "hardware repair", "other", "software patch", "system update", "training"])
        #expect(ResolutionCode.standard.first?.name == "Assistance")
        #expect(ResolutionCode(id: "bug fixed").name == "Bug fixed")
    }

    @Test("The detail now carries the service and subcategory ids, for prefilling Resolve")
    func detailIDs() throws {
        let object = try #require(ITopObject(json: try JSONDecoder().decode(JSONValue.self, from: Data(
            #"{"class":"UserRequest","key":"5","fields":{"service_id":"3","service_id_friendlyname":"Network access","servicesubcategory_id":"12","servicesubcategory_id_friendlyname":"VPN"}}"#.utf8))))
        let detail = TicketDetail(object: object)
        #expect(detail.serviceID == "3" && detail.subcategoryID == "12")
        #expect(TicketDetail.fields.contains("service_id") && TicketDetail.fields.contains("servicesubcategory_id"))
    }
}
