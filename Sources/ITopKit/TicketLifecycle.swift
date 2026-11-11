import Foundation

/// What can be done to a ticket, each with its iTop stimulus (PLAN.md Task 8).
public enum TicketAction: String, CaseIterable, Sendable {
    case assign, reassign, pending, resolve, reopen, close

    public var stimulus: String {
        switch self {
        case .assign: return "ev_assign"
        case .reassign: return "ev_reassign"
        case .pending: return "ev_pending"
        case .resolve: return "ev_resolve"
        case .reopen: return "ev_reopen"
        case .close: return "ev_close"
        }
    }
}

/// The standard iTop request/incident lifecycle, as offered in the app. Your iTop has some
/// customisation, so iTop still has the last word: a refused stimulus comes back as its message.
public enum TicketLifecycle {
    public static func actions(for status: String) -> [TicketAction] {
        switch status {
        case "new", "escalated_tto": return [.assign]
        case "assigned", "escalated_ttr": return [.reassign, .pending, .resolve]
        case "pending": return [.reassign, .resolve]
        case "resolved": return [.reopen, .close]
        default: return []
        }
    }

    /// The one action shown as a button in the header (DESIGN.md: Ticket detail).
    public static func primary(for status: String) -> TicketAction? {
        let actions = actions(for: status)
        for candidate in [TicketAction.assign, .resolve, .reopen] where actions.contains(candidate) { return candidate }
        return nil
    }
}

/// What Resolve sends, matching iTop's "Mark as resolved" form. No time field.
public struct Resolution: Sendable, Equatable {
    public var serviceID: String
    public var subcategoryID: String
    public var code: String
    public var solution: String

    public init(serviceID: String, subcategoryID: String, code: String, solution: String) {
        self.serviceID = serviceID
        self.subcategoryID = subcategoryID
        self.code = code
        self.solution = solution
    }
}

/// A lifecycle change other than assigning (Task 7 has its own call).
public enum TicketChange: Sendable, Equatable {
    case pending(reason: String)
    case resolve(Resolution)
    case reopen
    case close

    public var action: TicketAction {
        switch self {
        case .pending: return .pending
        case .resolve: return .resolve
        case .reopen: return .reopen
        case .close: return .close
        }
    }
}

extension TicketWrites {
    /// Runs one lifecycle change. Required fields and the state are checked first, so nothing
    /// is sent for a change iTop would have to refuse.
    public func apply(_ change: TicketChange, to ticket: Ticket) async throws {
        guard TicketLifecycle.actions(for: ticket.status).contains(change.action) else {
            throw Failure.notAllowed(status: ticket.status)
        }
        var fields: [String: JSONValue] = [:]
        switch change {
        case .pending(let reason):
            let text = reason.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { throw Failure.missing("reason") }
            fields["pending_reason"] = .string(text)
        case .resolve(let r):
            guard Self.isID(r.serviceID) else { throw Failure.missing("service") }
            guard Self.isID(r.subcategoryID) else { throw Failure.missing("subcategory") }
            guard !r.code.isEmpty else { throw Failure.missing("resolution code") }
            let solution = r.solution.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !solution.isEmpty else { throw Failure.missing("solution") }
            fields = ["service_id": .string(r.serviceID), "servicesubcategory_id": .string(r.subcategoryID),
                      "resolution_code": .string(r.code), "solution": .string(NewTicket.html(solution))]  // HTML (SPEC answer 7)
        case .reopen, .close:
            break
        }
        let response = try await client.send(.applyStimulus(className: ticket.ticketClass.rawValue, key: ticket.key,
                                                            stimulus: change.action.stimulus, fields: fields, comment: Self.comment))
        try Self.check(response)
    }
}

// MARK: - Services and resolution codes

/// A service or service subcategory, for the Resolve sheet's pickers.
public struct ServiceRef: Sendable, Equatable, Hashable, Identifiable {
    public let id: String
    public let name: String

    public init(id: String, name: String) {
        self.id = id
        self.name = name
    }
}

public enum ServiceQueries {
    /// Services the ticket's organization has a customer contract for.
    public static func services(forOrg orgID: String) -> String? {
        guard TicketWrites.isID(orgID) else { return nil }
        return "SELECT Service AS s JOIN lnkCustomerContractToService AS l ON l.service_id = s.id JOIN CustomerContract AS cc ON l.customercontract_id = cc.id WHERE cc.org_id = \(orgID) AND s.status != 'obsolete'"
    }

    public static let allServices = "SELECT Service WHERE status != 'obsolete'"

    public static func subcategories(ofService serviceID: String) -> String? {
        guard TicketWrites.isID(serviceID) else { return nil }
        return "SELECT ServiceSubcategory WHERE service_id = \(serviceID) AND status != 'obsolete'"
    }
}

public struct ServiceCatalog: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    /// The org's contracted services, or every live service when it has none (or no contracts are kept).
    public func services(for ticket: Ticket) async throws -> [ServiceRef] {
        if let oql = ServiceQueries.services(forOrg: ticket.orgID) {
            let found = try await refs("Service", oql)
            if !found.isEmpty { return found }
        }
        return try await refs("Service", ServiceQueries.allServices)
    }

    public func subcategories(ofService serviceID: String) async throws -> [ServiceRef] {
        guard let oql = ServiceQueries.subcategories(ofService: serviceID) else { return [] }
        return try await refs("ServiceSubcategory", oql)
    }

    private func refs(_ className: String, _ oql: String) async throws -> [ServiceRef] {
        let response = try await client.send(.get(className: className, key: oql, outputFields: ["friendlyname"], limit: nil))
        var seen = Set<String>()
        return response.objects
            .filter { seen.insert($0.key).inserted }
            .map { ServiceRef(id: $0.key, name: $0.fields["friendlyname"]?.stringValue ?? "\(className) \($0.key)") }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}

/// iTop's standard resolution codes. Read through REST there's no list of allowed values, so a
/// customised list shows up as iTop refusing the code, with its message.
public struct ResolutionCode: Sendable, Equatable, Hashable, Identifiable {
    public let id: String

    public init(id: String) {
        self.id = id
    }

    public var name: String { id.prefix(1).uppercased() + id.dropFirst() }

    public static let standard = ["assistance", "bug fixed", "hardware repair", "other", "software patch", "system update", "training"]
        .map(ResolutionCode.init)
}
