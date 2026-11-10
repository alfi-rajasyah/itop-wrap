import Foundation

/// Changes to tickets (PLAN.md Phase 3). Every one goes through `ITopClient`, which refuses it
/// unless writes are on and, with an allowlist, the ticket is on it.
public struct TicketWrites: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    /// Shown in the ticket's history next to the change, so it's traceable (SPEC.md, Boundaries).
    public static let comment = "iTop Wrap"

    public enum Failure: Error, Equatable {
        case emptyEntry
        /// The ticket's state has no assign or reassign transition.
        case cannotAssign(status: String)
        /// A team or agent id that isn't a plain number.
        case invalidChoice
        /// Assign to me found the ticket already taken by someone else (UX-FLOWS F4).
        case alreadyTaken(by: String)
        /// Assign to me needs one of your teams.
        case noTeam
        /// A required field is empty (or not a valid id): "reason", "service", "subcategory", "resolution code", "solution".
        case missing(String)
        /// The ticket's state doesn't offer this action.
        case notAllowed(status: String)
    }

    public static func allowlistKey(for ticket: Ticket) -> String {
        "\(ticket.ticketClass.rawValue)::\(ticket.key)"
    }

    /// Appends an entry to the public or private log. Text format keeps the line breaks.
    public func addLogEntry(to ticket: Ticket, text: String, isPrivate: Bool) async throws {
        let message = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { throw Failure.emptyEntry }
        let field = isPrivate ? "private_log" : "public_log"
        try await update(ticket, fields: [field: .object(["add_item": .object(["message": .string(message), "format": .string("text")])])])
    }

    /// The stimulus that sets team and agent for a ticket in this state: unowned tickets are
    /// assigned, owned ones reassigned. Nil when the state has neither (resolved, closed).
    public static func assignStimulus(for status: String) -> String? {
        let actions = TicketLifecycle.actions(for: status)
        if actions.contains(.assign) { return TicketAction.assign.stimulus }
        if actions.contains(.reassign) { return TicketAction.reassign.stimulus }
        return nil
    }

    /// Sets the ticket's team and agent with one stimulus.
    public func assign(_ ticket: Ticket, teamID: String, agentID: String) async throws {
        guard let stimulus = Self.assignStimulus(for: ticket.status) else { throw Failure.cannotAssign(status: ticket.status) }
        guard Self.isID(teamID), Self.isID(agentID) else { throw Failure.invalidChoice }
        let response = try await client.send(.applyStimulus(className: ticket.ticketClass.rawValue, key: ticket.key, stimulus: stimulus,
                                                            fields: ["team_id": .string(teamID), "agent_id": .string(agentID)],
                                                            comment: Self.comment))
        try Self.check(response)
    }

    /// Assigns the ticket to you: its own team when you're in it, otherwise your first team.
    /// Reads the ticket first so a colleague who just took it keeps it.
    public func assignToMe(_ ticket: Ticket, me: CurrentUser) async throws {
        guard let team = me.teams.first(where: { $0.id == ticket.teamID }) ?? me.teams.first else { throw Failure.noTeam }
        guard Self.isID(ticket.key) else { throw Failure.invalidChoice }
        let latest = try await client.send(.get(className: ticket.ticketClass.rawValue,
                                               key: "SELECT \(ticket.ticketClass.rawValue) WHERE id = \(ticket.key)",
                                               outputFields: ["agent_id", "agent_id_friendlyname", "status"], limit: 1))
        var current = ticket
        if let object = latest.objects.first {
            let agent = object.fields["agent_id"]?.stringValue ?? "0"
            if agent != "0", !agent.isEmpty, agent != me.contactID {
                throw Failure.alreadyTaken(by: object.fields["agent_id_friendlyname"]?.stringValue ?? "someone else")
            }
            if let status = object.fields["status"]?.stringValue, !status.isEmpty {
                current = Ticket(key: ticket.key, ticketClass: ticket.ticketClass, ref: ticket.ref, title: ticket.title, status: status,
                                 priority: ticket.priority, callerName: ticket.callerName, orgName: ticket.orgName,
                                 teamID: ticket.teamID, teamName: ticket.teamName, agentID: agent, agentName: ticket.agentName,
                                 startDate: ticket.startDate, lastUpdate: ticket.lastUpdate, orgID: ticket.orgID)
            }
        }
        try await assign(current, teamID: team.id, agentID: me.contactID)
    }

    public static func isID(_ value: String) -> Bool {
        !value.isEmpty && value.allSatisfy { $0.isASCII && $0.isNumber }
    }

    static func check(_ response: ITopResponse) throws {
        if let object = response.objects.first, object.code != 0 {
            throw ITopError.server(code: object.code, message: object.message)
        }
    }

    /// Runs a `core/update` and checks iTop's verdict on the object itself: the call can
    /// succeed while the object was refused.
    func update(_ ticket: Ticket, fields: [String: JSONValue]) async throws {
        let response = try await client.send(.update(className: ticket.ticketClass.rawValue, key: ticket.key,
                                                     fields: fields, comment: Self.comment))
        try Self.check(response)
    }
}

/// Someone a ticket can be assigned to.
public struct Agent: Sendable, Equatable, Hashable, Identifiable {
    public let id: String
    public let name: String

    public init(id: String, name: String) {
        self.id = id
        self.name = name
    }
}

public enum TeamQueries {
    public static let teams = "SELECT Team WHERE status = 'active'"

    /// Active people linked to the team. Nil for an id that isn't a number.
    public static func agents(inTeam teamID: String) -> String? {
        guard TicketWrites.isID(teamID) else { return nil }
        return "SELECT Person AS p JOIN lnkPersonToTeam AS l ON l.person_id = p.id WHERE l.team_id = \(teamID) AND p.status = 'active'"
    }
}

/// Reads teams and their agents for the Assign sheet's pickers.
public struct TeamDirectory: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    public func teams() async throws -> [Team] {
        let response = try await client.send(.get(className: "Team", key: TeamQueries.teams, outputFields: ["friendlyname"], limit: nil))
        return response.objects.map { Team(id: $0.key, name: $0.fields["friendlyname"]?.stringValue ?? "Team \($0.key)") }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    public func agents(inTeam teamID: String) async throws -> [Agent] {
        guard let oql = TeamQueries.agents(inTeam: teamID) else { return [] }
        let response = try await client.send(.get(className: "Person", key: oql, outputFields: ["friendlyname"], limit: nil))
        return response.objects.map { Agent(id: $0.key, name: $0.fields["friendlyname"]?.stringValue ?? "Person \($0.key)") }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}
