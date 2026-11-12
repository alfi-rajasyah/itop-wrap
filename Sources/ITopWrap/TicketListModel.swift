import Foundation
import ITopKit
import Observation

/// Where a list gets its tickets: iTop in the app, canned tickets in the debug demo.
struct TicketSource: Sendable {
    var load: @Sendable (TicketScope) async throws -> [Ticket]
    var lookup: @Sendable (String) async throws -> Ticket?
    var detail: @Sendable (Ticket) async throws -> TicketDetail
    var image: @Sendable (String) async throws -> InlineImageLoader.Image
    var attachments: @Sendable (Ticket) async throws -> [AttachmentRef]
    var download: @Sendable (AttachmentRef) async throws -> AttachmentFile
    var addEntry: @Sendable (Ticket, String, [LogImage], Bool) async throws -> Void
    var canWrite: @Sendable (Ticket) -> Bool
    var assign: @Sendable (Ticket, String, String) async throws -> Void
    var assignToMe: @Sendable (Ticket) async throws -> Void
    var teams: @Sendable () async throws -> [Team]
    var agents: @Sendable (String) async throws -> [Agent]
    var change: @Sendable (Ticket, TicketChange) async throws -> Void
    var services: @Sendable (Ticket) async throws -> [ServiceRef]
    var subcategories: @Sendable (String) async throws -> [ServiceRef]

    static func live(_ session: Session, view: TicketList.View) -> TicketSource {
        let service = TicketService(client: session.client)
        let loader = InlineImageLoader(client: session.client)
        let writes = TicketWrites(client: session.client)
        let client = session.client
        let me = session.user
        let directory = TeamDirectory(client: session.client)
        let catalog = ServiceCatalog(client: session.client)
        return TicketSource(
            load: { scope in view == .mine ? try await service.loadMine(me: me, scope: scope) : try await service.loadTeam(me: me, scope: scope) },
            lookup: { try await service.lookup($0) },
            detail: { try await service.detail(for: $0) },
            image: { try await loader.image(id: $0) },
            attachments: { try await loader.attachments(of: $0) },
            download: { try await loader.download($0) },
            addEntry: { try await writes.addLogEntry(to: $0, text: $1, images: $2, isPrivate: $3) },
            canWrite: { client.canWrite(className: $0.ticketClass.rawValue, key: $0.key) },
            assign: { try await writes.assign($0, teamID: $1, agentID: $2) },
            assignToMe: { try await writes.assignToMe($0, me: me) },
            teams: { try await directory.teams() },
            agents: { try await directory.agents(inTeam: $0) },
            change: { try await writes.apply($1, to: $0) },
            services: { try await catalog.services(for: $0) },
            subcategories: { try await catalog.subcategories(ofService: $0) })
    }
}

/// One ticket list (My tickets or Team tickets): loading, refresh, search and selection.
@MainActor @Observable
final class TicketListModel {
    enum Phase: Equatable {
        case loading
        case loaded
        case failed(String)
    }

    enum Lookup: Equatable {
        case none
        case searching(String)
        case found(Ticket)
        case missing(String)
        case failed(String)
    }

    let view: TicketList.View
    let me: CurrentUser
    let server: ServerAddress
    let source: TicketSource
    let options: ListOptions

    private(set) var phase: Phase = .loading
    private(set) var tickets: [Ticket] = []
    private(set) var updatedAt: Date?
    private(set) var refreshing = false
    private var refreshAgain = false
    private(set) var refreshError: String?
    private(set) var lookup: Lookup = .none
    var query = ""
    var selection: Ticket.ID?
    /// The org filter (DESIGN.md: Org filter): one organization, or `nil` for all.
    var orgFilter: String?
    /// The app's one timer, shared by both lists (set by the shell).
    var timer: TimerModel?
    /// Unsent log entries per ticket and log, kept across ticket switches and failed sends.
    var drafts: [String: String] = [:]
    /// Images waiting in the composer, by the same key as `drafts`.
    var draftImages: [String: [LogImage]] = [:]

    /// The page shown; back to 1 whenever what's listed changes.
    var page = 1 { didSet { if page != oldValue { UITrace.log("\(view) list: page \(page)") } } }

    init(view: TicketList.View, session: Session, options: ListOptions, source: TicketSource? = nil) {
        self.view = view
        self.options = options
        me = session.user
        server = session.client.server
        self.source = source ?? .live(session, view: view)
    }

