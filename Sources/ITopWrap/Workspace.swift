import Foundation
import ITopKit
import Observation

/// Everything one signed-in session works with, shared by the main window and the menu bar
/// (PLAN.md Task 8c): the lists, the one timer and the CMDB views. It lives as long as the
/// session, not the window, so closing the window leaves the menu bar working.
@MainActor @Observable
final class Workspace {
    let session: Session
    let options: ListOptions
    let mine: TicketListModel
    let team: TicketListModel
    let cmdb: CMDBModel
    let orgs: OrgsModel
    let people: PeopleModel
    let timer: TimerModel
    let newTickets: NewTicketSource
    /// Set to show New ticket in the main window (from ⌘N, an org or a CI). The menu bar opens
    /// its own New Ticket window instead.
    var newTicketStart: NewTicketStart?
    /// What you typed in New ticket before cancelling, offered again next time.
    var newTicketDraft: NewTicket?
    private var refreshLoop: Task<Void, Never>?
    private var clockLoop: Task<Void, Never>?
    /// Ticks every 15 seconds so the menu bar's timer (h:mm) stays current.
    private(set) var clock = Date()

    init(session: Session) {
        self.session = session
        ErrorText.host = session.client.server.baseURL.host() ?? "the server"
        let options = ListOptions(defaults: DebugLaunch.listDefaults)
        self.options = options
        mine = TicketListModel(view: .mine, session: session, options: options, source: DebugLaunch.demoSource(.mine))
        team = TicketListModel(view: .team, session: session, options: options, source: DebugLaunch.demoSource(.team))
        cmdb = CMDBModel(session: session, source: DebugLaunch.demoCMDB())
        orgs = OrgsModel(session: session, options: options, source: DebugLaunch.demoOrgs())
        people = PeopleModel(session: session, options: options, source: DebugLaunch.demoPeople())
        let demo = DebugLaunch.demoTimer()
        let client = session.client
        let isDemo = demo != nil
        let canWrite: @Sendable (Ticket) -> Bool = { t in isDemo || client.canWrite(className: t.ticketClass.rawValue, key: t.key) }
        timer = TimerModel(source: demo ?? TimerSource.live(session), store: TimerStore(defaults: DebugLaunch.listDefaults),
                           me: session.user, canWrite: canWrite)
        mine.timer = timer
        team.timer = timer
        newTickets = DebugLaunch.demoNewTickets() ?? NewTicketSource.live(session)
    }

    /// A ticket made with New ticket: opened in My tickets, and assigned to you if you asked.
    func created(_ ticket: Ticket, assign: Bool) {
        mine.show(ticket, notice: "\(ticket.ref) is created.")
        if assign { Task { await mine.assignNew(ticket) } }
    }

    func requestNewTicket(_ start: NewTicketStart = NewTicketStart()) {
        newTicketStart = start
    }

    /// My tickets drives the menu bar count, so it refreshes every 2 minutes (UX-FLOWS F10)
    /// whether or not the window is open.
    func start() {
        guard refreshLoop == nil else { return }
        clockLoop = Task { [weak self] in
            while !Task.isCancelled {
                // Every second while a timer runs, so the menu bar counter ticks; otherwise
                // every 15 s is enough, and skipping the update spares the redraw.
                try? await Task.sleep(for: .seconds(1))
                guard let self else { return }
                let now = Date()
                if self.timer.running != nil || now.timeIntervalSince(self.clock) >= 15 { self.clock = now }
            }
        }
        refreshLoop = Task { [weak self] in
            if let self, self.mine.phase != .loaded { await self.mine.refresh() }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(120))
                guard !Task.isCancelled, let self else { return }
                await self.mine.refresh()
            }
        }
    }

    func stop() {
        refreshLoop?.cancel()
        refreshLoop = nil
        clockLoop?.cancel()
        clockLoop = nil
    }
}
