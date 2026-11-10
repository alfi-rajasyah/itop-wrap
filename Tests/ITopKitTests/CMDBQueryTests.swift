import Foundation
import Testing
@testable import ITopKit

private func client(_ stub: StubServer) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "agent", password: "pw"), session: stub.session)
}

private func json(_ text: String) throws -> JSONValue {
    try JSONDecoder().decode(JSONValue.self, from: Data(text.utf8))
}

// MARK: - Search

@Suite("CMDB search")
struct CMDBSearchTests {
    @Test("Search matches names that contain the words, safely quoted")
    func oql() {
        #expect(CMDBQueries.search("vpn") == "SELECT FunctionalCI WHERE name LIKE '%vpn%'")
        #expect(CMDBQueries.search("  o'neil ") == #"SELECT FunctionalCI WHERE name LIKE '%o\'neil%'"#)
    }

    @Test("Fewer than 2 characters doesn't search", arguments: ["", " ", "v"])
    func tooShort(_ q: String) {
        #expect(CMDBQueries.search(q) == nil)
    }

    @Test("Results show their real class and come sorted by name, with a cap on how many are asked for")
    func decode() async throws {
        let stub = StubServer(.json(#"""
        {"code":0,"message":"","objects":{
          "FunctionalCI::2":{"class":"FunctionalCI","key":"2","code":0,"message":"","fields":{"name":"vpn-gw-02","finalclass":"NetworkDevice","org_id_friendlyname":"Org A"}},
          "FunctionalCI::1":{"class":"FunctionalCI","key":"1","code":0,"message":"","fields":{"name":"Remote access VPN","finalclass":"ApplicationSolution","org_id_friendlyname":"Org A"}}}}
        """#))

        let items = try await CMDBService(client: client(stub)).search("vpn")

        #expect(items.map(\.name) == ["Remote access VPN", "vpn-gw-02"])
        #expect(items.first?.className == "ApplicationSolution")
        #expect(items.first?.classLabel == "Application solution")
        let request = try #require(stub.requests.first).jsonData
        #expect(request["limit"] as? String == "\(CMDBQueries.searchLimit)")
    }
}

// MARK: - Detail fields

@Suite("CI detail")
struct CIDetailTests {
    @Test("The real Server sample gives its organization and criticality, and skips empty fields")
    func fields() throws {
        let data = try Fixture.data("ci")
        let object = try #require(ITopResponse(body: try JSONDecoder().decode(JSONValue.self, from: data)).objects.first)

        let fields = ConfigItemDetail.fields(from: object.fields)

        #expect(fields.map(\.label).contains("Organization"))
        #expect(fields.map(\.label).contains("Business criticality"))
        #expect(!fields.map(\.label).contains("In production since"), "move2production is empty in the sample")
    }

    @Test("Codes read as words", arguments: [("production", "Production"), ("implementation", "Implementation"), ("high", "High"), ("not_applicable", "Not applicable")] as [(String, String)])
    func labels(code: String, label: String) {
        #expect(ConfigItemDetail.humanize(code) == label)
    }

    @Test("Contacts come from contacts_list")
    func contacts() throws {
        let fields = ConfigItemDetail.fields(from: [
            "contacts_list": try json(#"[{"contact_id_friendlyname":"Team Network"},{"contact_id_friendlyname":"Agent 4"}]"#),
            "status": .string("production"),
        ])
        #expect(fields.first { $0.label == "Contacts" }?.value == "Team Network, Agent 4")
        #expect(fields.first { $0.label == "Status" }?.value == "Production")
    }
}

// MARK: - Relationships

@Suite("CI relationships")
struct CIRelationTests {
    @Test("core/get_related asks for direct impacts in the given direction")
    func operation() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{},"relations":{}}"#))
        _ = try await CMDBService(client: client(stub)).related(className: "Server", key: "5", direction: .up)

        let r = try #require(stub.requests.first).jsonData
        #expect(r["operation"] as? String == "core/get_related")
        #expect(r["relation"] as? String == "impacts")
        #expect(r["direction"] as? String == "up")
        #expect(r["depth"] as? Int == 1)
        #expect(RESTOperation.getRelated(className: "Server", key: "5", relation: "impacts", depth: 1, direction: "up").isWrite == false)
    }

    @Test("Related objects exclude the CI itself and anything that isn't a CI, sorted by name")
    func parse() throws {
        let body = try json(#"""
        {"code":0,"message":"","objects":{
          "Server::5":{"class":"Server","key":"5","fields":{"friendlyname":"srv-01"}},
          "ApplicationSolution::9":{"class":"ApplicationSolution","key":"9","fields":{"friendlyname":"Remote access VPN"}},
          "VirtualMachine::3":{"class":"VirtualMachine","key":"3","fields":{"name":"app-vm-03"}},
          "Person::2":{"class":"Person","key":"2","fields":{"friendlyname":"Agent 4"}}},
         "relations":{"Server::5":[{"key":"ApplicationSolution::9"},{"key":"VirtualMachine::3"}]}}
        """#)

        let refs = CMDBService.relatedItems(ITopResponse(body: body), excluding: "Server", key: "5")

        #expect(refs == [CIRef(id: "3", name: "app-vm-03", className: "VirtualMachine"),
                         CIRef(id: "9", name: "Remote access VPN", className: "ApplicationSolution")])
    }
}

// MARK: - Open tickets on a CI

@Suite("CI open tickets")
struct CITicketTests {
    @Test("Open tickets come through the ticket-to-CI link table, one query per ticket class")
    func oql() {
        #expect(CMDBQueries.openTickets(.userRequest, ciID: "55") ==
            "SELECT UserRequest AS t JOIN lnkFunctionalCIToTicket AS l ON l.ticket_id = t.id WHERE l.functionalci_id = 55 AND t.status NOT IN ('resolved','closed')")
    }

    @Test("A non-numeric id never reaches the query")
    func unsafeID() {
        #expect(CMDBQueries.openTickets(.incident, ciID: "5 OR 1=1") == nil)
    }
}