    /// Only open tickets can need you; resolved and closed ones in the range don't count.
    var needCount: Int { tickets.filter { TicketList.needsYou($0, me: me) }.count }

    var visibleSections: [TicketSection] {
        TicketList.groups(TicketSearch.filter(TicketSearch.filter(tickets, orgs: orgFilter.map { [$0] } ?? []), query: query), me: me, view: view, sort: options.sort)
    }

    var currentPage: TicketPage { TicketPage(visibleSections, size: options.pageSize, page: page) }

    var orgChoices: [String] { TicketSearch.orgs(in: tickets) }

    /// The org id behind a name, from any ticket for it, for "See all of its tickets".
    func orgID(named name: String) -> String? {
        tickets.first { $0.orgName == name && $0.orgID != "0" }?.orgID
    }

    func toggleOrg(_ name: String) {
        UITrace.log("\(view) list: org filter changed")
        orgFilter = orgFilter == name ? nil : name
    }

    var isSearching: Bool { !query.trimmingCharacters(in: .whitespaces).isEmpty }

    var hasNoTeams: Bool { view == .team && me.teams.isEmpty }

    func ticket(_ id: Ticket.ID?) -> Ticket? {
        guard let id else { return nil }
        if case .found(let t) = lookup, t.id == id { return t }
        return tickets.first { $0.id == id }
    }

    /// First load shows a spinner; later loads keep the list on screen.
    func refresh() async {
        // A range change during a refresh runs right after it, instead of being dropped.
        if refreshing { refreshAgain = true; return }
        refreshing = true
        UITrace.log("\(view) list: reload started (\(tickets.count) shown)")
        defer {
            refreshing = false
            if refreshAgain { refreshAgain = false; Task { await refresh() } }
        }
        do {
            tickets = try await source.load(options.scope)
            UITrace.log("\(view) list: reload finished, \(tickets.count) tickets, selection \(selection ?? "none")")
            updatedAt = Date()
            phase = .loaded
            refreshError = nil
            if let s = selection, ticket(s) == nil { selection = nil }
            if let timer {
                if view == .mine { await timer.myTicketsChanged(tickets) }
                let loaded = tickets
                Task { await timer.refreshTotals(loaded) }
            }
        } catch {
            if phase == .loaded { refreshError = ErrorText.describe(error) } else { phase = .failed(ErrorText.describe(error)) }
        }
    }

    /// When the search is a full ref that isn't in the list, ask iTop for it.
    func searchChanged() async {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard TicketQueries.refLookup(q) != nil,
              !tickets.contains(where: { $0.ref.caseInsensitiveCompare(q) == .orderedSame }) else {
            lookup = .none
            return
        }
        let ref = q.uppercased()
        if case .found(let t) = lookup, t.ref == ref { return }
        lookup = .searching(ref)
        UITrace.log("\(view) list: looking up a ref")
        try? await Task.sleep(for: .milliseconds(250))
        guard !Task.isCancelled else { return }
        do {
            let found = try await source.lookup(ref)
            guard query.trimmingCharacters(in: .whitespaces).uppercased() == ref else { return }
            lookup = found.map(Lookup.found) ?? .missing(ref)
        } catch {
            lookup = .failed(ErrorText.describe(error))
        }
    }

    // MARK: Assign (PLAN.md Task 7)

    /// The ticket an assign is running on, for a spinner and to block a second click.
    private(set) var working: Ticket.ID?
    /// One line about the last assign, shown above the list: what happened, or why not.
    private(set) var notice: Notice?

    struct Notice: Equatable {
        let text: String
        let isError: Bool
    }

    func clearNotice() { notice = nil }

    func canAssign(_ t: Ticket) -> Bool { TicketWrites.assignStimulus(for: t.status) != nil }

    /// Assign to me from a row, the header or the sheet: one call, then a refresh.
    func assignToMe(_ t: Ticket) async {
        guard working == nil else { return }
        working = t.id
        notice = nil
        defer { working = nil }
        UITrace.log("\(view) list: assign to me on \(t.id)")
        do {
            try await source.assignToMe(t)
            notice = Notice(text: "\(t.ref) is assigned to you.", isError: false)
            await refresh()
        } catch {
            notice = Notice(text: Self.assignError(error, ticket: t), isError: true)
            if case TicketWrites.Failure.alreadyTaken = error { await refresh() }
        }
    }

