import AppKit
import ITopKit
import SwiftUI

/// The ticket on the right (DESIGN.md: Ticket detail; wireframe Main.dc.html). Read-only for now:
/// actions, the log composer and the timer arrive with the write tasks.
struct TicketDetailView: View {
    let list: TicketListModel
    var openCI: (LinkedCI) -> Void
    @State private var model: TicketDetailModel

    init(ticket: Ticket, list: TicketListModel, openCI: @escaping (LinkedCI) -> Void) {
        self.list = list
        self.openCI = openCI
        _model = State(initialValue: TicketDetailModel(ticket: ticket, source: list.source))
    }

    private var ticket: Ticket { model.ticket }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                strip
                switch model.phase {
                case .loading:
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading the ticket from iTop").foregroundStyle(Theme.secondary)
                    }
                    .padding(.top, 8)
                case .failed(let message):
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Couldn't load this ticket. \(message)").foregroundStyle(Theme.warning)
                            .fixedSize(horizontal: false, vertical: true)
                        Button("Retry") { Task { await model.load() } }
                    }
                case .loaded(let detail):
                    if !detail.serviceName.isEmpty {
                        Text([detail.serviceName, detail.subcategoryName].filter { !$0.isEmpty }.joined(separator: " › "))
                            .font(.system(size: 12)).foregroundStyle(Theme.secondary)
                            .padding(.top, -6)
                    }
                    description(detail)
                    logs(detail)
                }
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 18)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(.system(size: 13))
        .task(id: ticket.lastUpdate) { await model.load() }
    }

    // MARK: Header

    private var header: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Button {
                        Task { await model.copyRef { ref in
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(ref, forType: .string)
                        } }
                    } label: {
                        Text(ticket.ref).font(.system(size: 12, design: .monospaced))
                    }
                    .buttonStyle(.plain)
                    .help("Copy \(ticket.ref)")
                    .accessibilityLabel("\(ticket.ref), copy ref")
                    Image(systemName: ticket.ticketClass == .userRequest ? "questionmark.bubble" : "exclamationmark.triangle")
                        .accessibilityLabel(ticket.ticketClass == .userRequest ? "User request" : "Incident")
                        .help(ticket.ticketClass == .userRequest ? "User request" : "Incident")
                    Text(TicketGroup.statusLabel(ticket.status))
                    if TicketList.needsYou(ticket, me: list.me) {
                        Text("Needs you")
                            .font(.system(size: 11, weight: .semibold))
                            .padding(.horizontal, 7).padding(.vertical, 1)
                            .foregroundStyle(Theme.accentText)
                            .background(Theme.accentTint, in: Capsule())
                    }
                }
                .font(.system(size: 12))
                .foregroundStyle(Theme.secondary)
                Text(ticket.title)
                    .font(.system(size: 22, weight: .semibold))
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
                if model.copied {
                    Label("Copied \(ticket.ref)", systemImage: "checkmark")
                        .font(.system(size: 12))
                        .padding(.horizontal, 8).padding(.vertical, 2)
                        .background(Theme.chip, in: RoundedRectangle(cornerRadius: 5))
                        .accessibilityAddTraits(.updatesFrequently)
                }
            }
            Spacer(minLength: 0)
            Menu {
                Button("Copy ref \(ticket.ref)") {
                    Task { await model.copyRef { ref in
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(ref, forType: .string)
                    } }
                }
                if let url = list.webURL(ticket) {
                    Link("Open in iTop on the web", destination: url)
                }
            } label: {
                Image(systemName: "ellipsis")
            }
            .menuIndicator(.hidden)
            .fixedSize()
            .help("More actions")
            .accessibilityLabel("More actions")
        }
    }

    // MARK: Details strip

    private var strip: some View {
        FlowLayout(spacing: 16, lineSpacing: 8) {
            item("person", ticket.callerName, help: "Caller")
            item("building.2", ticket.orgName, help: "Organization")
            item("person.2", ticket.teamName.isEmpty ? "No team" : ticket.teamName, help: "Team")
            agent
            if case .loaded(let detail) = model.phase {
                ForEach(detail.linkedCIs) { ci in
                    Button { openCI(ci) } label: {
                        Label(ci.name, systemImage: "server.rack")
                            .font(.system(size: 12, design: .monospaced))
                            .underline()
                    }
                    .buttonStyle(.plain)
                    .help("Linked CI: open \(ci.name) in Config items")
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.chip.opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
    }

    private func item(_ symbol: String, _ text: String, help: String) -> some View {
        Label(text.isEmpty ? "None" : text, systemImage: symbol)
            .help(help)
            .accessibilityLabel("\(help): \(text.isEmpty ? "none" : text)")
    }

    @ViewBuilder
    private var agent: some View {
        if ticket.isUnassigned {
            HStack(spacing: 5) {
                Circle().strokeBorder(Theme.secondary, style: StrokeStyle(lineWidth: 1, dash: [2, 2])).frame(width: 18, height: 18)
                Text("Unassigned").fontWeight(.semibold)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Agent: unassigned")
        } else {
            let mine = ticket.agentID == list.me.contactID
            let name = mine ? "You" : ticket.agentName
            HStack(spacing: 5) {
                Text(mine ? list.me.initials : initials(ticket.agentName))
                    .font(.system(size: 8.5, weight: .semibold))
                    .foregroundStyle(Theme.secondary)
                    .frame(width: 18, height: 18)
                    .background(Theme.chip, in: Circle())
                Text(name)
            }
            .help("Agent")
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Agent: \(name)")
        }
    }

    private func initials(_ name: String) -> String {
        name.split(separator: " ").prefix(2).compactMap(\.first).map { String($0).uppercased() }.joined()
    }

    // MARK: Description and logs

    private func description(_ detail: TicketDetail) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Description").font(.system(size: 12, weight: .semibold))
            Text(detail.description.isEmpty ? "No description." : detail.description)
                .foregroundStyle(detail.description.isEmpty ? Theme.secondary : Theme.text)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
    }

    private func logs(_ detail: TicketDetail) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("Log", selection: $model.tab) {
                Text("Public log").tag(TicketDetailModel.Tab.public)
                // macOS segmented controls drop icons, even inline ones; the lock is on each private entry.
                Text("Private log").tag(TicketDetailModel.Tab.private)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()

            let isPublic = model.tab == .public
            let entries = isPublic ? detail.publicLog : detail.privateLog
            if entries.isEmpty {
                Text(isPublic ? "No public entries yet." : "No private entries yet. Only agents can see the private log.")
                    .foregroundStyle(Theme.secondary)
            }
            ForEach(entries) { e in
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 4) {
                        if !isPublic { Image(systemName: "lock").accessibilityLabel("Private") }
                        Text([e.author, e.date.map { LogDate.short($0) }].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                    }
                    .font(.system(size: 11))
                    .foregroundStyle(Theme.secondary)
                    Text(e.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.separator))
            }
        }
    }
}

/// Lays children out left to right and wraps them onto new lines, like the wireframe's details strip.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8
    var lineSpacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let width = rows.map { $0.width }.max() ?? 0
        let height = rows.reduce(0) { $0 + $1.height } + lineSpacing * CGFloat(max(0, rows.count - 1))
        return CGSize(width: min(width, proposal.width ?? width), height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y + (row.height - size.height) / 2), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + lineSpacing
        }
    }

    private struct Row { var indices: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = [Row()]
        for (i, view) in subviews.enumerated() {
            let size = view.sizeThatFits(.unspecified)
            let needed = rows[rows.count - 1].indices.isEmpty ? size.width : rows[rows.count - 1].width + spacing + size.width
            if needed > width && !rows[rows.count - 1].indices.isEmpty {
                rows.append(Row())
            }
            var row = rows.removeLast()
            row.width = row.indices.isEmpty ? size.width : row.width + spacing + size.width
            row.height = max(row.height, size.height)
            row.indices.append(i)
            rows.append(row)
        }
        return rows.filter { !$0.indices.isEmpty }
    }
}
