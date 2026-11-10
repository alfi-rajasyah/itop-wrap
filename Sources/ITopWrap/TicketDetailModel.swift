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
    private(set) var sending = false
    private(set) var sendError: String?

    enum Tab: Hashable { case `public`, `private` }

    init(ticket: Ticket, source: TicketSource) {
        self.ticket = ticket
        self.source = source
    }

    func load() async {
        UITrace.log("detail: loading \(ticket.ticketClass.rawValue)::\(ticket.key)")
        defer { UITrace.log("detail: loaded \(ticket.ticketClass.rawValue)::\(ticket.key)") }
        if case .loaded = phase {} else { phase = .loading }
        do {
            phase = .loaded(try await source.detail(ticket))
        } catch {
            if case .loaded = phase { return }
            phase = .failed(ErrorText.describe(error))
        }
    }

    var canWrite: Bool { source.canWrite(ticket) }

    /// Adds the entry to the log of the current tab, then reloads the ticket to show it.
    /// On failure the draft stays and the error says why.
    func send(_ text: String) async -> Bool {
        guard !sending else { return false }
        sending = true
        sendError = nil
        defer { sending = false }
        UITrace.log("detail: adding a \(tab == .private ? "private" : "public") entry to \(ticket.ticketClass.rawValue)::\(ticket.key)")
        do {
            try await source.addEntry(ticket, text, tab == .private)
            await load()
            return true
        } catch TicketWrites.Failure.emptyEntry {
            sendError = "Write something first."
        } catch ITopError.writesDisabled {
            sendError = "Changes to iTop are switched off in this build, so nothing was sent. Your draft is kept."
        } catch {
            var reason = ErrorText.describe(error).trimmingCharacters(in: .whitespaces)
            if let last = reason.last, !".!?".contains(last) { reason += "." }
            sendError = "iTop didn't save the entry. \(reason) Your draft is kept."
        }
        return false
    }

    /// "Copied R-000412" shows for 2 seconds (wireframe, review 002).
    func copyRef(_ write: (String) -> Void) async {
        write(ticket.ref)
        copied = true
        try? await Task.sleep(for: .seconds(2))
        copied = false
    }
}
