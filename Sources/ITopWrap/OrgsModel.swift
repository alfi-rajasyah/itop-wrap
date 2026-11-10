import Foundation
import ITopKit
import Observation

struct OrgSource: Sendable {
    var list: @Sendable () async throws -> [Organization]
    var tickets: @Sendable (String, TicketScope) async throws -> [Ticket]
    var counts: @Sendable (String) async throws -> (people: Int, configItems: Int)

    static func live(_ session: Session) -> OrgSource {
        let s = OrgService(client: session.client)
        return OrgSource(list: { try await s.organizations() }, tickets: { try await s.openTickets(orgID: $0, scope: $1) },
                         counts: { try await s.counts(orgID: $0) })
    }
}

struct PeopleSource: Sendable {
    var search: @Sendable (String) async throws -> (people: [Person], total: Int)
    var inOrg: @Sendable (String) async throws -> (people: [Person], total: Int)
    var assigned: @Sendable (String, TicketScope) async throws -> [Ticket]
    var reported: @Sendable (String, TicketScope) async throws -> [Ticket]

    static func live(_ session: Session) -> PeopleSource {
        let s = OrgService(client: session.client)
        return PeopleSource(search: { try await s.peoplePage(matching: $0) }, inOrg: { try await s.peoplePage(inOrg: $0) },
                            assigned: { try await s.tickets(assignedTo: $0, scope: $1) },
                            reported: { try await s.tickets(reportedBy: $0, scope: $1) })
    }
}

enum Loadable<Value: Equatable>: Equatable {
    case idle, loading, loaded(Value), failed(String)
}

/// Organizations: the list with counts, and the selected org's open tickets.
@MainActor @Observable
final class OrgsModel {
    let me: CurrentUser
    let options: ListOptions
    private let source: OrgSource

    private(set) var orgs: Loadable<[Organization]> = .idle
    var query = ""
    var selectedID: Organization.ID?
    private(set) var tickets: Loadable<[Ticket]> = .idle
    private(set) var counts: (people: Int, configItems: Int)?

    init(session: Session, options: ListOptions, source: OrgSource? = nil) {
        me = session.user
        self.options = options
        self.source = source ?? .live(session)
    }

    var visible: [Organization] {
        guard case .loaded(let all) = orgs else { return [] }
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        return q.isEmpty ? all : all.filter { ($0.name + " " + $0.code).lowercased().contains(q) }
    }

    var selected: Organization? {
        guard case .loaded(let all) = orgs else { return nil }
        return all.first { $0.id == selectedID }
    }

    func loadList() async {
        if case .loaded = orgs { return }
        orgs = .loading
        do { orgs = .loaded(try await source.list()) } catch { orgs = .failed(ErrorText.describe(error)) }
    }

    func reloadList() async {
        orgs = .idle
        await loadList()
    }

    func loadSelected() async {
        guard let id = selectedID else { return }
        tickets = .loading
        counts = nil
        do {
            async let t = source.tickets(id, options.scope)
            async let c = source.counts(id)
            tickets = .loaded(try await t)
            counts = try? await c
        } catch {
            tickets = .failed(ErrorText.describe(error))
        }
    }
}

/// People: search (or one org's people), the selected person and their tickets.
@MainActor @Observable
final class PeopleModel {
    enum Tab: Hashable { case assigned, reported }

    let me: CurrentUser
    let options: ListOptions
    private let source: PeopleSource

    var query = ""
    /// Set by the org view's People link; typing a search clears it.
    private(set) var scope: (id: String, name: String)?
    private(set) var results: Loadable<[Person]> = .idle
    /// How many iTop found in total; more than shown when the search hit the 100-person cap.
    private(set) var total = 0
    var selectedID: Person.ID?
    var tab: Tab = .assigned
    private(set) var assigned: Loadable<[Ticket]> = .idle
    private(set) var reported: Loadable<[Ticket]> = .idle
    private var searched: String?

    init(session: Session, options: ListOptions, source: PeopleSource? = nil) {
        me = session.user
        self.options = options
        self.source = source ?? .live(session)
    }

    var selected: Person? {
        guard case .loaded(let all) = results else { return nil }
        return all.first { $0.id == selectedID }
    }

    func showOrg(id: String, name: String) {
        scope = (id, name)
        query = ""
        searched = nil
        selectedID = nil
    }

    func clearScope() {
        scope = nil
        results = .idle
        searched = nil
    }

    func search() async {
        let q = query.trimmingCharacters(in: .whitespaces)
        if !q.isEmpty, scope != nil { scope = nil }
        if let scope {
            if searched == "org:\(scope.id)", case .loaded = results { return }
            results = .loading
            do {
                let page = try await source.inOrg(scope.id)
                results = .loaded(page.people); total = page.total; searched = "org:\(scope.id)"
            } catch { results = .failed(ErrorText.describe(error)) }
            return
        }
        guard q.count >= 2 else { results = .idle; searched = nil; return }
        if q == searched, case .loaded = results { return }
        let previous = results
        results = .loading
        try? await Task.sleep(for: .milliseconds(300))
        guard !Task.isCancelled else { results = previous; return }
        do {
            let page = try await source.search(q)
            guard query.trimmingCharacters(in: .whitespaces) == q else { return }
            results = .loaded(page.people)
            total = page.total
            searched = q
        } catch {
            results = .failed(ErrorText.describe(error))
        }
    }

    /// Agents open on "Assigned to them", callers on "Reported by them" (DESIGN.md: People).
    func loadSelected(keepTab: Bool = false) async {
        guard let p = selected else { return }
        if !keepTab { tab = p.isAgent ? .assigned : .reported }
        assigned = .loading
        reported = .loading
        let scope = options.scope
        async let a = source.assigned(p.id, scope)
        async let r = source.reported(p.id, scope)
        do { assigned = .loaded(try await a) } catch { assigned = .failed(ErrorText.describe(error)) }
        do { reported = .loaded(try await r) } catch { reported = .failed(ErrorText.describe(error)) }
    }
}
