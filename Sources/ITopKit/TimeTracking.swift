import Foundation

// Time spent (PLAN.md Task 8b; SPEC.md Server facts: TimeSpent → Activity → ticket).
// The app keeps its own timer and saves one TimeSpent per stop. A web stopwatch that's
// running is a TimeSpent of yours with no end date (probe 2026-11-11), so the app shows it
// instead of starting a second timer.

/// The one timer that's running, saved so it survives quitting the app.
public struct RunningTimer: Codable, Sendable, Equatable {
    public let ticketClass: String
    public let ticketKey: String
    public let ref: String
    public let title: String
    public let orgID: String
    public let start: Date

    public init(ticket: Ticket, start: Date) {
        ticketClass = ticket.ticketClass.rawValue
        ticketKey = ticket.key
        ref = ticket.ref
        title = ticket.title
        orgID = ticket.orgID
        self.start = start
    }

    public var ticketID: Ticket.ID { "\(ticketClass)::\(ticketKey)" }
}

/// What one stop saves.
public struct TimerEntry: Sendable, Equatable {
    public let start: Date
    public let end: Date
    public let duration: Int
    /// The timer ran past the limit and was cut at it.
    public let capped: Bool

    public init(start: Date, end: Date, duration: Int, capped: Bool) {
        self.start = start
        self.end = end
        self.duration = duration
        self.capped = capped
    }

    /// iTop's stopwatch stops itself after a while (4 hours by default in the extension's
    /// docs); the app stops at the same point so the two agree.
    public static let limit: TimeInterval = 4 * 3600

    public static func stop(_ timer: RunningTimer, at now: Date, limit: TimeInterval = limit) -> TimerEntry {
        let elapsed = max(0, now.timeIntervalSince(timer.start))
        let capped = elapsed > limit
        let seconds = Int(min(elapsed, limit))
        return TimerEntry(start: timer.start, end: timer.start.addingTimeInterval(TimeInterval(seconds)), duration: seconds, capped: capped)
    }

    /// A stop under a minute is most likely a misclick, so it isn't saved.
    public var isTooShort: Bool { duration < 60 }
}

public enum TimeFormat {
    /// "0:42", "1:05", "10:05": hours and minutes.
    public static func clock(_ seconds: Int) -> String {
        let minutes = max(0, seconds) / 60
        return "\(minutes / 60):" + String(format: "%02d", minutes % 60)
    }

    static func itop(_ date: Date) -> String { formatter.string(from: date) }

    private static let formatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm:ss"
        f.timeZone = .current
        return f
    }()
}

/// Where the running timer is kept between launches.
public struct TimerStore: @unchecked Sendable {
    // UserDefaults is thread-safe; @unchecked because older SDKs don't mark it Sendable.
    let defaults: UserDefaults
    static let key = "timer.running"

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func load() -> RunningTimer? {
        guard let data = defaults.data(forKey: Self.key) else { return nil }
        return try? JSONDecoder().decode(RunningTimer.self, from: data)
    }

    public func save(_ timer: RunningTimer?) {
        if let timer, let data = try? JSONEncoder().encode(timer) {
            defaults.set(data, forKey: Self.key)
        } else {
            defaults.removeObject(forKey: Self.key)
        }
    }
}

