import Foundation
import ITopKit
import Observation

/// Where the timer saves time and reads totals: iTop in the app, memory in the debug demo.
struct TimerSource: Sendable {
    var save: @Sendable (TimerEntry, Ticket) async throws -> Void
    var webStopwatch: @Sendable (Ticket) async throws -> Date?
    var totals: @Sendable ([Ticket]) async throws -> [Ticket.ID: Int]

    static func live(_ session: Session) -> TimerSource {
        let tracking = TimeTracking(client: session.client)
        let me = session.user
        return TimerSource(save: { try await tracking.save($0, on: $1, me: me) },
                           webStopwatch: { try await tracking.runningStopwatch(on: $0, me: me) },
                           totals: { try await tracking.totals(for: $0) })
    }
}

/// The one timer (PLAN.md Task 8b; DESIGN.md: Time). Starting another stops and saves the
/// first; a ticket that leaves Assigned stops it; it survives quitting.
@MainActor @Observable
final class TimerModel {
    struct Notice: Equatable {
        let text: String
        let isError: Bool
        /// A save that failed, kept so you can retry it instead of losing the time.
        let canRetry: Bool
    }

    private let source: TimerSource
    private let store: TimerStore
    private let me: CurrentUser
    private let canWrite: @Sendable (Ticket) -> Bool

    private(set) var running: RunningTimer?
    private(set) var totals: [Ticket.ID: Int] = [:]
    /// Web stopwatches of yours seen running, by ticket, with their start.
    private(set) var webStopwatches: [Ticket.ID: Date] = [:]
    private(set) var notice: Notice?
    private(set) var saving = false
    private var failed: (TimerEntry, Ticket)?

    init(source: TimerSource, store: TimerStore, me: CurrentUser, canWrite: @escaping @Sendable (Ticket) -> Bool) {
        self.source = source
        self.store = store
        self.me = me
        self.canWrite = canWrite
        running = store.load()
    }

    /// Only tickets assigned to you and in the Assigned state get a start button.
    func canStart(_ t: Ticket) -> Bool { t.status == "assigned" && t.agentID == me.contactID }

    func isRunning(_ t: Ticket) -> Bool { running?.ticketID == t.id }

    func clearNotice() {
        notice = nil
        failed = nil
    }

    func toggle(_ t: Ticket) async {
        if isRunning(t) { await stop() } else { await start(t) }
    }

    func start(_ t: Ticket) async {
        guard canStart(t), !isRunning(t), !saving else { return }
        guard canWrite(t) else {
            notice = Notice(text: "Saving time to iTop is switched off in this build, so the timer can't start.", isError: true, canRetry: false)
            return
        }
        // Don't count twice: a web stopwatch of yours on this ticket wins.
        if let since = try? await source.webStopwatch(t) {
            webStopwatches[t.id] = since
            notice = Notice(text: "Your web stopwatch is already running on \(t.ref) since \(Self.time(since)). Stop it in iTop, or keep using it there.",
                            isError: false, canRetry: false)
            return
        }
        webStopwatches[t.id] = nil
        if running != nil { await stop() }
        UITrace.log("timer: started on \(t.id)")
        running = RunningTimer(ticket: t, start: Date())
        store.save(running)
        notice = nil
    }

    /// Stops the timer and saves its time. `because` explains a stop you didn't ask for.
    func stop(because reason: String? = nil) async {
        guard let timer = running else { return }
        let entry = TimerEntry.stop(timer, at: Date())
        running = nil
        store.save(nil)
        UITrace.log("timer: stopped on \(timer.ticketID), \(entry.duration) s")
        let prefix = reason.map { "\($0) " } ?? ""
        if entry.isTooShort {
            notice = Notice(text: "\(prefix)The timer on \(timer.ref) ran under a minute, so nothing was saved.", isError: false, canRetry: false)
            return
        }
        await save(entry, Self.ticket(for: timer), prefix: prefix)
    }

    func retry() async {
        guard let (entry, ticket) = failed else { return }
        await save(entry, ticket, prefix: "")
    }

    private func save(_ entry: TimerEntry, _ ticket: Ticket, prefix: String) async {
        saving = true
        defer { saving = false }
        do {
            try await source.save(entry, ticket)
            failed = nil
            totals[ticket.id, default: 0] += entry.duration
            let cap = entry.capped ? " It stopped at \(TimeFormat.clock(entry.duration)), like iTop's own stopwatch." : ""
            notice = Notice(text: "\(prefix)Saved \(TimeFormat.clock(entry.duration)) to \(ticket.ref).\(cap)", isError: false, canRetry: false)
        } catch {
            failed = (entry, ticket)
            var reason = ErrorText.describe(error).trimmingCharacters(in: .whitespaces)
            if let last = reason.last, !".!?".contains(last) { reason += "." }
            notice = Notice(text: "\(prefix)Couldn't save \(TimeFormat.clock(entry.duration)) to \(ticket.ref). \(reason)", isError: true, canRetry: true)
        }
    }

    /// After My tickets reloads: a running ticket that's no longer Assigned to you stops and saves.
    func myTicketsChanged(_ tickets: [Ticket]) async {
        guard let timer = running else { return }
        if let t = tickets.first(where: { $0.id == timer.ticketID }), canStart(t) { return }
        await stop(because: "\(timer.ref) left Assigned, so its timer stopped.")
    }

    /// Before Set to pending or Resolve on the running ticket.
    func stopIfRunning(on t: Ticket, because reason: String) async {
        if isRunning(t) { await stop(because: reason) }
    }

    func refreshTotals(_ tickets: [Ticket]) async {
        let open = tickets.filter { !["resolved", "closed"].contains($0.status) }
        guard let found = try? await source.totals(open) else { return }
        totals.merge(found) { _, new in new }
    }

    func checkWebStopwatch(_ t: Ticket) async {
        guard canStart(t), !isRunning(t) else { return }
        webStopwatches[t.id] = (try? await source.webStopwatch(t)) ?? nil
    }

    static func ticket(for timer: RunningTimer) -> Ticket {
        Ticket(key: timer.ticketKey, ticketClass: TicketClass(rawValue: timer.ticketClass) ?? .userRequest, ref: timer.ref,
               title: timer.title, status: "", priority: 0, callerName: "", orgName: "", teamID: "0", teamName: "",
               agentID: "0", agentName: "", startDate: nil, lastUpdate: nil, orgID: timer.orgID)
    }

    static func time(_ date: Date) -> String {
        date.formatted(date: Calendar.current.isDateInToday(date) ? .omitted : .abbreviated, time: .shortened)
    }

    /// "0:42:10" for the running display.
    static func elapsed(since start: Date, now: Date = Date()) -> String {
        let s = Int(max(0, now.timeIntervalSince(start)))
        return "\(s / 3600):" + String(format: "%02d:%02d", (s / 60) % 60, s % 60)
    }
}
