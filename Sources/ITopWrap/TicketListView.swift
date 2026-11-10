import ITopKit
import SwiftUI

/// My tickets / Team tickets (DESIGN.md: Ticket list; wireframe Main.dc.html).
struct TicketListView: View {
    @Bindable var model: TicketListModel

    var body: some View {
        content
            .safeAreaInset(edge: .bottom, spacing: 0) { footer }
            .searchable(text: $model.query, placement: .toolbar, prompt: "R-000412 or VPN")
            .task(id: model.query) { await model.searchChanged() }
            .toolbar {
                ToolbarItem {
                    Button { Task { await model.refresh() } } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                    .keyboardShortcut("r", modifiers: .command)
                    .help("Refresh (⌘R)")
                    .disabled(model.refreshing)
                }
            }
            .task {
                if model.phase != .loaded { await model.refresh() }
                // UX-FLOWS F10: every 2 minutes while the list is open.
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(120))
                    if !Task.isCancelled { await model.refresh() }
                }
            }
    }

    @ViewBuilder
    private var content: some View {
        switch model.phase {
        case .loading:
            VStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text("Loading tickets from iTop").foregroundStyle(Theme.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            notice(message, retry: true)
        case .loaded:
            if model.hasNoTeams {
                notice("You're not in any iTop team, so there are no team tickets to show.", retry: false)
            } else if model.tickets.isEmpty && !model.isSearching {
                notice(model.view == .mine ? "Nothing waiting on you. New assignments show up here." : "Your teams have no open tickets.", retry: false)
            } else {
                list
            }
        }
    }

    private var list: some View {
        List(selection: $model.selection) {
            if let error = model.refreshError {
                Text("Couldn't refresh: \(error)")
                    .font(.system(size: 12)).foregroundStyle(Theme.warning)
                    .listRowSeparator(.hidden)
            }
            lookupSection
            let sections = model.visibleSections
            if model.isSearching && sections.isEmpty && model.lookup == .none && !isLookupPending {
                Text("No tickets in your list match “\(model.query.trimmingCharacters(in: .whitespaces))”.")
                    .foregroundStyle(Theme.secondary)
                    .listRowSeparator(.hidden)
            }
            ForEach(sections) { section in
                Section {
                    ForEach(section.tickets) { t in
                        TicketRow(ticket: t, me: model.me, showAgent: model.view == .team || t.isUnassigned)
                            .tag(t.id)
                    }
                } header: {
                    HStack {
                        Text(section.group.title)
                        Spacer()
                        Text("\(section.tickets.count)").monospacedDigit()
                    }
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Theme.secondary)
                }
            }
        }
        .listStyle(.inset)
        .safeAreaInset(edge: .top, spacing: 0) { lookupStatus }
    }

    private var isLookupPending: Bool {
        if case .searching = model.lookup { return true }
        return false
    }

    /// The found ticket, as the list's only extra section.
    @ViewBuilder
    private var lookupSection: some View {
        if case .found(let t) = model.lookup {
            Section("Search result: not in your queue") {
                TicketRow(ticket: t, me: model.me, showAgent: true).tag(t.id)
            }
        }
    }

    /// Lookup progress and misses sit above the list, not as rows: a status row followed by a
    /// ticket row in the same place kept the status row's height and clipped the ticket.
    @ViewBuilder
    private var lookupStatus: some View {
        switch model.lookup {
        case .searching(let ref):
            statusLine(Label("Looking up \(ref) in iTop…", systemImage: "magnifyingglass"), color: Theme.secondary)
        case .missing(let ref):
            statusLine(Text("\(ref) doesn't exist, or your iTop account can't see it."), color: Theme.secondary)
        case .failed(let message):
            statusLine(Text("Couldn't look that up: \(message)"), color: Theme.warning)
        case .none, .found:
            EmptyView()
        }
    }

    private func statusLine(_ content: some View, color: Color) -> some View {
        content
            .font(.system(size: 12))
            .foregroundStyle(color)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16).padding(.vertical, 8)
            .background(.bar)
    }

    private func notice(_ text: String, retry: Bool) -> some View {
        VStack(spacing: 12) {
            Text(text).foregroundStyle(Theme.secondary).multilineTextAlignment(.center)
            if retry {
                Button("Retry") { Task { await model.refresh() } }
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var footer: some View {
        TimelineView(.periodic(from: .now, by: 30)) { context in
            HStack(spacing: 6) {
                if model.refreshing && model.phase == .loaded { ProgressView().controlSize(.mini) }
                Text(updatedText(now: context.date))
            }
            .font(.system(size: 11))
            .foregroundStyle(Theme.secondary)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(.bar)
        }
    }

    private func updatedText(now: Date) -> String {
        guard let at = model.updatedAt else { return model.phase == .loading ? "Loading" : "Not updated yet" }
        let minutes = Int(now.timeIntervalSince(at) / 60)
        return minutes < 1 ? "Updated just now" : "Updated \(minutes) min ago"
    }
}

/// One row: needs-you dot, ref, priority, age / title / caller and assignee.
struct TicketRow: View {
    let ticket: Ticket
    let me: CurrentUser
    let showAgent: Bool

    private var needsYou: Bool { TicketList.needsYou(ticket, me: me) }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                if needsYou {
                    Circle().fill(Theme.accent).frame(width: 8, height: 8)
                        .accessibilityLabel("Needs you").help("Needs you")
                }
                Text(ticket.ref)
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(Theme.secondary)
                priority
                Spacer()
                if let start = ticket.startDate {
                    Text(Age.short(from: start))
                        .font(.system(size: 11)).foregroundStyle(Theme.secondary)
                        .help("Opened \(start.formatted(date: .abbreviated, time: .shortened))")
                }
            }
            Text(ticket.title)
                .font(.system(size: 13, weight: .medium))
                .lineLimit(2)
            HStack(spacing: 6) {
                Text(ticket.callerName).lineLimit(1)
                Spacer()
                if showAgent { agent }
            }
            .font(.system(size: 11))
            .foregroundStyle(Theme.secondary)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var priority: some View {
        switch ticket.priority {
        case 1:
            Image(systemName: "exclamationmark")
                .font(.system(size: 11, weight: .black)).foregroundStyle(Theme.warning)
                .accessibilityLabel("Priority 1").help("Priority 1")
        case 2:
            Circle().fill(Theme.secondary).frame(width: 6, height: 6)
                .accessibilityLabel("Priority 2").help("Priority 2")
        default:
            EmptyView()
        }
    }

    @ViewBuilder
    private var agent: some View {
        if ticket.isUnassigned {
            HStack(spacing: 4) {
                Circle().strokeBorder(Theme.secondary, style: StrokeStyle(lineWidth: 1, dash: [2, 2]))
                    .frame(width: 14, height: 14)
                Text("Unassigned").fontWeight(.semibold)
            }
        } else if ticket.agentID == me.contactID {
            Text("You")
        } else {
            Text(ticket.agentName).lineLimit(1)
        }
    }
}

/// The right-hand pane until Task 4 brings the full ticket view.
struct TicketDetailPlaceholder: View {
    let ticket: Ticket?
    let webURL: URL?

    var body: some View {
        if let t = ticket {
            VStack(alignment: .leading, spacing: 10) {
                Text("\(t.ref) · \(TicketGroup.statusLabel(t.status))")
                    .font(.system(size: 12, design: .monospaced)).foregroundStyle(Theme.secondary)
                Text(t.title).font(.system(size: 22, weight: .semibold))
                Text("\(t.callerName) · \(t.orgName) · \(t.teamName)").foregroundStyle(Theme.secondary)
                Text("The full ticket view, with the description and logs, comes in the next build.")
                    .foregroundStyle(Theme.secondary).padding(.top, 8)
                if let webURL {
                    Link("Open in iTop on the web", destination: webURL)
                        .foregroundStyle(Theme.text)
                }
                Spacer()
            }
            .textSelection(.enabled)
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text("Select a ticket to see its details.")
                .foregroundStyle(Theme.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

extension TicketGroup {
    /// The status as people read it, for the detail header.
    static func statusLabel(_ status: String) -> String {
        switch status {
        case "escalated_ttr": "Escalated TTR"
        case "escalated_tto": "Escalated TTO"
        case "waiting_for_approval": "Waiting for approval"
        default: status.prefix(1).uppercased() + status.dropFirst().replacingOccurrences(of: "_", with: " ")
        }
    }
}