public struct TimeTracking: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    static func label(_ ticket: Ticket) -> String { "\(ticket.ref) \(ticket.title)" }

    /// The ticket's Activity id, or nil when it has none yet.
    func activityID(for ticket: Ticket) async throws -> String? {
        guard TicketWrites.isID(ticket.key) else { return nil }
        let response = try await client.send(.get(className: "Activity",
                                                  key: "SELECT Activity WHERE obj_class = '\(ticket.ticketClass.rawValue)' AND obj_id = \(ticket.key)",
                                                  outputFields: ["obj_id"], limit: nil))
        return response.objects.first?.key
    }

    /// Saves one stop. Creates the ticket's Activity first when it has none (your choice, 2026-11-11).
    public func save(_ entry: TimerEntry, on ticket: Ticket, me: CurrentUser) async throws {
        let link = TicketWrites.allowlistKey(for: ticket)
        var activity = try await activityID(for: ticket)
        if activity == nil {
            let response = try await client.send(.create(className: "Activity", fields: [
                "obj_class": .string(ticket.ticketClass.rawValue), "obj_id": .string(ticket.key), "label": .string(Self.label(ticket)),
            ], comment: TicketWrites.comment), onTicket: link)
            try TicketWrites.check(response)
            activity = response.objects.first?.key
        }
        guard let activity, TicketWrites.isID(activity) else {
            throw ITopError.server(code: 0, message: "iTop didn't return the ticket's time-tracking activity.")
        }
        let response = try await client.send(.create(className: "TimeSpent", fields: [
            "activity_id": .string(activity), "user_id": .string(me.userID), "contact_id": .string(me.contactID),
            "org_id": .string(ticket.orgID), "start_date": .string(TimeFormat.itop(entry.start)),
            "end_date": .string(TimeFormat.itop(entry.end)), "duration": .string(String(entry.duration)),
            "title": .string(Self.label(ticket)),
        ], comment: TicketWrites.comment), onTicket: link)
        try TicketWrites.check(response)
    }

    /// When a web stopwatch of yours is running on the ticket: when it started. Nil otherwise.
    public func runningStopwatch(on ticket: Ticket, me: CurrentUser) async throws -> Date? {
        guard let activity = try await activityID(for: ticket), TicketWrites.isID(me.userID) else { return nil }
        let response = try await client.send(.get(className: "TimeSpent",
                                                  key: "SELECT TimeSpent WHERE activity_id = \(activity) AND user_id = \(me.userID)",
                                                  outputFields: ["start_date", "end_date", "duration"], limit: nil))
        let open = response.objects.last { ($0.fields["end_date"]?.stringValue ?? "").isEmpty }
        return open.flatMap { Ticket.parseDate($0.fields["start_date"]?.stringValue ?? "") }
    }

    /// Total seconds spent per ticket, everyone's time included. Tickets with no Activity are left out.
    public func totals(for tickets: [Ticket]) async throws -> [Ticket.ID: Int] {
        var result: [Ticket.ID: Int] = [:]
        for cls in TicketClass.allCases {
            let keys = tickets.filter { $0.ticketClass == cls && TicketWrites.isID($0.key) }.map(\.key)
            guard !keys.isEmpty else { continue }
            let activities = try await client.send(.get(className: "Activity",
                                                        key: "SELECT Activity WHERE obj_class = '\(cls.rawValue)' AND obj_id IN (\(keys.joined(separator: ",")))",
                                                        outputFields: ["obj_id"], limit: nil)).objects
            var ticketFor: [String: Ticket.ID] = [:]
            for a in activities {
                guard let obj = a.fields["obj_id"]?.stringValue ?? a.fields["obj_id"]?.intValue.map(String.init) else { continue }
                ticketFor[a.key] = "\(cls.rawValue)::\(obj)"
                result["\(cls.rawValue)::\(obj)"] = result["\(cls.rawValue)::\(obj)"] ?? 0
            }
            let ids = activities.map(\.key).filter(TicketWrites.isID)
            guard !ids.isEmpty else { continue }
            let spent = try await client.send(.get(className: "TimeSpent", key: "SELECT TimeSpent WHERE activity_id IN (\(ids.joined(separator: ",")))",
                                                   outputFields: ["activity_id", "duration"], limit: nil)).objects
            for s in spent {
                guard let a = s.fields["activity_id"]?.stringValue, let id = ticketFor[a] else { continue }
                let seconds = Int(s.fields["duration"]?.stringValue ?? "") ?? s.fields["duration"]?.intValue ?? 0
                result[id, default: 0] += seconds
            }
        }
        return result
    }
}
