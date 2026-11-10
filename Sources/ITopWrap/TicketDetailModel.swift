import Foundation
import ITopKit
import Observation

/// Loads one ticket's description, logs and linked CIs for the detail pane.
@MainActor @Observable
final class TicketDetailModel {
    enum Phase: Equatable {
        case loading
        case loaded(TicketDetail)
        case failed(String)
    }

    let ticket: Ticket
    private let source: TicketSource
    private(set) var phase: Phase = .loading
    var tab: Tab = .public
    private(set) var copied = false

    enum Tab: Hashable { case `public`, `private` }

    init(ticket: Ticket, source: TicketSource) {
        self.ticket = ticket
        self.source = source
    }

    func load() async {
        if case .loaded = phase {} else { phase = .loading }
        do {
            phase = .loaded(try await source.detail(ticket))
        } catch {
            if case .loaded = phase { return }
            phase = .failed(error.localizedDescription)
        }
    }

    /// "Copied R-000412" shows for 2 seconds (wireframe, review 002).
    func copyRef(_ write: (String) -> Void) async {
        write(ticket.ref)
        copied = true
        try? await Task.sleep(for: .seconds(2))
        copied = false
    }
}
