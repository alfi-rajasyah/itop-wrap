import Foundation
import Testing
@testable import ITopKit

private let start = Ticket.parseDate("2026-11-11 09:30:00")!

private func timer() -> RunningTimer {
    RunningTimer(ticket: Ticket(key: "412", ticketClass: .userRequest, ref: "R-000412", title: "VPN drops", status: "assigned",
                                priority: 3, callerName: "", orgName: "", teamID: "10", teamName: "", agentID: "2", agentName: "",
                                startDate: nil, lastUpdate: nil),
                 start: start)
}

@Suite("Menu bar status")
struct MenuBarTests {
    @Test("The item shows the running timer, then the needs-you count, each only when there is one", arguments: [
        (0, false, ""), (2, false, "2"), (0, true, "00:12:40"), (3, true, "00:12:40  3"),
    ] as [(Int, Bool, String)])
    func title(count: Int, running: Bool, expected: String) {
        let status = MenuBarStatus(needCount: count, timer: running ? timer() : nil)
        #expect(status.title(now: start.addingTimeInterval(12 * 60 + 40)) == expected)
    }

    @Test("VoiceOver reads it as one sentence", arguments: [
        (2, true, "iTop Wrap: 2 tickets need you, timer 0:12 on R-000412"),
        (1, false, "iTop Wrap: 1 ticket needs you"),
        (0, false, "iTop Wrap: nothing needs you"),
        (0, true, "iTop Wrap: nothing needs you, timer 0:12 on R-000412"),
    ] as [(Int, Bool, String)])
    func spoken(count: Int, running: Bool, expected: String) {
        let status = MenuBarStatus(needCount: count, timer: running ? timer() : nil)
        #expect(status.accessibilityLabel(now: start.addingTimeInterval(12 * 60 + 40)) == expected)
    }

    @Test("The panel's groups put escalated first, as DESIGN.md's menu bar table says")
    func order() {
        #expect(MenuBarStatus.groupOrder == [.escalatedTTR, .escalatedTTO, .assigned, .pending])
    }
}
