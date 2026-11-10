import ITopKit
import SwiftUI

/// Assign or reassign a ticket (DESIGN.md: Unassigned tickets, Sheets; PLAN.md Task 7).
/// "Assign to me" sits at the top, then a team and an agent from that team.
/// Escape cancels, ⌘↩ assigns.
struct AssignSheet: View {
    let ticket: Ticket
    let list: TicketListModel
    @Environment(\.dismiss) private var dismiss

    @State private var teams: Loadable<[Team]> = .loading
    @State private var agents: Loadable<[Agent]> = .loading
    @State private var team: Team?
    @State private var agent: Agent?
    @State private var error: String?

    private var busy: Bool { list.working != nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text(ticket.isUnassigned ? "Assign \(ticket.ref)" : "Reassign \(ticket.ref)")
                    .font(.system(size: 15, weight: .semibold))
                Text(ticket.title).foregroundStyle(Theme.secondary).lineLimit(2)
            }

            Button {
                Task {
                    await list.assignToMe(ticket)
                    if list.notice?.isError == false { dismiss() } else { error = list.notice?.text }
                }
            } label: {
                Text("Assign to me")
            }
            .buttonStyle(PrimaryButtonStyle())
            .disabled(busy || ticket.agentID == list.me.contactID)
            .help(ticket.agentID == list.me.contactID ? "It's already yours" : "Assign \(ticket.ref) to you")

            Text("or choose a team and agent")
                .font(.system(size: 12))
                .foregroundStyle(Theme.secondary)
                .frame(maxWidth: .infinity)

            field("Team") {
                DropdownField(placeholder: placeholder(teams, empty: "No teams found", loading: "Loading teams…"),
                              items: teams.value ?? [], selection: $team, label: \.name)
                    .disabled(teams.value == nil)
            }
            field("Agent") {
                DropdownField(placeholder: team == nil ? "Choose a team first" : placeholder(agents, empty: "Nobody in this team", loading: "Loading agents…"),
                              items: team == nil ? [] : (agents.value ?? []), selection: $agent,
                              label: { $0.id == list.me.contactID ? "\($0.name) (you)" : $0.name })
                    .disabled(team == nil || agents.value == nil)
            }

            if let error {
                Text(error).font(.system(size: 12)).foregroundStyle(Theme.warning).fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Spacer()
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button {
                    guard let team, let agent else { return }
                    Task {
                        if let message = await list.assign(ticket, team: team, agent: agent) { error = message } else { dismiss() }
                    }
                } label: {
                    HStack(spacing: 6) {
                        if busy { ProgressView().controlSize(.mini) }
                        Text(ticket.isUnassigned ? "Assign ticket" : "Reassign ticket")
                    }
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.primaryFill)
                .foregroundStyle(Theme.onPrimary)
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(busy || team == nil || agent == nil)
            }
        }
        .padding(20)
        .frame(width: 380)
        .task { await loadTeams() }
        .onChange(of: team) { _, new in
            agent = nil
            if let new { Task { await loadAgents(new) } }
        }
    }

    private func field(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(Theme.secondary)
            content()
        }
    }

    private func placeholder<T>(_ state: Loadable<[T]>, empty: String, loading: String) -> String {
        switch state {
        case .idle, .loading: return loading
        case .failed(let message): return "Couldn't load: \(message)"
        case .loaded(let items): return items.isEmpty ? empty : "Choose…"
        }
    }

    private func loadTeams() async {
        do {
            let all = try await list.source.teams()
            teams = .loaded(all)
            // Start on the ticket's own team, so a reassign within the team is one pick.
            team = all.first { $0.id == ticket.teamID }
        } catch {
            teams = .failed(ErrorText.describe(error))
        }
    }

    private func loadAgents(_ chosen: Team) async {
        agents = .loading
        do {
            let found = try await list.source.agents(chosen.id)
            guard team == chosen else { return }
            agents = .loaded(found)
        } catch {
            agents = .failed(ErrorText.describe(error))
        }
    }
}

/// The one picker used across the app (DESIGN.md: "One dropdown component"): a field that opens
/// a filterable list in a popover. Typing narrows it; Return picks the first match.
struct DropdownField<Item: Identifiable & Hashable>: View {
    let placeholder: String
    let items: [Item]
    @Binding var selection: Item?
    let label: (Item) -> String

    init(placeholder: String, items: [Item], selection: Binding<Item?>, label: @escaping (Item) -> String) {
        self.placeholder = placeholder
        self.items = items
        _selection = selection
        self.label = label
    }

    init(placeholder: String, items: [Item], selection: Binding<Item?>, label: KeyPath<Item, String>) {
        self.init(placeholder: placeholder, items: items, selection: selection, label: { $0[keyPath: label] })
    }

    @State private var open = false
    @State private var filter = ""
    @Environment(\.isEnabled) private var isEnabled

    private var matches: [Item] {
        let words = filter.lowercased().split(separator: " ").map(String.init)
        guard !words.isEmpty else { return items }
        return items.filter { item in let l = label(item).lowercased(); return words.allSatisfy { l.contains($0) } }
    }

    var body: some View {
        Button {
            filter = ""
            open = true
        } label: {
            HStack {
                Text(selection.map(label) ?? placeholder)
                    .foregroundStyle(selection == nil ? Theme.secondary : Theme.text)
                    .lineLimit(1)
                Spacer(minLength: 6)
                Image(systemName: "chevron.up.chevron.down").font(.system(size: 10)).foregroundStyle(Theme.secondary)
            }
            .padding(.horizontal, 10).frame(height: 30)
            .background(Theme.chip.opacity(isEnabled ? 0.6 : 0.3), in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).stroke(Theme.separator))
            .contentShape(RoundedRectangle(cornerRadius: 7))
        }
        .buttonStyle(.plain)
        .accessibilityValue(selection.map(label) ?? "none")
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 0) {
                TextField("Filter", text: $filter)
                    .textFieldStyle(.roundedBorder)
                    .padding(8)
                    .onSubmit { if let first = matches.first { pick(first) } }
                Divider()
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(matches) { item in
                            Button { pick(item) } label: {
                                HStack {
                                    Text(label(item)).lineLimit(1)
                                    Spacer()
                                    if item == selection { Image(systemName: "checkmark").font(.system(size: 11)) }
                                }
                                .padding(.horizontal, 10).padding(.vertical, 5)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                        if matches.isEmpty {
                            Text("Nothing matches “\(filter)”.").foregroundStyle(Theme.secondary).padding(10)
                        }
                    }
                }
                .frame(maxHeight: 260)
            }
            .frame(width: 300)
        }
    }

    private func pick(_ item: Item) {
        selection = item
        open = false
    }
}
