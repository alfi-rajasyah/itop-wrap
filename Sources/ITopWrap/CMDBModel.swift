import Foundation
import ITopKit
import Observation

struct CMDBSource: Sendable {
    var search: @Sendable (String) async throws -> [ConfigItem]
    var detail: @Sendable (ConfigItem) async throws -> ConfigItemDetail
    var inOrg: @Sendable (String) async throws -> [ConfigItem]

    static func live(_ session: Session) -> CMDBSource {
        let service = CMDBService(client: session.client)
        return CMDBSource(search: { try await service.search($0) }, detail: { try await service.detail($0) },
                          inOrg: { try await service.items(inOrg: $0) })
    }
}

/// Config items: search, the selected CI and its detail.
@MainActor @Observable
final class CMDBModel {
    enum Results: Equatable {
        case idle
        case searching
        case loaded([ConfigItem])
        case failed(String)
    }

    enum Detail: Equatable {
        case loading
        case loaded(ConfigItemDetail)
        case failed(String)
    }

    let me: CurrentUser
    let server: ServerAddress
    private let source: CMDBSource

    var query = ""
    private(set) var results: Results = .idle
    var selectedID: ConfigItem.ID? {
        didSet {
            guard selectedID != current?.id, case .loaded(let items) = results,
                  let item = items.first(where: { $0.id == selectedID }) else { return }
            current = item
        }
    }
    /// The CI shown on the right. It can be one that isn't in the results (a relationship chip, a ticket's CI).
    private(set) var current: ConfigItem?
    private(set) var detail: Detail = .loading
    private(set) var copied = false

    init(session: Session, source: CMDBSource? = nil) {
        me = session.user
        server = session.client.server
        self.source = source ?? .live(session)
    }

    /// The query the current results belong to, so coming back to the section doesn't search again.
    private var searched: String?

    /// Set by the org view's Config items link; typing a search clears it.
    private(set) var scope: (id: String, name: String)?

    func showOrg(id: String, name: String) {
        scope = (id, name)
        query = ""
        searched = nil
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
            results = .searching
            do { results = .loaded(try await source.inOrg(scope.id)); searched = "org:\(scope.id)" } catch { results = .failed(ErrorText.describe(error)) }
            return
        }
        guard q.count >= 2 else { results = .idle; searched = nil; return }
        if q == searched, case .loaded = results { return }
        let previous = results
        results = .searching
        try? await Task.sleep(for: .milliseconds(300))
        guard !Task.isCancelled else { results = previous; return }
        do {
            let items = try await source.search(q)
            guard query.trimmingCharacters(in: .whitespaces) == q else { return }
            results = .loaded(items)
            searched = q
        } catch {
            results = .failed(ErrorText.describe(error))
        }
    }

    /// Show a CI from a relationship chip or a ticket.
    func open(_ item: ConfigItem) {
        current = item
        selectedID = item.id
    }

    func loadDetail() async {
        guard let item = current else { return }
        detail = .loading
        do {
            let d = try await source.detail(item)
            guard current?.id == item.id else { return }
            detail = .loaded(d)
        } catch {
            detail = .failed(ErrorText.describe(error))
        }
    }

    func copyName(_ write: (String) -> Void) async {
        guard let name = current?.name else { return }
        write(name)
        copied = true
        try? await Task.sleep(for: .seconds(2))
        copied = false
    }

    func webURL(_ item: ConfigItem) -> URL? {
        URL(string: server.baseURL.absoluteString + "/pages/UI.php?operation=details&class=\(item.className)&id=\(item.key)")
    }
}
