import Foundation
import Synchronization
import Testing
@testable import ITopKit
@testable import ITopWrap

private let me = CurrentUser(userID: "1", contactID: "7", login: "agent", displayName: "Agent Seven", profiles: [],
                             teams: [Team(id: "3", name: "Service desk")])

private func ticket(status: String, agentID: String) -> Ticket {
    Ticket(key: "601", ticketClass: .userRequest, ref: "R-000601", title: "New one", status: status, priority: 4,
           callerName: "Caller", orgName: "Org A", teamID: agentID == "0" ? "0" : "3", teamName: "",
           agentID: agentID, agentName: agentID == "0" ? "" : me.displayName, startDate: nil, lastUpdate: nil)
}

/// iTop as the list sees it: the ticket is assigned once `assigned` is set; it joins My tickets only if `listed`.
private final class FakeITop: Sendable {
    let assigned = Mutex(false)
    let listed: Bool
    let refuse: Bool
    init(listed: Bool, refuse: Bool = false) { self.listed = listed; self.refuse = refuse }

    var current: Ticket { assigned.withLock { $0 } ? ticket(status: "assigned", agentID: "7") : ticket(status: "new", agentID: "0") }

    func source() -> TicketSource {
        TicketSource(
            load: { [self] _ in listed && assigned.withLock { $0 } ? [current] : [] },
            lookup: { [self] ref in ref == "R-000601" ? current : nil },
            detail: { _ in throw CancellationError() },
            image: { _ in throw CancellationError() },
            attachments: { _ in [] },
            download: { _ in throw CancellationError() },
            addEntry: { _, _, _ in },
            canWrite: { _ in true },
            assign: { [self] _, _, _ in assigned.withLock { $0 = true } },
            assignToMe: { [self] _ in
                if refuse { throw ITopError.server(code: 100, message: "Not allowed") }
                assigned.withLock { $0 = true }
            },
            teams: { [] },
            agents: { _ in [] },
            change: { _, _ in },
            services: { _ in [] },
            subcategories: { _ in [] })
    }
}

@MainActor
private func model(_ itop: FakeITop) throws -> TicketListModel {
    let client = ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"))
    return TicketListModel(view: .mine, session: Session(client: client, user: me),
                           options: ListOptions(defaults: UserDefaults(suiteName: UUID().uuidString)!), source: itop.source())
}

@Suite("A ticket opened outside the list stays current after a change")
@MainActor
struct LookupRefreshTests {
    @Test("Created, then assigned to me: the open ticket shows as mine, not the copy from before")
    func assignedJoinsList() async throws {
        let itop = FakeITop(listed: true)
        let list = try model(itop)
        list.show(itop.current, notice: "R-000601 is created.")

        await list.assignToMe(itop.current)

        let shown = try #require(list.ticket(list.selection))
        #expect(shown.agentID == "7")
        #expect(shown.status == "assigned")
    }

    @Test("Changed but still outside the list: the open ticket is read again")
    func staysOutside() async throws {
        let itop = FakeITop(listed: false)
        let list = try model(itop)
        list.show(itop.current)

        await list.assignToMe(itop.current)

        let shown = try #require(list.ticket(list.selection))
        #expect(shown.agentID == "7")
    }
}

@Suite("New ticket with Assign to me")
@MainActor
struct AssignNewTests {
    @Test("The new ticket opens, is assigned, and one notice says both")
    func assigned() async throws {
        let itop = FakeITop(listed: true)
        let list = try model(itop)
        list.show(itop.current, notice: "R-000601 is created.")
        await list.assignNew(itop.current)
        #expect(list.notice == TicketListModel.Notice(text: "R-000601 is created and assigned to you.", isError: false))
        #expect(list.ticket(list.selection)?.agentID == "7")
    }

    @Test("If iTop refuses the assignment, the notice says the ticket exists anyway, and why")
    func refused() async throws {
        let itop = FakeITop(listed: true, refuse: true)
        let list = try model(itop)
        list.show(itop.current, notice: "R-000601 is created.")
        await list.assignNew(itop.current)
        let notice = try #require(list.notice)
        #expect(notice.isError)
        #expect(notice.text.hasPrefix("R-000601 is created, but not assigned to you. "))
        #expect(notice.text.contains("Not allowed"))
        #expect(list.selection == itop.current.id, "the new ticket stays open")
    }
}
