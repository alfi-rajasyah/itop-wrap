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
                // DESIGN.md: a stop icon and the live 00:12:34 counter, bold, in SF Mono.
                Button { Task { await timer.stop() } } label: {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        HStack(spacing: 5) {
                            Image(systemName: "stop.fill")
                            Text(TimeFormat.counter(Int(context.date.timeIntervalSince(running.start))))
                                .font(.system(size: 12, weight: .bold, design: .monospaced))
                        }
                    }
                }
                .buttonStyle(HeaderControlStyle())
                .help("Stop timer: saves its time (⌘T)")
                .accessibilityLabel("Stop timer")
            } else if let since = timer.webStopwatches[ticket.id] {
                Label("Web stopwatch since \(TimerModel.time(since))", systemImage: "globe")
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.secondary)
                    .help("Your stopwatch in iTop's web UI is running on this ticket, so the app doesn't start its own")
            } else if timer.canStart(ticket) {
                // DESIGN.md: an icon-only stopwatch next to the main action.
                Button { Task { await timer.start(ticket) } } label: {
                    Image(systemName: "stopwatch")
                }
                .buttonStyle(HeaderControlStyle())
                .accessibilityLabel("Start timer")
                .disabled(!canWrite || timer.saving)
                .help(canWrite ? (timer.running.map { "Start timer: stops the one on \($0.ref) and saves its time (⌘T)" } ?? "Start timer (⌘T)")
                               : "Changes to iTop are switched off in this build")
            }
        }
        .keyboardShortcut("t", modifiers: .command)
        .task(id: ticket.id) { await timer.checkWebStopwatch(ticket) }
    }
}
