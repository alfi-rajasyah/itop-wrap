import ITopKit
import SwiftUI

/// My tickets / Team tickets (DESIGN.md: Ticket list; wireframe Main.dc.html).
struct TicketListView: View {
    @Bindable var model: TicketListModel
    /// Opens the Organizations view; `nil` hides the org filter (Team tickets).
    var openOrg: ((String, String) -> Void)? = nil

    var body: some View {
        content
            // The controls stay when the list is empty: with no open tickets, the View menu is
            // how you get to resolved and closed ones.
            .safeAreaInset(edge: .top, spacing: 0) {
                if model.phase == .loaded && !model.hasNoTeams { controls }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) { footer }
            .searchable(text: $model.query, placement: .toolbar, prompt: "R-000412 or VPN")
            .task(id: model.query) { await model.searchChanged() }
            .onChange(of: model.options.scope) { model.page = 1; Task { await model.refresh() } }
            .onChange(of: model.query) { model.page = 1 }
            .onChange(of: model.orgFilter) { model.page = 1 }
            .onChange(of: model.options.sort) { model.page = 1 }
            .onChange(of: model.options.pageSize) { model.page = 1 }
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
                notice(model.options.scope != .open ? "No tickets in this range. Try a longer range in the View menu."
                       : model.view == .mine ? "Nothing waiting on you. New assignments show up here." : "Your teams have no open tickets.", retry: false)
            } else {
                list
            }
        }
    }

    private var list: some View {
        List(selection: $model.selection.deferred(model.view == .mine ? "my-tickets" : "team-tickets")) {
            if model.orgFilter != nil && model.visibleSections.isEmpty && !model.isSearching {
                orgNoMatch.listRowSeparator(.hidden)
            }
            if let error = model.refreshError {
                Text("Couldn't refresh: \(error)")
                    .font(.system(size: 12)).foregroundStyle(Theme.warning)
                    .listRowSeparator(.hidden)
            }
            lookupSection
            let paged = model.currentPage
            let sections = paged.sections
            if model.isSearching && sections.isEmpty && model.lookup == .none && !isLookupPending {
                Text("No tickets in your list match “\(model.query.trimmingCharacters(in: .whitespaces))”.")
                    .foregroundStyle(Theme.secondary)
                    .listRowSeparator(.hidden)
            }
            ForEach(sections) { section in
                Section {
                    ForEach(section.tickets) { t in
                        TicketRow(ticket: t, me: model.me, showAgent: model.view == .team || t.isUnassigned,
                                  assignToMe: assignAction(t), spent: model.timer?.totals[t.id], running: model.timer?.running.flatMap { $0.ticketID == t.id ? $0 : nil })
                            .tag(t.id)
                    }
                } header: {
                    HStack {
                        Text(section.group.title)
                        Spacer()
                        Text("\(paged.groupTotal(section.group))").monospacedDigit()
                    }
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Theme.secondary)
                }
            }
        }
        .listStyle(.inset)
        .safeAreaInset(edge: .top, spacing: 0) { VStack(spacing: 0) { lookupStatus } }
    }

    /// The org filter and View menu sit above the list, not in it: a menu inside a list row
    /// that reloads that list made AppKit warn of a reentrant NSTableView operation. They also
    /// stay in view when you scroll.
    private var controls: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                if openOrg != nil, model.orgChoices.count > 1 || model.orgFilter != nil { orgFilterMenu }
                ListViewMenu(options: model.options)
            }
            ListViewSummary(options: model.options)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16).padding(.vertical, 8)
        .background(.bar)
    }

    private var isLookupPending: Bool {
        if case .searching = model.lookup { return true }
        return false
    }

    /// A building-icon menu: one organization at a time, "All organizations" resets, darker
    /// border when on (DESIGN.md). The label is capped so a long name can't push the column.
    private var orgFilterMenu: some View {
        Menu {
            Button { model.orgFilter = nil } label: {
                if model.orgFilter == nil { Label("All organizations", systemImage: "checkmark") } else { Text("All organizations") }
            }
            Divider()
            ForEach(model.orgChoices, id: \.self) { name in
                Button { model.toggleOrg(name) } label: {
                    if model.orgFilter == name { Label(name, systemImage: "checkmark") } else { Text(name) }
                }
            }
        } label: {
            FilterChip(text: model.orgFilter ?? "All organizations", symbol: "building.2", on: model.orgFilter != nil)
                .frame(maxWidth: 200, alignment: .leading)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.visible)
        // Its own width, up to 200 points: a flexible frame kept it 220 wide and pushed View away.
        .fixedSize()
        .help(model.orgFilter.map { "Showing tickets for \($0)" } ?? "Filter by organization")
        .accessibilityLabel("Filter by organization: \(model.orgFilter ?? "all")")
    }

    @ViewBuilder
    private var orgNoMatch: some View {
        if let name = model.orgFilter {
            VStack(alignment: .leading, spacing: 6) {
                Text("None of your tickets are for \(name).").foregroundStyle(Theme.secondary)
                if let id = model.orgID(named: name), let openOrg {
                    // Switches sections, so it waits until the click in this list row is handled.
                    Button("See all of its tickets") { DispatchQueue.main.async { openOrg(id, name) } }
                        .buttonStyle(.link).foregroundStyle(Theme.text)
                }
            }
        }
    }

    /// The found ticket, as the list's only extra section.
    @ViewBuilder
    private var lookupSection: some View {
        if case .found(let t) = model.lookup {
            Section("Search result: not in your queue") {
                TicketRow(ticket: t, me: model.me, showAgent: true, assignToMe: assignAction(t)).tag(t.id)
            }
        }
    }

    /// The row's "Assign to me" pill: opens the ticket and assigns it, when it can be assigned.
    private func assignAction(_ t: Ticket) -> (() -> Void)? {
        guard t.isUnassigned, model.canAssign(t), model.source.canWrite(t) else { return nil }
        return {
            model.selection = t.id
            Task { await model.assignToMe(t) }
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
            if let t = model.timer, let n = t.notice {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "timer").accessibilityHidden(true)
                    Text(n.text).fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 4)
                    if n.canRetry {
                        Button("Retry") { Task { await t.retry() } }.disabled(t.saving)
                    }
                    Button { t.clearNotice() } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain)
                        .help(n.canRetry ? "Discard this time" : "Dismiss")
                        .accessibilityLabel(n.canRetry ? "Discard this time" : "Dismiss")
                }
                .font(.system(size: 12))
                .foregroundStyle(n.isError ? Theme.warning : Theme.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16).padding(.vertical, 8)
                .background(.bar)
            }
            if let n = model.notice {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(n.text).fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 4)
                    Button { model.clearNotice() } label: { Image(systemName: "xmark") }
                        .buttonStyle(.plain)
                        .help("Dismiss")
                        .accessibilityLabel("Dismiss")
                }
                .font(.system(size: 12))
                .foregroundStyle(n.isError ? Theme.warning : Theme.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 16).padding(.vertical, 8)
                .background(.bar)
            }
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
            VStack(spacing: 4) {
                if model.phase == .loaded { Pager(page: model.currentPage, current: $model.page) }
                HStack(spacing: 6) {
                    if model.refreshing && model.phase == .loaded { ProgressView().controlSize(.mini) }
                    Text(updatedText(now: context.date))
                }
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
    var assignToMe: (() -> Void)? = nil
    /// Total time saved on the ticket, in seconds; nil when unknown.
    var spent: Int? = nil
    /// The timer, when it's running on this ticket.
    var running: RunningTimer? = nil

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
                if let running {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        Label(TimerModel.elapsed(since: running.start, now: context.date), systemImage: "timer")
                            .monospacedDigit()
                            .fontWeight(.semibold)
                            .foregroundStyle(Theme.text)
                    }
                    .help("Timer running")
                    .accessibilityLabel("Timer running")
                } else if let spent, spent > 0 {
                    Text(TimeFormat.clock(spent)).monospacedDigit().help("Time spent")
                        .accessibilityLabel("\(TimeFormat.clock(spent)) spent")
                }
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
        if ticket.isUnassigned, let assignToMe {
            Button(action: assignToMe) {
                Text("Assign to me")
                    .font(.system(size: 11, weight: .semibold))
                    .padding(.horizontal, 8).padding(.vertical, 2)
                    .overlay(Capsule().strokeBorder(Theme.secondary, style: StrokeStyle(lineWidth: 1, dash: [3, 2])))
                    .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .help("Assign \(ticket.ref) to you")
            .accessibilityLabel("Assign \(ticket.ref) to me")
        } else if ticket.isUnassigned {
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
