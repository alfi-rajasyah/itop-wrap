import Foundation

/// Which tickets a list covers over time: open ones by default, optionally only those opened
/// recently, and optionally resolved and closed ones too. Including resolved and closed always
/// keeps a date range (30 days by default): iTop's OQL can't order results, so an unbounded
/// history couldn't be cut to the most recent ones.
public struct TicketScope: Sendable, Equatable, Hashable {
    public let includeClosed: Bool
    public let openedWithinDays: Int?

    public init(includeClosed: Bool, openedWithinDays: Int?) {
        self.includeClosed = includeClosed
        self.openedWithinDays = includeClosed ? (openedWithinDays ?? 30) : openedWithinDays
    }

    public static let open = TicketScope(includeClosed: false, openedWithinDays: nil)

    /// The OQL condition. The date test runs on the server's clock, so time zones don't matter.
    public var clause: String {
        var parts: [String] = []
        if !includeClosed { parts.append(TicketQueries.open) }
        if let days = openedWithinDays { parts.append("start_date >= DATE_SUB(NOW(), INTERVAL \(days) DAY)") }
        return parts.joined(separator: " AND ")
    }
}

/// The order inside each status group (the sort menu).
public enum TicketSort: String, Sendable, CaseIterable, Identifiable {
    case priority, lastUpdate, newest, oldest

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .priority: "Priority, then oldest"
        case .lastUpdate: "Last update, newest first"
        case .newest: "Opened, newest first"
        case .oldest: "Opened, oldest first"
        }
    }

    func before(_ a: Ticket, _ b: Ticket) -> Bool {
        let past = Date.distantPast, future = Date.distantFuture
        switch self {
        case .priority: return (a.priority, a.startDate ?? future, a.ref) < (b.priority, b.startDate ?? future, b.ref)
        case .lastUpdate: return (a.lastUpdate ?? past, b.ref) > (b.lastUpdate ?? past, a.ref)
        case .newest: return (a.startDate ?? past, b.ref) > (b.startDate ?? past, a.ref)
        case .oldest: return (a.startDate ?? future, a.ref) < (b.startDate ?? future, b.ref)
        }
    }
}

/// The OQL behind each list (PLAN.md Task 3; the "needs you" rule from DESIGN.md).
public enum TicketQueries {
    static let open = "status NOT IN ('resolved','closed')"

    /// Tickets assigned to me, plus unassigned Escalated TTO tickets in my teams.
    public static func mine(_ cls: TicketClass, me: CurrentUser, scope: TicketScope = .open) -> String {
        let assigned = "agent_id = \(me.contactID)"
        guard !me.teams.isEmpty else { return "SELECT \(cls.rawValue) WHERE \(scope.clause) AND \(assigned)" }
        return "SELECT \(cls.rawValue) WHERE \(scope.clause) AND (\(assigned) OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (\(teamIDs(me)))))"
    }

    /// Everything in my teams, or `nil` when I'm in no team.
    public static func team(_ cls: TicketClass, me: CurrentUser, scope: TicketScope = .open) -> String? {
        guard !me.teams.isEmpty else { return nil }
        return "SELECT \(cls.rawValue) WHERE \(scope.clause) AND team_id IN (\(teamIDs(me)))"
    }

    /// For a full ref like "R-000512", the class and the query that finds it; otherwise `nil`.
    public static func refLookup(_ text: String) -> (TicketClass, String)? {
        let ref = text.trimmingCharacters(in: .whitespaces).uppercased()
        guard ref.wholeMatch(of: /[RI]-\d{6}/) != nil,
              let cls = TicketClass.allCases.first(where: { ref.hasPrefix($0.refPrefix + "-") }) else { return nil }
        return (cls, "SELECT \(cls.rawValue) WHERE ref = \(OQL.quote(ref))")
    }

    private static func teamIDs(_ me: CurrentUser) -> String {
        me.teams.compactMap { Int($0.id) }.map(String.init).joined(separator: ",")
    }
}

