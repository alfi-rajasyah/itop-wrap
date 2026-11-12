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

    var ticket: Ticket
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

    var loadedDetail: TicketDetail? { if case .loaded(let d) = phase { d } else { nil } }

    /// Adds the entry, with any images, to the log of the current tab, then reloads the ticket
    /// to show it. On failure the draft and its images stay and the error says why.
    func send(_ text: String, images: [LogImage] = []) async -> Bool {
        guard !sending else { return false }
        sending = true
        sendError = nil
        defer { sending = false }
        UITrace.log("detail: adding a \(tab == .private ? "private" : "public") entry with \(images.count) images to \(ticket.ticketClass.rawValue)::\(ticket.key)")
        do {
            try await source.addEntry(ticket, text, images, tab == .private)
            await load()
            return true
        } catch TicketWrites.Failure.emptyEntry {
            sendError = "Write something or add an image first."
        } catch TicketWrites.Failure.tooManyImages(let max) {
            sendError = "An entry can have up to \(max) images. Remove some, then add the entry. Your draft is kept."
        } catch ITopError.writesDisabled {
            sendError = "Changes to iTop are switched off in this build, so nothing was sent. Your draft is kept."
        } catch {
            var reason = ErrorText.describe(error).trimmingCharacters(in: .whitespaces)
            if let last = reason.last, !".!?".contains(last) { reason += "." }
            sendError = "iTop didn't save the entry. \(reason) Your draft is kept."
        }
        return false
    }

    /// Checks pasted, dropped or chosen images and returns the ones that can go with the entry,
    /// up to `LogImage.maxPerEntry` with the `existing` ones. Says why any were left out.
    func intake(_ items: [ImageIntake.Item], existing: Int) -> [LogImage] {
        var accepted: [LogImage] = []
        var problems: [String] = []
        for item in items {
            do {
                accepted.append(try ImageIntake.logImage(item))
            } catch LogImage.Failure.tooLarge(let name, let maxMB) {
                problems.append("\(name) is larger than \(maxMB) MB.")
            } catch {
                problems.append("\(item.filename.isEmpty ? "That file" : item.filename) isn't an image iTop Wrap can add. Use PNG, JPEG or GIF.")
            }
        }
        let room = max(0, LogImage.maxPerEntry - existing)
        if accepted.count > room {
            let left = accepted.count - room
            problems.append("An entry can have up to \(LogImage.maxPerEntry) images, so \(left) \(left == 1 ? "wasn't" : "weren't") added.")
            accepted = Array(accepted.prefix(room))
        }
        sendError = problems.isEmpty ? nil : problems.joined(separator: " ")
        return accepted
    }

    func clearError() { sendError = nil }

    /// "Copied R-000412" shows for 2 seconds (wireframe, review 002).
    func copyRef(_ write: (String) -> Void) async {
        write(ticket.ref)
        copied = true
        try? await Task.sleep(for: .seconds(2))
        copied = false
    }
}
