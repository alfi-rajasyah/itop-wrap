import Foundation
import Testing
@testable import ITopKit

private func client(_ stub: StubServer) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "agent", password: "pw"), session: stub.session)
}

private func objects(_ cls: String, _ rows: [(String, [String: String])]) -> StubReply {
    let body = rows.map { key, fields in
        let f = fields.map { "\"\($0.key)\":\"\($0.value)\"" }.joined(separator: ",")
        return "\"\(cls)::\(key)\":{\"class\":\"\(cls)\",\"key\":\"\(key)\",\"code\":0,\"message\":\"\",\"fields\":{\(f)}}"
    }.joined(separator: ",")
    return .json(#"{"code":0,"message":"","objects":{\#(body)}}"#)
}

// MARK: - Organizations

@Suite("Organizations")
struct OrganizationTests {
    @Test("OQL for the list, an org's open tickets, and its people and CIs")
    func oql() {
        #expect(OrgQueries.organizations == "SELECT Organization WHERE status = 'active'")
        #expect(OrgQueries.openTickets(.incident, orgID: "7") == "SELECT Incident WHERE status NOT IN ('resolved','closed') AND org_id = 7")
        #expect(OrgQueries.people(orgID: "7") == "SELECT Person WHERE org_id = 7")
        #expect(OrgQueries.configItems(orgID: "7") == "SELECT FunctionalCI WHERE org_id = 7")
        #expect(OrgQueries.openTickets(.userRequest, orgID: "7 OR 1=1") == nil)
    }

    @Test("Organizations come with their open-ticket counts, sorted by name; counts only cover orgs iTop listed")
    func counts() async throws {
        let stub = StubServer { request in
            switch request.jsonData["class"] as? String {
            case "Organization": return objects("Organization", [("2", ["name": "Org B", "code": "B"]), ("1", ["name": "Org A", "code": "A"])])
            case "UserRequest": return objects("UserRequest", [("10", ["org_id": "1"]), ("11", ["org_id": "1"]), ("12", ["org_id": "2"])])
            case "Incident": return objects("Incident", [("20", ["org_id": "1"]), ("21", ["org_id": "99"])])
            default: return .json(#"{"code":11,"message":"x"}"#)
            }
        }

        let orgs = try await OrgService(client: client(stub)).organizations()

        #expect(orgs.map(\.name) == ["Org A", "Org B"])
        #expect(orgs.map(\.openCount) == [3, 1])
        let ticketFields = stub.requests.first { $0.jsonData["class"] as? String == "UserRequest" }?.jsonData["output_fields"] as? String
        #expect(ticketFields == "org_id", "counting asks for nothing but the org")
    }
}

// MARK: - People

@Suite("People")
struct PeopleTests {
    @Test("Each word has to match the first or last name")
    func searchOQL() {
        #expect(OrgQueries.peopleSearch("Jane doe") ==
            "SELECT Person WHERE (name LIKE '%Jane%' OR first_name LIKE '%Jane%') AND (name LIKE '%doe%' OR first_name LIKE '%doe%')")
        #expect(OrgQueries.peopleSearch("o'neil") == #"SELECT Person WHERE (name LIKE '%o\'neil%' OR first_name LIKE '%o\'neil%')"#)
    }

    @Test("Fewer than 2 characters doesn't search", arguments: ["", "j", "  "])
    func tooShort(_ q: String) {
        #expect(OrgQueries.peopleSearch(q) == nil)
    }

    @Test("Your real Person sample decodes: teams make an agent, and the phone stays cleaned")
    func fixture() async throws {
        let stub = StubServer(.fixture("person"))
        let people = try await OrgService(client: client(stub)).people(matching: "agent")

        let p = try #require(people.first)
        #expect(p.id == "1002")
        #expect(p.isAgent)
        #expect(p.teams.count == 1)
        #expect(p.displayName == "[text] [text]")
        #expect(p.phone == "[number]")
    }

    @Test("Someone in no team is a caller")
    func caller() {
        let p = Person(id: "3", displayName: "Caller One", orgID: "1", orgName: "Org A", email: "", phone: "", function: "", teams: [])
        #expect(!p.isAgent)
        #expect(p.subtitle == "Caller · Org A")
        #expect(p.initials == "CO")
    }

    @Test("An agent's subtitle names their teams")
    func agentSubtitle() {
        let p = Person(id: "4", displayName: "Agent Four", orgID: "1", orgName: "Org A", email: "", phone: "", function: "",
                       teams: [Team(id: "1", name: "Network"), Team(id: "2", name: "Service desk")])
        #expect(p.subtitle == "Agent · Network, Service desk")
    }

    @Test("Tickets assigned to someone and tickets they reported")
    func personTickets() {
        #expect(OrgQueries.assignedTo(.userRequest, personID: "4") == "SELECT UserRequest WHERE status NOT IN ('resolved','closed') AND agent_id = 4")
        #expect(OrgQueries.reportedBy(.incident, personID: "4") == "SELECT Incident WHERE status NOT IN ('resolved','closed') AND caller_id = 4")
    }
}

// MARK: - Org filter on My tickets

@Suite("Org filter")
struct OrgFilterTests {
    func t(_ ref: String, org: String) -> Ticket {
        Ticket(key: ref, ticketClass: .userRequest, ref: ref, title: "t", status: "assigned", priority: 3, callerName: "",
               orgName: org, teamID: "0", teamName: "", agentID: "2", agentName: "", startDate: nil, lastUpdate: nil)
    }

    @Test("No orgs chosen means everything; otherwise only the chosen orgs")
    func filter() {
        let tickets = [t("R-1", org: "Org A"), t("R-2", org: "Org B"), t("R-3", org: "Org C")]
        #expect(TicketSearch.filter(tickets, orgs: []).map(\.ref) == ["R-1", "R-2", "R-3"])
        #expect(TicketSearch.filter(tickets, orgs: ["Org A", "Org C"]).map(\.ref) == ["R-1", "R-3"])
    }

    @Test("The menu offers the orgs your tickets are for, sorted")
    func choices() {
        #expect(TicketSearch.orgs(in: [t("R-1", org: "Org B"), t("R-2", org: "Org A"), t("R-3", org: "Org B"), t("R-4", org: "")]) == ["Org A", "Org B"])
    }

    @Test("Tickets now carry their org id, for opening the org view")
    func orgID() throws {
        let object = try #require(ITopObject(json: try JSONDecoder().decode(JSONValue.self, from: Data(
            #"{"class":"UserRequest","key":"5","fields":{"ref":"R-5","org_id":"14","org_id_friendlyname":"Org A"}}"#.utf8))))
        #expect(Ticket(object: object)?.orgID == "14")
        #expect(Ticket.listFields.contains("org_id"))
    }
}

// MARK: - CIs of an org

@Suite("CMDB by org")
struct CMDBByOrgTests {
    @Test("An org's CIs use the same result shape as search")
    func inOrg() async throws {
        let stub = StubServer(objects("FunctionalCI", [("1", ["name": "srv-01", "finalclass": "Server", "org_id_friendlyname": "Org A"])]))
        let items = try await CMDBService(client: client(stub)).items(inOrg: "7")
        #expect(items.map(\.className) == ["Server"])
        #expect(stub.requests.first?.jsonData["key"] as? String == "SELECT FunctionalCI WHERE org_id = 7")
    }
}
