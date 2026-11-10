import Foundation

public struct Organization: Sendable, Equatable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let code: String
    /// Open tickets you can see for this org.
    public let openCount: Int

    public init(id: String, name: String, code: String, openCount: Int) {
        self.id = id
        self.name = name
        self.code = code
        self.openCount = openCount
    }
}

/// A Person from iTop. Someone in at least one team counts as an agent (DESIGN.md: People).
public struct Person: Sendable, Equatable, Identifiable, Hashable {
    public let id: String
    public let displayName: String
    public let orgID: String
    public let orgName: String
    public let email: String
    public let phone: String
    public let function: String
    public let teams: [Team]

    public init(id: String, displayName: String, orgID: String, orgName: String, email: String, phone: String,
                function: String, teams: [Team]) {
        self.id = id
        self.displayName = displayName
        self.orgID = orgID
        self.orgName = orgName
        self.email = email
        self.phone = phone
        self.function = function
        self.teams = teams
    }

    public var isAgent: Bool { !teams.isEmpty }

    /// "Agent · Network, Service desk" or "Caller · Org A".
    public var subtitle: String {
        isAgent ? "Agent · " + teams.map(\.name).joined(separator: ", ") : "Caller" + (orgName.isEmpty ? "" : " · \(orgName)")
    }

    public var initials: String {
        displayName.split(separator: " ").prefix(2).compactMap(\.first).map { String($0).uppercased() }.joined()
    }

    static let fields = ["name", "first_name", "org_id", "org_id_friendlyname", "email", "phone", "function", "team_list"]

    init(object: ITopObject) {
        let f = object.fields
        func text(_ k: String) -> String { f[k]?.stringValue ?? "" }
        var teams: [Team] = []
        if case .array(let links)? = f["team_list"] {
            teams = links.compactMap { l in
                l["team_id"]?.intValue.map { Team(id: String($0), name: l["team_name"]?.stringValue ?? "Team \($0)") }
            }
        }
        let name = [text("first_name"), text("name")].filter { !$0.isEmpty }.joined(separator: " ")
        self.init(id: object.key, displayName: name.isEmpty ? "Person \(object.key)" : name,
                  orgID: f["org_id"]?.intValue.map(String.init) ?? "0", orgName: text("org_id_friendlyname"),
                  email: text("email"), phone: text("phone"), function: text("function"), teams: teams)
    }
}

/// The OQL behind Organizations and People (PLAN.md Task 5b).
public enum OrgQueries {
    public static let organizations = "SELECT Organization WHERE status = 'active'"
    static let open = TicketQueries.open

    public static func openTickets(_ cls: TicketClass, orgID: String, scope: TicketScope = .open) -> String? {
        Int(orgID).map { "SELECT \(cls.rawValue) WHERE \(scope.clause) AND org_id = \($0)" }
    }

    public static func people(orgID: String) -> String? {
        Int(orgID).map { "SELECT Person WHERE org_id = \($0)" }
    }

    public static func configItems(orgID: String) -> String? {
        CMDBQueries.inOrg(orgID)
    }

    /// Every word must appear in the first or last name; `nil` under 2 characters.
    public static func peopleSearch(_ text: String) -> String? {
        let q = text.trimmingCharacters(in: .whitespaces)
        guard q.count >= 2 else { return nil }
        let clauses = q.split(separator: " ").map { word in
            let w = OQL.quote("%\(word)%")
            return "(name LIKE \(w) OR first_name LIKE \(w))"
        }
        return "SELECT Person WHERE " + clauses.joined(separator: " AND ")
    }

    public static func assignedTo(_ cls: TicketClass, personID: String, scope: TicketScope = .open) -> String? {
        Int(personID).map { "SELECT \(cls.rawValue) WHERE \(scope.clause) AND agent_id = \($0)" }
    }

    public static func reportedBy(_ cls: TicketClass, personID: String, scope: TicketScope = .open) -> String? {
        Int(personID).map { "SELECT \(cls.rawValue) WHERE \(scope.clause) AND caller_id = \($0)" }
    }
}

/// Organizations and People reads. Read-only.
public struct OrgService: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    /// The orgs your account can see, with open-ticket counts. iTop's API can't count, so this loads
    /// every open ticket you can see with only its org id: one light query per ticket class.
    public func organizations() async throws -> [Organization] {
        async let orgs = client.send(.get(className: "Organization", key: OrgQueries.organizations, outputFields: ["name", "code"]))
        var counts: [String: Int] = [:]
        for cls in TicketClass.allCases {
            let r = try await client.send(.get(className: cls.rawValue, key: "SELECT \(cls.rawValue) WHERE \(TicketQueries.open)",
                                               outputFields: ["org_id"]))
            for o in r.objects { counts[o.fields["org_id"]?.intValue.map(String.init) ?? "0", default: 0] += 1 }
        }
        return try await orgs.objects.map { o in
            Organization(id: o.key, name: o.fields["name"]?.stringValue ?? "Organization \(o.key)",
                         code: o.fields["code"]?.stringValue ?? "", openCount: counts[o.key] ?? 0)
        }
        .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    /// An org's open tickets from every agent.
    public func openTickets(orgID: String, scope: TicketScope = .open) async throws -> [Ticket] {
        try await tickets(TicketClass.allCases.compactMap { cls in OrgQueries.openTickets(cls, orgID: orgID, scope: scope).map { (cls, $0) } })
    }

    /// How many people and CIs an org has, for the org header's links.
    public func counts(orgID: String) async throws -> (people: Int, configItems: Int) {
        guard let p = OrgQueries.people(orgID: orgID), let c = OrgQueries.configItems(orgID: orgID) else { return (0, 0) }
        async let people = client.send(.get(className: "Person", key: p, outputFields: ["id"]))
        async let cis = client.send(.get(className: "FunctionalCI", key: c, outputFields: ["id"]))
        return try await (people.objects.count, cis.objects.count)
    }

    public func people(matching text: String) async throws -> [Person] {
        guard let oql = OrgQueries.peopleSearch(text) else { return [] }
        return try await people(oql)
    }

    public func people(inOrg orgID: String) async throws -> [Person] {
        guard let oql = OrgQueries.people(orgID: orgID) else { return [] }
        return try await people(oql)
    }

    public func tickets(assignedTo personID: String, scope: TicketScope = .open) async throws -> [Ticket] {
        try await tickets(TicketClass.allCases.compactMap { cls in OrgQueries.assignedTo(cls, personID: personID, scope: scope).map { (cls, $0) } })
    }

    public func tickets(reportedBy personID: String, scope: TicketScope = .open) async throws -> [Ticket] {
        try await tickets(TicketClass.allCases.compactMap { cls in OrgQueries.reportedBy(cls, personID: personID, scope: scope).map { (cls, $0) } })
    }

    static let peopleLimit = 100

    private func people(_ oql: String) async throws -> [Person] {
        let r = try await client.send(.get(className: "Person", key: oql, outputFields: Person.fields, limit: Self.peopleLimit))
        return r.objects.prefix(Self.peopleLimit).map(Person.init(object:))
            .sorted { $0.displayName.localizedStandardCompare($1.displayName) == .orderedAscending }
    }

    private func tickets(_ queries: [(TicketClass, String)]) async throws -> [Ticket] {
        var all: [Ticket] = []
        for (cls, oql) in queries {
            let r = try await client.send(.get(className: cls.rawValue, key: oql, outputFields: Ticket.listFields))
            all += r.objects.compactMap(Ticket.init(object:))
        }
        return all
    }
}