    /// Assign or reassign to a chosen team and agent. Returns the error text, or nil when done.
    func assign(_ t: Ticket, team: Team, agent: Agent) async -> String? {
        guard working == nil else { return "Another assignment is still running." }
        working = t.id
        notice = nil
        defer { working = nil }
        UITrace.log("\(view) list: assign \(t.id) to a chosen team and agent")
        do {
            try await source.assign(t, team.id, agent.id)
            let who = agent.id == me.contactID ? "you" : agent.name
            notice = Notice(text: "\(t.ref) is assigned to \(who) in \(team.name).", isError: false)
            await refresh()
            return nil
        } catch {
            return Self.assignError(error, ticket: t)
        }
    }

    /// Pending, resolve, reopen or close, then a refresh. Returns the error text, or nil when done.
    func apply(_ change: TicketChange, to t: Ticket) async -> String? {
        guard working == nil else { return "Another change is still running." }
        working = t.id
        notice = nil
        defer { working = nil }
        UITrace.log("\(view) list: \(change.action.rawValue) on \(t.id)")
        // Pending and Resolve stop a running timer and save its time first (DESIGN.md: Time).
        switch change {
        case .pending: await timer?.stopIfRunning(on: t, because: "Setting \(t.ref) to pending stopped its timer.")
        case .resolve: await timer?.stopIfRunning(on: t, because: "Resolving \(t.ref) stopped its timer.")
        default: break
        }
        do {
            try await source.change(t, change)
            let done: String
            switch change {
            case .pending: done = "set to pending"
            case .resolve: done = "resolved"
            case .reopen: done = "reopened"
            case .close: done = "closed"
            }
            notice = Notice(text: "\(t.ref) is \(done).", isError: false)
            await refresh()
            return nil
        } catch {
            let verb: String
            switch change {
            case .pending: verb = "set \(t.ref) to pending"
            case .resolve: verb = "resolve \(t.ref)"
            case .reopen: verb = "reopen \(t.ref)"
            case .close: verb = "close \(t.ref)"
            }
            return Self.actionError(error, ticket: t, verb: verb)
        }
    }

    /// Reopen and Close have no sheet, so their errors go to the notice line.
    func applyFromHeader(_ change: TicketChange, to t: Ticket) async {
        if let message = await apply(change, to: t) { notice = Notice(text: message, isError: true) }
    }

    static func assignError(_ error: Error, ticket t: Ticket) -> String {
        actionError(error, ticket: t, verb: "assign \(t.ref)")
    }

    static func actionError(_ error: Error, ticket t: Ticket, verb: String) -> String {
        switch error {
        case TicketWrites.Failure.alreadyTaken(let name):
            return "\(name) took \(t.ref) a moment ago, so it stays with them."
        case TicketWrites.Failure.noTeam:
            return "You aren't in any team in iTop, so Assign to me can't pick one. Choose a team and agent instead."
        case TicketWrites.Failure.cannotAssign(let status):
            return "\(TicketGroup.statusLabel(status)) tickets can't be assigned."
        case TicketWrites.Failure.invalidChoice:
            return "Pick a team and an agent first."
        case TicketWrites.Failure.missing(let field):
            return "Fill in the \(field) first."
        case TicketWrites.Failure.notAllowed(let status):
            return "\(TicketGroup.statusLabel(status)) tickets can't do that. Refresh to see the ticket as it is now."
        case ITopError.writesDisabled:
            return "Changes to iTop are switched off in this build, so nothing was sent."
        default:
            var reason = ErrorText.describe(error).trimmingCharacters(in: .whitespaces)
            if let last = reason.last, !".!?".contains(last) { reason += "." }
            return "iTop didn't \(verb). \(reason)"
        }
    }

    /// Shows a ticket that may not be in the list yet (a new one): as the search result, selected.
    func show(_ t: Ticket, notice text: String? = nil) {
        query = t.ref
        lookup = .found(t)
        selection = t.id
        if let text { notice = Notice(text: text, isError: false) }
    }

    /// "Open in iTop on the web" for one ticket.
    func webURL(_ t: Ticket) -> URL? {
        webURL(className: t.ticketClass.rawValue, id: t.key)
    }

    func webURL(className: String, id: String) -> URL? {
        URL(string: server.baseURL.absoluteString + "/pages/UI.php?operation=details&class=\(className)&id=\(id)")
    }
}
