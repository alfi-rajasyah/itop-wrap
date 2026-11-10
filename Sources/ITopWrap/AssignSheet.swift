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
                DropdownField(name: "Team", placeholder: placeholder(teams, empty: "No teams found", loading: "Loading teams…"),
                              items: teams.value ?? [], selection: $team, label: \.name)
                    .disabled(teams.value == nil)
            }
            field("Agent") {
                DropdownField(name: "Agent", placeholder: team == nil ? "Choose a team first" : placeholder(agents, empty: "Nobody in this team", loading: "Loading agents…"),
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
                    .cancelShortcutUnlessDropdownOpen()
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
        .dropdownHost()
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
