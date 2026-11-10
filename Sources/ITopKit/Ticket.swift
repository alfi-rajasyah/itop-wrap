import Foundation

public enum TicketClass: String, Sendable, CaseIterable {
    case userRequest = "UserRequest"
    case incident = "Incident"

    /// The prefix iTop uses for this class's refs: R-000412, I-000088.
    public var refPrefix: String { self == .userRequest ? "R" : "I" }
}

/// One open ticket as the list shows it. Values come straight from iTop; `status` is the
/// iTop code ("assigned", "escalated_ttr"), and ids are strings, "0" meaning empty.
public struct Ticket: Sendable, Equatable, Identifiable, Hashable {
    public var id: String { "\(ticketClass.rawValue)::\(key)" }
    public let key: String
    public let ticketClass: TicketClass
    public let ref: String
    public let title: String
    public let status: String
    public let priority: Int
    public let callerName: String
    public let orgName: String
    public let orgID: String
    public let teamID: String
    public let teamName: String
    public let agentID: String
    public let agentName: String
    public let startDate: Date?
    public let lastUpdate: Date?

    public init(key: String, ticketClass: TicketClass, ref: String, title: String, status: String, priority: Int,
                callerName: String, orgName: String, teamID: String, teamName: String, agentID: String,
                agentName: String, startDate: Date?, lastUpdate: Date?, orgID: String = "0") {
        self.key = key
        self.ticketClass = ticketClass
        self.ref = ref
        self.title = title
        self.status = status
        self.priority = priority
        self.callerName = callerName
        self.orgName = orgName
        self.orgID = orgID
        self.teamID = teamID
        self.teamName = teamName
        self.agentID = agentID
        self.agentName = agentName
        self.startDate = startDate
        self.lastUpdate = lastUpdate
    }

    public var isUnassigned: Bool { agentID == "0" || agentID.isEmpty }

    /// The fields the list needs, so iTop doesn't send logs and descriptions for every row.
    static let listFields = ["ref", "title", "status", "priority", "caller_id_friendlyname", "org_id", "org_id_friendlyname",
                             "team_id", "team_id_friendlyname", "agent_id", "agent_id_friendlyname",
                             "start_date", "last_update"]

    init?(object: ITopObject) {
        guard let cls = TicketClass(rawValue: object.className) else { return nil }
        let f = object.fields
        func text(_ k: String) -> String { f[k]?.stringValue ?? "" }
        func ident(_ k: String) -> String { f[k]?.intValue.map(String.init) ?? "0" }
        self.init(key: object.key, ticketClass: cls, ref: text("ref"), title: text("title"), status: text("status"),
                  priority: f["priority"]?.intValue ?? 4, callerName: text("caller_id_friendlyname"),
                  orgName: text("org_id_friendlyname"), teamID: ident("team_id"), teamName: text("team_id_friendlyname"),
                  agentID: ident("agent_id"), agentName: text("agent_id_friendlyname"),
                  startDate: Self.parseDate(text("start_date")), lastUpdate: Self.parseDate(text("last_update")),
                  orgID: ident("org_id"))
    }

    /// iTop sends dates as "2026-11-09 11:00:32" without a time zone, in the server's zone.
    /// They're read in the Mac's zone, which is right when both are set the same way.
    public static func parseDate(_ text: String) -> Date? {
        guard !text.isEmpty else { return nil }
        return dateFormatter.date(from: text)
    }

    private static let dateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm:ss"
        f.timeZone = .current
        return f
    }()
}
