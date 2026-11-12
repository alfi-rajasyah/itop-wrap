import AppKit
import ITopKit
import SwiftUI

/// The menu bar item's label: the Stacked stubs icon, the running timer, the Needs you count.
struct MenuBarLabel: View {
    let workspace: Workspace

    var body: some View {
        let status = MenuBarStatus(needCount: workspace.mine.needCount, timer: workspace.timer.running)
        let title = status.title(now: workspace.clock)
        Image(nsImage: MenuBarIcon.label(title))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(status.accessibilityLabel(now: workspace.clock))
    }
}

/// The menu bar panel (DESIGN.md: Menu bar item): My tickets grouped by status, escalated first,
/// each with its time and a start/stop button. The running row is tinted and shows the live
/// counter. Shares the timer and list with the main window.
struct MenuBarPanel: View {
    let workspace: Workspace
    let openTicket: (Ticket?) -> Void
    @State private var query = ""

    private var mine: TicketListModel { workspace.mine }
    private var timer: TimerModel { workspace.timer }

    private var sections: [TicketSection] {
        let tickets = TicketSearch.filter(mine.tickets.filter { !["resolved", "closed"].contains($0.status) }, query: query)
        let groups = TicketList.groups(tickets, me: mine.me, view: .mine, sort: workspace.options.sort)
        return MenuBarStatus.groupOrder.compactMap { g in groups.first { $0.group == g } }
    }

    /// The running ticket sits on top under its own heading (you asked to see what's timing first).
    private var pinned: (running: Ticket?, sections: [TicketSection]) {
        MenuBarStatus.pinRunning(sections, runningID: timer.running?.ticketID)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // New ticket sits next to search, labelled: as an icon in the footer it went unnoticed.
            HStack(spacing: 8) {
                TextField("Search my tickets", text: $query)
                    .textFieldStyle(.roundedBorder)
                Button { workspace.requestNewTicket(); openTicket(nil) } label: {
                    Label("New ticket", systemImage: "square.and.pencil")
                }
                .buttonStyle(.bordered)
                .keyboardShortcut("n", modifiers: .command)
                .help("New ticket (⌘N)")
            }
            .padding(10)
            if let n = timer.notice {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Image(systemName: "stopwatch").accessibilityHidden(true)
                    Text(n.text).fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 4)
                    if n.canRetry { Button("Retry") { Task { await timer.retry() } }.disabled(timer.saving) }
                    Button { timer.clearNotice() } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain).accessibilityLabel(n.canRetry ? "Discard this time" : "Dismiss")
                }
                .font(.system(size: 11.5))
                .foregroundStyle(n.isError ? Theme.warning : Theme.secondary)
                .padding(.horizontal, 12).padding(.bottom, 8)
            }
            Divider()
            content
            Divider()
            footer
        }
        .frame(width: 360)
    }

    @ViewBuilder
    private var content: some View {
        switch mine.phase {
        case .loading:
            ProgressView("Loading tickets from iTop").controlSize(.small).frame(maxWidth: .infinity).padding(24)
        case .failed(let message):
            VStack(spacing: 8) {
                Text("Couldn't load your tickets. \(message)").foregroundStyle(Theme.warning).multilineTextAlignment(.center)
                Button("Retry") { Task { await mine.refresh() } }
            }
            .font(.system(size: 12)).frame(maxWidth: .infinity).padding(20)
        case .loaded:
            let pinned = pinned
            if sections.isEmpty {
                Text(query.isEmpty ? "Nothing waiting on you. New assignments show up here." : "No tickets of yours match “\(query)”.")
                    .font(.system(size: 12)).foregroundStyle(Theme.secondary)
                    .frame(maxWidth: .infinity).padding(24)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if let running = pinned.running {
                            heading("Running")
                            row(running)
                        }
                        ForEach(pinned.sections, id: \.group) { section in
                            heading("\(section.group.title)  \(section.tickets.count)")
                            ForEach(section.tickets) { t in row(t) }
                        }
                    }
                    .padding(.bottom, 6)
                }
                // A scroll view in a menu bar panel has no height of its own and collapses to
                // nothing, so it gets one: its rows, up to 420 points.
                .frame(height: min(420, CGFloat(pinned.sections.count + (pinned.running == nil ? 0 : 1)) * 30
                                   + CGFloat(sections.reduce(0) { $0 + $1.tickets.count }) * 48 + 6))
            }
        }
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(Theme.secondary)
            .padding(.horizontal, 12).padding(.top, 10).padding(.bottom, 4)
    }

    private func row(_ t: Ticket) -> some View {
        let running = timer.running.flatMap { $0.ticketID == t.id ? $0 : nil }
        return HStack(spacing: 8) {
            Button { openTicket(t) } label: {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 5) {
                        if TicketList.needsYou(t, me: mine.me) {
                            Circle().fill(Theme.accent).frame(width: 7, height: 7).accessibilityLabel("Needs you")
                        }
                        Text(t.ref).font(.system(size: 11, design: .monospaced)).foregroundStyle(Theme.secondary)
                    }
                    Text(t.title).font(.system(size: 12.5, weight: .medium)).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Open \(t.ref) in iTop Wrap")
            if let running {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(TimeFormat.counter(Int(context.date.timeIntervalSince(running.start))))
                        .font(.system(size: 12, weight: .bold, design: .monospaced))
                }
                .font(.system(size: 12))
            } else if let spent = timer.totals[t.id], spent > 0 {
                Text(TimeFormat.total(spent)).font(.system(size: 12, design: .monospaced)).foregroundStyle(Theme.secondary)
            }
            timerButton(t, running: running != nil)
        }
        .padding(.horizontal, 12).padding(.vertical, 6)
        .background(running != nil ? Theme.chip : Color.clear)
    }

    @ViewBuilder
    private func timerButton(_ t: Ticket, running: Bool) -> some View {
        if running {
            Button { Task { await timer.stop() } } label: { Image(systemName: "stop.fill").frame(width: 22, height: 22) }
                .buttonStyle(.borderless)
                .help("Stop the timer and save its time")
                .accessibilityLabel("Stop timer on \(t.ref)")
        } else if timer.canStart(t) && timer.webStopwatches[t.id] == nil {
            Button { Task { await timer.start(t) } } label: { Image(systemName: "stopwatch").frame(width: 22, height: 22) }
                .buttonStyle(.borderless)
                .disabled(timer.saving)
                .help(timer.running.map { "Stops the timer on \($0.ref), saves its time, and starts one here" } ?? "Start timer")
                .accessibilityLabel("Start timer on \(t.ref)")
        } else {
            // Keeps the times lined up (DESIGN.md: the menu bar keeps an empty slot).
            Color.clear.frame(width: 22, height: 22).accessibilityHidden(true)
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Button { Task { await mine.refresh() } } label: { Image(systemName: "arrow.clockwise") }
                .buttonStyle(.borderless)
                .keyboardShortcut("r", modifiers: .command)
                .disabled(mine.refreshing)
                .help("Refresh (⌘R)")
                .accessibilityLabel("Refresh")
            Spacer()
            if let updated = mine.updatedAt {
                Text("Updated \(updated.formatted(.relative(presentation: .named)))")
                    .font(.system(size: 11)).foregroundStyle(Theme.secondary)
            }
            Spacer()
            Button { openTicket(nil) } label: { Image(systemName: "macwindow") }
                .buttonStyle(.borderless)
                .keyboardShortcut("o", modifiers: .command)
                .help("Open iTop Wrap (⌘O)")
                .accessibilityLabel("Open iTop Wrap")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
    }
}
