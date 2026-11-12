import Foundation

/// What the menu bar item says (PLAN.md Task 8c; DESIGN.md: Menu bar item): the running timer,
/// then the Needs you count, each only when there is one.
public struct MenuBarStatus: Sendable, Equatable {
    public let needCount: Int
    public let timer: RunningTimer?

    public init(needCount: Int, timer: RunningTimer?) {
        self.needCount = needCount
        self.timer = timer
    }

    /// The panel's groups, escalated first.
    public static let groupOrder: [TicketGroup] = [.escalatedTTR, .escalatedTTO, .assigned, .pending]

    func elapsed(_ timer: RunningTimer, now: Date) -> String {
        TimeFormat.clock(Int(max(0, now.timeIntervalSince(timer.start))))
    }

    /// The title ticks by the second (`00:12:40`), like the window's counter, so a running timer
    /// visibly runs; VoiceOver keeps hours and minutes.
    public func title(now: Date = Date()) -> String {
        var parts: [String] = []
        if let timer { parts.append(TimeFormat.counter(Int(max(0, now.timeIntervalSince(timer.start))))) }
        if needCount > 0 { parts.append(String(needCount)) }
        return parts.joined(separator: "  ")
    }

    public func accessibilityLabel(now: Date = Date()) -> String {
        var text = "iTop Wrap: "
        switch needCount {
        case 0: text += "nothing needs you"
        case 1: text += "1 ticket needs you"
        default: text += "\(needCount) tickets need you"
        }
        if let timer { text += ", timer \(elapsed(timer, now: now)) on \(timer.ref)" }
        return text
    }
}
