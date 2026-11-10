import Foundation
import ITopKit
import Observation

/// Where a list gets its tickets: iTop in the app, canned tickets in the debug demo.
struct TicketSource: Sendable {
    var load: @Sendable (TicketScope) async throws -> [Ticket]
    var lookup: @Sendable (String) async throws -> Ticket?
    var detail: @Sendable (Ticket) async throws -> TicketDetail

    static func live(_ session: Session, view: TicketList.View) -> TicketSource {
        let service = TicketService(client: session.client)
        let me = session.user
        return TicketSource(
            load: { scope in view == .mine ? try await service.loadMine(me: me, scope: scope) : try await service.loadTeam(me: me, scope: scope) },
            lookup: { try await service.lookup($0) },
            detail: { try await service.detail(for: $0) })
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
    /// The org filter (DESIGN.md: Org filter); empty means all organizations.
    var orgFilter: Set<String> = []
    /// The page shown; back to 1 whenever what's listed changes.
    var page = 1

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
        TicketList.groups(TicketSearch.filter(TicketSearch.filter(tickets, orgs: orgFilter), query: query), me: me, view: view, sort: options.sort)
    }

    var currentPage: TicketPage { TicketPage(visibleSections, size: options.pageSize, page: page) }

    var orgChoices: [String] { TicketSearch.orgs(in: tickets) }

    /// The org id behind a name, from any ticket for it, for "See all of its tickets".
    func orgID(named name: String) -> String? {
        tickets.first { $0.orgName == name && $0.orgID != "0" }?.orgID
    }

    func toggleOrg(_ name: String) {
        if orgFilter.contains(name) { orgFilter.remove(name) } else { orgFilter.insert(name) }
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
        defer {
            refreshing = false
            if refreshAgain { refreshAgain = false; Task { await refresh() } }
        }
        do {
            tickets = try await source.load(options.scope)
            updatedAt = Date()
            phase = .loaded
            refreshError = nil
            if let s = selection, ticket(s) == nil { selection = nil }
        } catch {
            if phase == .loaded { refreshError = error.localizedDescription } else { phase = .failed(error.localizedDescription) }
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
        try? await Task.sleep(for: .milliseconds(250))
        guard !Task.isCancelled else { return }
        do {
            let found = try await source.lookup(ref)
            guard query.trimmingCharacters(in: .whitespaces).uppercased() == ref else { return }
            lookup = found.map(Lookup.found) ?? .missing(ref)
        } catch {
            lookup = .failed(error.localizedDescription)
        }
    }

    /// "Open in iTop on the web" for one ticket.
    func webURL(_ t: Ticket) -> URL? {
        webURL(className: t.ticketClass.rawValue, id: t.key)
    }

    func webURL(className: String, id: String) -> URL? {
        URL(string: server.baseURL.absoluteString + "/pages/UI.php?operation=details&class=\(className)&id=\(id)")
    }
}
