import Foundation
import Testing
@testable import ITopKit

private let me = CurrentUser(userID: "1", contactID: "2", login: "a", displayName: "A", profiles: [], teams: [Team(id: "10", name: "N")])

private func t(_ ref: String, status: String = "assigned", prio: Int = 3, opened: String, updated: String) -> Ticket {
    Ticket(key: ref, ticketClass: .userRequest, ref: ref, title: ref, status: status, priority: prio, callerName: "", orgName: "",
           teamID: "10", teamName: "", agentID: "2", agentName: "", startDate: Ticket.parseDate(opened), lastUpdate: Ticket.parseDate(updated))
}

@Suite("Ticket scope (date range)")
struct TicketScopeTests {
    @Test("Default: open tickets, any time")
    func open() {
        #expect(TicketScope.open.clause == "status NOT IN ('resolved','closed')")
    }

    @Test("Open tickets opened in the last 30 days")
    func openRecent() {
        #expect(TicketScope(includeClosed: false, openedWithinDays: 30).clause ==
            "status NOT IN ('resolved','closed') AND start_date >= DATE_SUB(NOW(), INTERVAL 30 DAY)")
    }

    @Test("Including resolved and closed drops the status test")
    func withClosed() {
        #expect(TicketScope(includeClosed: true, openedWithinDays: 7).clause == "start_date >= DATE_SUB(NOW(), INTERVAL 7 DAY)")
    }

    @Test("All tickets, any time: a condition that's always true, so the queries stay valid OQL")
    func everything() {
        let all = TicketScope(includeClosed: true, openedWithinDays: nil)
        #expect(all.openedWithinDays == nil)
        #expect(all.clause == "id > 0")
        #expect(OrgQueries.openTickets(.userRequest, orgID: "5", scope: all) == "SELECT UserRequest WHERE id > 0 AND org_id = 5")
    }

    @Test("The scope reaches every list query", arguments: [true, false])
    func reachesQueries(_ closed: Bool) {
        let scope = TicketScope(includeClosed: closed, openedWithinDays: 7)
        for oql in [TicketQueries.mine(.userRequest, me: me, scope: scope),
                    TicketQueries.team(.userRequest, me: me, scope: scope)!,
                    OrgQueries.openTickets(.userRequest, orgID: "5", scope: scope)!,
                    OrgQueries.assignedTo(.incident, personID: "4", scope: scope)!,
                    OrgQueries.reportedBy(.incident, personID: "4", scope: scope)!] {
            #expect(oql.contains("INTERVAL 7 DAY"))
            #expect(oql.contains("status NOT IN") == !closed)
        }
    }

    @Test("My tickets keeps the unassigned-TTO-in-my-teams part with any scope")
    func mineKeepsTTO() {
        #expect(TicketQueries.mine(.userRequest, me: me, scope: TicketScope(includeClosed: true, openedWithinDays: 7)) ==
            "SELECT UserRequest WHERE start_date >= DATE_SUB(NOW(), INTERVAL 7 DAY) AND (agent_id = 2 OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (10)))")
    }

    @Test("Resolved and closed tickets get their own groups, after the open ones")
    func closedGroups() {
        let groups = TicketList.groups([t("R-1", status: "closed", opened: "2026-11-01 09:00:00", updated: "2026-11-02 09:00:00"),
                                        t("R-2", status: "resolved", opened: "2026-11-01 09:00:00", updated: "2026-11-02 09:00:00"),
                                        t("R-3", status: "pending", opened: "2026-11-01 09:00:00", updated: "2026-11-02 09:00:00")],
                                       me: me, view: .mine)
        #expect(groups.map(\.group) == [.pending, .resolved, .closed])
    }
}

@Suite("Ticket sort")
struct TicketSortTests {
    let tickets = [
        t("R-1", prio: 3, opened: "2026-11-01 09:00:00", updated: "2026-11-09 09:00:00"),
        t("R-2", prio: 1, opened: "2026-11-05 09:00:00", updated: "2026-11-06 09:00:00"),
        t("R-3", prio: 3, opened: "2026-10-20 09:00:00", updated: "2026-11-10 09:00:00"),
    ]

    @Test("Each sort orders tickets inside a group", arguments: [
        (TicketSort.priority, ["R-2", "R-3", "R-1"]),
        (TicketSort.lastUpdate, ["R-3", "R-1", "R-2"]),
        (TicketSort.newest, ["R-2", "R-1", "R-3"]),
        (TicketSort.oldest, ["R-3", "R-1", "R-2"]),
    ] as [(TicketSort, [String])])
    func orders(sort: TicketSort, expected: [String]) {
        #expect(TicketList.groups(tickets, me: me, view: .mine, sort: sort).first?.tickets.map(\.ref) == expected)
    }

    @Test("Sorting never changes the status grouping")
    func keepsGroups() {
        let mixed = tickets + [t("R-4", status: "escalated_ttr", prio: 4, opened: "2026-11-10 08:00:00", updated: "2026-11-10 08:00:00")]
        #expect(TicketList.groups(mixed, me: me, view: .mine, sort: .lastUpdate).map(\.group) == [.escalatedTTR, .assigned])
    }
}

@Suite("Ticket paging")
struct TicketPagingTests {
    let me = CurrentUser(userID: "1", contactID: "2", login: "a", displayName: "A", profiles: [], teams: [])

    func tickets(_ ttr: Int, _ assigned: Int) -> [Ticket] {
        (0..<ttr).map { t(String(format: "R-%06d", $0), status: "escalated_ttr", opened: "2026-11-01 09:00:00", updated: "2026-11-01 09:00:00") } +
        (0..<assigned).map { t(String(format: "R-%06d", 100 + $0), opened: "2026-11-01 09:00:00", updated: "2026-11-01 09:00:00") }
    }

    @Test("Page 1 takes the first tickets in group order, so the most urgent come first")
    func firstPage() {
        let groups = TicketList.groups(tickets(3, 30), me: me, view: .mine)
        let page = TicketPage(groups, size: 10, page: 1)
        #expect(page.sections.map(\.group) == [.escalatedTTR, .assigned])
        #expect(page.sections.map(\.tickets.count) == [3, 7])
        #expect(page.total == 33)
        #expect(page.pageCount == 4)
        #expect(page.label == "1–10 of 33")
        #expect(page.groupTotal(.assigned) == 30, "headers count the whole group, not just this page")
    }

    @Test("A later page keeps the group headers of what it shows")
    func laterPage() {
        let page = TicketPage(TicketList.groups(tickets(3, 30), me: me, view: .mine), size: 10, page: 4)
        #expect(page.sections.map(\.group) == [.assigned])
        #expect(page.sections.first?.tickets.count == 3)
        #expect(page.label == "31–33 of 33")
    }

    @Test("Pages out of range are clamped, and All shows everything on one page")
    func clampAndAll() {
        let groups = TicketList.groups(tickets(3, 30), me: me, view: .mine)
        #expect(TicketPage(groups, size: 10, page: 99).page == 4)
        #expect(TicketPage(groups, size: 10, page: 0).page == 1)
        let all = TicketPage(groups, size: nil, page: 3)
        #expect(all.page == 1 && all.pageCount == 1 && all.sections.flatMap(\.tickets).count == 33)
    }

    @Test("No tickets: one empty page")
    func empty() {
        let page = TicketPage([], size: 20, page: 1)
        #expect(page.pageCount == 1 && page.total == 0 && page.sections.isEmpty)
    }
}
