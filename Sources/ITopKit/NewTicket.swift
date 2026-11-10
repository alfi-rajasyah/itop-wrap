import Foundation

/// What New ticket sends (PLAN.md Task 9; UX-FLOWS F7: organization first).
public struct NewTicket: Sendable, Equatable {
    public var ticketClass: TicketClass
    public var orgID: String
    public var callerID: String
    public var serviceID: String
    public var subcategoryID: String
    public var impact: String
    public var urgency: String
    public var title: String
    public var description: String
    /// Set when the ticket starts from a config item ("New ticket on this CI").
    public var ciID: String?

    /// Impact and urgency start at iTop's own defaults (1 and 4), like its web form.
    public init(ticketClass: TicketClass, orgID: String = "", callerID: String = "", serviceID: String = "", subcategoryID: String = "",
                impact: String = "1", urgency: String = "4", title: String = "", description: String = "", ciID: String? = nil) {
        self.ticketClass = ticketClass
        self.orgID = orgID
        self.callerID = callerID
        self.serviceID = serviceID
        self.subcategoryID = subcategoryID
        self.impact = impact
        self.urgency = urgency
        self.title = title
        self.description = description
        self.ciID = ciID
    }

    /// The description as iTop's HTML: markup escaped, paragraphs kept, line breaks as <br>.
    static func html(_ text: String) -> String {
        let escaped = text.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "\"", with: "&quot;")
        return escaped.components(separatedBy: "\n\n")
            .map { "<p>" + $0.replacingOccurrences(of: "\n", with: "<br>") + "</p>" }
            .joined()
    }
}

extension TicketWrites {
    /// Creates the ticket and reads it back, so the app can open it.
    public func create(_ t: NewTicket) async throws -> Ticket {
        for (value, name) in [(t.orgID, "organization"), (t.callerID, "caller"), (t.serviceID, "service"), (t.subcategoryID, "subcategory")]
            where !Self.isID(value) { throw Failure.missing(name) }
        let title = t.title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else { throw Failure.missing("title") }
        let description = t.description.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !description.isEmpty else { throw Failure.missing("description") }
        var fields: [String: JSONValue] = [
            "org_id": .string(t.orgID), "caller_id": .string(t.callerID), "service_id": .string(t.serviceID),
            "servicesubcategory_id": .string(t.subcategoryID), "impact": .string(t.impact), "urgency": .string(t.urgency),
            "title": .string(title), "description": .string(NewTicket.html(description)),
        ]
        if let ci = t.ciID, Self.isID(ci) {
            fields["functionalcis_list"] = .array([.object(["functionalci_id": .string(ci)])])
        }
        let response = try await client.send(.create(className: t.ticketClass.rawValue, fields: fields, comment: Self.comment))
        try Self.check(response)
        guard let key = response.objects.first?.key, Self.isID(key) else {
            throw ITopError.server(code: 0, message: "iTop didn't return the new ticket.")
        }
        let read = try await client.send(.get(className: t.ticketClass.rawValue, key: "SELECT \(t.ticketClass.rawValue) WHERE id = \(key)",
                                              outputFields: Ticket.listFields, limit: 1))
        guard let object = read.objects.first, let ticket = Ticket(object: object) else {
            throw ITopError.server(code: 0, message: "The ticket was created, but the app couldn't read it back. Refresh to see it.")
        }
        return ticket
    }
}

/// Lists for New ticket, read from iTop. Impact and urgency can't be read through REST, so they
/// use iTop's standard values; a customised list shows up as iTop refusing the value.
public struct PickLists: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    public static let organizationsOQL = "SELECT Organization WHERE status = 'active'"

    public static func callersOQL(orgID: String) -> String? {
        TicketWrites.isID(orgID) ? "SELECT Person WHERE org_id = \(orgID) AND status = 'active'" : nil
    }

    public static let impacts = [ServiceRef(id: "1", name: "A department"), ServiceRef(id: "2", name: "A service"), ServiceRef(id: "3", name: "A person")]
    public static let urgencies = [ServiceRef(id: "1", name: "Critical"), ServiceRef(id: "2", name: "High"),
                                   ServiceRef(id: "3", name: "Medium"), ServiceRef(id: "4", name: "Low")]

    public func organizations() async throws -> [ServiceRef] {
        try await refs("Organization", Self.organizationsOQL)
    }

    public func callers(inOrg orgID: String) async throws -> [ServiceRef] {
        guard let oql = Self.callersOQL(orgID: orgID) else { return [] }
        return try await refs("Person", oql)
    }

    private func refs(_ className: String, _ oql: String) async throws -> [ServiceRef] {
        let response = try await client.send(.get(className: className, key: oql, outputFields: ["friendlyname"], limit: nil))
        return response.objects.map { ServiceRef(id: $0.key, name: $0.fields["friendlyname"]?.stringValue ?? "\(className) \($0.key)") }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}

extension ServiceCatalog {
    /// The services an organization's contracts cover, every live service when there are none.
    public func services(forOrg orgID: String) async throws -> [ServiceRef] {
        let probe = Ticket(key: "0", ticketClass: .userRequest, ref: "", title: "", status: "", priority: 0, callerName: "", orgName: "",
                           teamID: "0", teamName: "", agentID: "0", agentName: "", startDate: nil, lastUpdate: nil, orgID: orgID)
        return try await services(for: probe)
    }
}