/// Loads the lists. Read-only.
public struct TicketService: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    public func loadMine(me: CurrentUser, scope: TicketScope = .open) async throws -> [Ticket] {
        try await load(TicketClass.allCases.map { ($0, TicketQueries.mine($0, me: me, scope: scope)) })
    }

    public func loadTeam(me: CurrentUser, scope: TicketScope = .open) async throws -> [Ticket] {
        try await load(TicketClass.allCases.compactMap { cls in TicketQueries.team(cls, me: me, scope: scope).map { (cls, $0) } })
    }

    /// A ticket by full ref, for search; `nil` if it doesn't exist or this account can't see it.
    public func lookup(_ ref: String) async throws -> Ticket? {
        guard let (cls, oql) = TicketQueries.refLookup(ref) else { return nil }
        return try await load([(cls, oql)]).first
    }

    private func load(_ queries: [(TicketClass, String)]) async throws -> [Ticket] {
        try await withThrowingTaskGroup(of: [Ticket].self) { group in
            for (cls, oql) in queries {
                group.addTask {
                    let response = try await client.send(.get(className: cls.rawValue, key: oql, outputFields: Ticket.listFields))
                    return response.objects.compactMap(Ticket.init(object:))
                }
            }
            return try await group.reduce(into: []) { $0 += $1 }
        }
    }
}

public enum TicketGroup: String, Sendable, CaseIterable {
    case escalatedTTR, escalatedTTO, new, assigned, pending, other, resolved, closed

    public var title: String {
        switch self {
        case .escalatedTTR: "Escalated TTR"
        case .escalatedTTO: "Escalated TTO"
        case .new: "New"
        case .assigned: "Assigned"
        case .pending: "Pending"
        case .other: "Other"
        case .resolved: "Resolved"
        case .closed: "Closed"
        }
    }

    static func of(_ status: String) -> TicketGroup {
        switch status {
        case "escalated_ttr": .escalatedTTR
        case "escalated_tto": .escalatedTTO
        case "new": .new
        case "assigned": .assigned
        case "pending": .pending
        case "resolved": .resolved
        case "closed": .closed
        default: .other
        }
    }
}

public struct TicketSection: Sendable, Equatable, Identifiable {
    public var id: TicketGroup { group }
    public let group: TicketGroup
    public let tickets: [Ticket]
}

public enum TicketList {
    public enum View: Sendable { case mine, team }

    /// "Needs you" (DESIGN.md): my Escalated TTR, or an unassigned Escalated TTO in one of my teams.
    public static func needsYou(_ t: Ticket, me: CurrentUser) -> Bool {
        switch t.status {
        case "escalated_ttr": return t.agentID == me.contactID
        case "escalated_tto": return t.isUnassigned && me.teams.contains { $0.id == t.teamID }
        default: return false
        }
    }

    /// Sections in DESIGN.md order, escalated first; empty ones are left out.
    /// Inside a section: priority 1 first, then the oldest.
    public static func groups(_ tickets: [Ticket], me: CurrentUser, view: View, sort: TicketSort = .priority) -> [TicketSection] {
        let order: [TicketGroup] = view == .team
            ? [.escalatedTTR, .escalatedTTO, .new, .assigned, .pending, .other, .resolved, .closed]
            : [.escalatedTTR, .escalatedTTO, .assigned, .new, .pending, .other, .resolved, .closed]
        let byGroup = Dictionary(grouping: tickets) { TicketGroup.of($0.status) }
        return order.compactMap { g in
            guard let list = byGroup[g], !list.isEmpty else { return nil }
            return TicketSection(group: g, tickets: list.sorted(by: sort.before))
        }
    }
}

public enum TicketSearch {
    /// The org filter: no orgs chosen means everything.
    public static func filter(_ tickets: [Ticket], orgs: Set<String>) -> [Ticket] {
        orgs.isEmpty ? tickets : tickets.filter { orgs.contains($0.orgName) }
    }

    /// The orgs the filter menu offers: the ones these tickets are for.
    public static func orgs(in tickets: [Ticket]) -> [String] {
        Set(tickets.map(\.orgName).filter { !$0.isEmpty }).sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    /// Every word must appear in the ref or the title. Order is kept.
    public static func filter(_ tickets: [Ticket], query: String) -> [Ticket] {
        let words = query.lowercased().split(separator: " ").map(String.init)
        guard !words.isEmpty else { return tickets }
        return tickets.filter { t in
            let haystack = (t.ref + " " + t.title).lowercased()
            return words.allSatisfy { haystack.contains($0) }
        }
    }
}

public enum Age {
    /// "now", "40m", "2h", "9d", as in the wireframe's list rows.
    public static func short(from date: Date, now: Date = Date()) -> String {
        let s = max(0, now.timeIntervalSince(date))
        switch s {
        case ..<60: return "now"
        case ..<3600: return "\(Int(s / 60))m"
        case ..<86400: return "\(Int(s / 3600))h"
        default: return "\(Int(s / 86400))d"
        }
    }
}
