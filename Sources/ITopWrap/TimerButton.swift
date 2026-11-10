import ITopKit
import SwiftUI

/// Start or stop the timer on one ticket (DESIGN.md: Time). ⌘T toggles it. Only Assigned tickets
/// that are yours get it; a web stopwatch of yours on the ticket is shown instead.
struct TimerButton: View {
    let timer: TimerModel
    let ticket: Ticket
    let canWrite: Bool

    var body: some View {
        Group {
            if let running = timer.running, running.ticketID == ticket.id {
                Button { Task { await timer.stop() } } label: {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        Label("Stop \(TimerModel.elapsed(since: running.start, now: context.date))", systemImage: "stop.fill")
                            .monospacedDigit()
                    }
                }
                .help("Stop the timer and save its time (⌘T)")
                .accessibilityLabel("Stop timer")
            } else if let since = timer.webStopwatches[ticket.id] {
                Label("Web stopwatch since \(TimerModel.time(since))", systemImage: "globe")
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.secondary)
                    .help("Your stopwatch in iTop's web UI is running on this ticket, so the app doesn't start its own")
            } else if timer.canStart(ticket) {
                Button { Task { await timer.start(ticket) } } label: {
                    Label(timer.running == nil ? "Start timer" : "Switch timer here", systemImage: "play.fill")
                }
                .disabled(!canWrite || timer.saving)
                .help(canWrite ? (timer.running.map { "Stops the timer on \($0.ref), saves its time, and starts one here (⌘T)" } ?? "Start the timer (⌘T)")
                               : "Changes to iTop are switched off in this build")
            }
        }
        .keyboardShortcut("t", modifiers: .command)
        .task(id: ticket.id) { await timer.checkWebStopwatch(ticket) }
    }
}
