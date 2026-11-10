import ITopKit
import SwiftUI

// MARK: - Shared ticket table

/// The org and people ticket lists: grouped by status, with a Ref / Title / Caller / Assignee header.
struct TicketTable: View {
    let tickets: [Ticket]
    let me: CurrentUser
    /// The last column: the assignee on org lists and "Reported by them", the caller on "Assigned to them".
    var lastColumn: LastColumn = .assignee
    var sort: TicketSort = .priority
    var pageSize: Int? = nil
    var openTicket: (Ticket) -> Void
    @State private var page = 1

    enum LastColumn { case assignee, caller }

    var body: some View {
        let paged = TicketPage(TicketList.groups(tickets, me: me, view: .team, sort: sort), size: pageSize, page: page)
        let sections = paged.sections
        // Lazy, so an agent with hundreds of tickets doesn't build every row at once.
        LazyVStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Text("Ref").frame(width: 96, alignment: .leading)
                Text("Title").frame(maxWidth: .infinity, alignment: .leading)
                if lastColumn == .assignee { Text("Caller").frame(width: 110, alignment: .leading) }
                Text(lastColumn == .assignee ? "Assignee" : "Caller").frame(width: 130, alignment: .leading)
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(Theme.secondary)
            .padding(.horizontal, 10).padding(.bottom, 6)
            .accessibilityHidden(true)
            ForEach(sections) { section in
                HStack {
                    Text(section.group.title)
                    Spacer()
                    Text("\(paged.groupTotal(section.group))").monospacedDigit()
                }
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Theme.secondary)
                .padding(.horizontal, 10).padding(.top, 12).padding(.bottom, 4)
                ForEach(section.tickets) { t in
                    Button { openTicket(t) } label: { row(t) }
                        .buttonStyle(.plain)
                        .help("Open \(t.ref)")
                    Divider()
                }
            }
            Pager(page: paged, current: $page)
                .frame(maxWidth: .infinity)
                .padding(.top, 12)
        }
        .onChange(of: tickets) { page = 1 }
        .onChange(of: sort) { page = 1 }
        .onChange(of: pageSize) { page = 1 }
    }

    private func row(_ t: Ticket) -> some View {
        HStack(spacing: 10) {
            HStack(spacing: 5) {
                if TicketList.needsYou(t, me: me) {
                    Circle().fill(Theme.accent).frame(width: 8, height: 8).accessibilityLabel("Needs you")
                }
                Text(t.ref).font(.system(size: 11.5, design: .monospaced)).foregroundStyle(Theme.secondary)
                if t.priority == 1 {
                    Image(systemName: "exclamationmark").font(.system(size: 11, weight: .black))
                        .foregroundStyle(Theme.warning).accessibilityLabel("Priority 1")
                }
            }
            .frame(width: 96, alignment: .leading)
            Text(t.title).fontWeight(.medium).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
            if lastColumn == .assignee {
                Text(t.callerName).foregroundStyle(Theme.secondary).lineLimit(1).frame(width: 110, alignment: .leading)
                PersonChip(name: t.isUnassigned ? nil : (t.agentID == me.contactID ? "You" : t.agentName),
                           initials: t.agentID == me.contactID ? me.initials : nil)
                    .frame(width: 130, alignment: .leading)
            } else {
                Text(t.callerName).foregroundStyle(Theme.secondary).lineLimit(1).frame(width: 130, alignment: .leading)
            }
        }
        .font(.system(size: 12.5))
        .padding(.horizontal, 10).padding(.vertical, 8)
        .contentShape(Rectangle())
    }
}

/// Initials avatar plus name, or a dashed circle and a bold "Unassigned" (DESIGN.md: Unassigned tickets).
struct PersonChip: View {
    let name: String?
    var initials: String?

    var body: some View {
        HStack(spacing: 5) {
            if let name {
                Text(initials ?? name.split(separator: " ").prefix(2).compactMap(\.first).map { String($0).uppercased() }.joined())
                    .font(.system(size: 8.5, weight: .semibold)).foregroundStyle(Theme.secondary)
                    .frame(width: 18, height: 18).background(Theme.chip, in: Circle())
                Text(name).lineLimit(1)
            } else {
                Circle().strokeBorder(Theme.secondary, style: StrokeStyle(lineWidth: 1, dash: [2, 2])).frame(width: 18, height: 18)
                Text("Unassigned").fontWeight(.semibold)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Organizations

struct OrgsListView: View {
    @Bindable var model: OrgsModel

    var body: some View {
        Group {
            switch model.orgs {
            case .idle, .loading:
                VStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text("Loading organizations from iTop").foregroundStyle(Theme.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let error):
                VStack(spacing: 10) {
                    Text("Couldn't load organizations. \(error)").foregroundStyle(Theme.warning).multilineTextAlignment(.center)
                    Button("Retry") { Task { await model.reloadList() } }
                }
                .padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
            case .loaded:
                let orgs = model.visible
                if orgs.isEmpty {
                    Text(model.query.isEmpty ? "Your iTop account can't see any organizations." : "No organization matches “\(model.query.trimmingCharacters(in: .whitespaces))”.")
                        .foregroundStyle(Theme.secondary).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(orgs, selection: $model.selectedID.deferred("organizations")) { org in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(org.name).fontWeight(.medium)
                                if !org.code.isEmpty { Text(org.code).font(.system(size: 11)).foregroundStyle(Theme.secondary) }
                            }
                            Spacer()
                            Text("\(org.openCount)").monospacedDigit().foregroundStyle(Theme.secondary)
                                .help("\(org.openCount) open tickets").accessibilityLabel("\(org.openCount) open tickets")
                        }
                        .padding(.vertical, 3)
                        .tag(org.id)
                    }
                    .listStyle(.inset)
                }
            }
        }
        .searchable(text: $model.query, placement: .toolbar, prompt: "Search organizations")
        .task { await model.loadList() }
    }
}

struct OrgDetailView: View {
    let model: OrgsModel
    let org: Organization
    var openTicket: (Ticket) -> Void
    var openPeople: (Organization) -> Void
    var openConfigItems: (Organization) -> Void
    var newTicket: (Organization) -> Void = { _ in }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(org.code.isEmpty ? "Organization" : "Organization · \(org.code)").font(.system(size: 12)).foregroundStyle(Theme.secondary)
                    Text(org.name).font(.system(size: 22, weight: .semibold))
                }
                HStack(spacing: 16) {
                    Button { openPeople(org) } label: {
                        Label(model.counts.map { "People \($0.people)" } ?? "People", systemImage: "person.text.rectangle")
                    }
                    Button { openConfigItems(org) } label: {
                        Label(model.counts.map { "Config items \($0.configItems)" } ?? "Config items", systemImage: "server.rack")
                    }
                    Button { newTicket(org) } label: {
                        Label("New ticket", systemImage: "square.and.pencil")
                    }
                    .help("New ticket for \(org.name)")
                }
                .buttonStyle(.link)
                .foregroundStyle(Theme.text)
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(model.options.includeClosed ? "Tickets" : "Open tickets").font(.system(size: 14, weight: .semibold))
                        ListViewSummary(options: model.options)
                    }
                    Spacer()
                    ListViewMenu(options: model.options)
                }
                switch model.tickets {
                case .idle, .loading:
                    HStack(spacing: 8) { ProgressView().controlSize(.small); Text("Loading \(org.name)'s tickets").foregroundStyle(Theme.secondary) }
                case .failed(let error):
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Couldn't load the tickets. \(error)").foregroundStyle(Theme.warning)
                        Button("Retry") { Task { await model.loadSelected() } }
                    }
                case .loaded(let tickets) where tickets.isEmpty:
                    Text(model.options.scope == .open ? "\(org.name) has no open tickets." : "\(org.name) has no tickets in this range.")
                        .foregroundStyle(Theme.secondary)
                case .loaded(let tickets):
                    TicketTable(tickets: tickets, me: model.me, sort: model.options.sort, pageSize: model.options.pageSize, openTicket: openTicket)
                }
            }
            .padding(.horizontal, 24).padding(.vertical, 18)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(.system(size: 13))
        .task(id: org.id) { await model.loadSelected() }
        .onChange(of: model.options.scope) { Task { await model.loadSelected() } }
    }
}

// MARK: - People

struct PeopleListView: View {
    @Bindable var model: PeopleModel

    var body: some View {
        VStack(spacing: 0) {
            if let scope = model.scope {
                HStack {
                    Label("People in \(scope.name)", systemImage: "building.2").font(.system(size: 12))
                    Spacer()
                    Button { model.clearScope() } label: { Image(systemName: "xmark.circle.fill") }
                        .buttonStyle(.plain).foregroundStyle(Theme.secondary)
                        .help("Show search instead").accessibilityLabel("Clear organization")
                }
                .padding(.horizontal, 14).padding(.vertical, 8)
                .background(.bar)
            }
            content
        }
        .searchable(text: $model.query, placement: .toolbar, prompt: "Search people by name")
        .task(id: model.query + (model.scope?.id ?? "")) { await model.search() }
    }

    @ViewBuilder
    private var content: some View {
        switch model.results {
        case .idle:
            Text("Search by name, for example a caller or an agent.")
                .foregroundStyle(Theme.secondary).multilineTextAlignment(.center)
                .padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loading:
            VStack(spacing: 10) { ProgressView().controlSize(.small); Text("Searching iTop").foregroundStyle(Theme.secondary) }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let error):
            Text("Couldn't search iTop. \(error)").foregroundStyle(Theme.warning).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let people) where people.isEmpty:
            Text(model.scope != nil ? "Nobody in this organization." : "Nobody matches “\(model.query.trimmingCharacters(in: .whitespaces))”.")
                .foregroundStyle(Theme.secondary).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let people):
            if model.total > people.count {
                Text(model.scope != nil
                     ? "Showing \(people.count) of \(model.total) people. Search by name to find someone."
                     : "Showing \(people.count) of \(model.total) people. Type more of the name to narrow it down.")
                    .font(.system(size: 12)).foregroundStyle(Theme.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(.bar)
            }
            List(people, selection: $model.selectedID.deferred("people")) { p in
                HStack(spacing: 10) {
                    Text(p.initials).font(.system(size: 10, weight: .semibold)).foregroundStyle(Theme.secondary)
                        .frame(width: 28, height: 28).background(Theme.chip, in: Circle())
                    VStack(alignment: .leading, spacing: 2) {
                        Text(p.displayName).fontWeight(.medium).lineLimit(1)
                        Text(p.subtitle).font(.system(size: 11)).foregroundStyle(Theme.secondary).lineLimit(1)
                    }
                }
                .padding(.vertical, 2)
                .tag(p.id)
            }
            .listStyle(.inset)
        }
    }
}

struct PersonDetailView: View {
    @Bindable var model: PeopleModel
    let person: Person
    var openTicket: (Ticket) -> Void
    var openOrg: (String, String) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 12) {
                    Text(person.initials).font(.system(size: 15, weight: .semibold)).foregroundStyle(Theme.secondary)
                        .frame(width: 44, height: 44).background(Theme.chip, in: Circle())
                    VStack(alignment: .leading, spacing: 2) {
                        Text(person.displayName).font(.system(size: 22, weight: .semibold)).textSelection(.enabled)
                        Text([person.isAgent ? "Agent" : "Caller", person.function].filter { !$0.isEmpty }.joined(separator: " · "))
                            .foregroundStyle(Theme.secondary)
                    }
                }
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                    if !person.orgName.isEmpty {
                        GridRow {
                            label("Organization")
                            Button(person.orgName) { openOrg(person.orgID, person.orgName) }
                                .buttonStyle(.link).foregroundStyle(Theme.text)
                        }
                    }
                    if !person.email.isEmpty {
                        GridRow {
                            label("Email")
                            if let url = URL(string: "mailto:\(person.email)") {
                                Link(person.email, destination: url).foregroundStyle(Theme.text)
                            }
                        }
                    }
                    if !person.phone.isEmpty { GridRow { label("Phone"); Text(person.phone).textSelection(.enabled) } }
                    if person.isAgent { GridRow { label("Teams"); Text(person.teams.map(\.name).joined(separator: ", ")) } }
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Theme.chip.opacity(0.6), in: RoundedRectangle(cornerRadius: 8))

                HStack {
                    Picker("Tickets", selection: $model.tab) {
                        Text("Assigned to them\(count(model.assigned))").tag(PeopleModel.Tab.assigned)
                        Text("Reported by them\(count(model.reported))").tag(PeopleModel.Tab.reported)
                    }
                    .pickerStyle(.segmented).labelsHidden().fixedSize()
                    Spacer()
                    ListViewMenu(options: model.options)
                }
                ListViewSummary(options: model.options)

                let current = model.tab == .assigned ? model.assigned : model.reported
                switch current {
                case .idle, .loading:
                    HStack(spacing: 8) { ProgressView().controlSize(.small); Text("Loading tickets").foregroundStyle(Theme.secondary) }
                case .failed(let error):
                    Text("Couldn't load the tickets. \(error)").foregroundStyle(Theme.warning)
                case .loaded(let tickets) where tickets.isEmpty:
                    let kind = model.options.scope == .open ? "open tickets" : "tickets in this range"
                    Text(model.tab == .assigned ? "No \(kind) assigned to \(person.displayName)." : "No \(kind) reported by \(person.displayName).")
                        .foregroundStyle(Theme.secondary)
                case .loaded(let tickets):
                    TicketTable(tickets: tickets, me: model.me, lastColumn: model.tab == .assigned ? .caller : .assignee,
                                sort: model.options.sort, pageSize: model.options.pageSize, openTicket: openTicket)
                }
            }
            .padding(.horizontal, 24).padding(.vertical, 18)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(.system(size: 13))
        .task(id: person.id) { await model.loadSelected() }
        .onChange(of: model.options.scope) { Task { await model.loadSelected(keepTab: true) } }
    }

    private func label(_ text: String) -> some View { Text(text).foregroundStyle(Theme.secondary) }

    private func count(_ state: Loadable<[Ticket]>) -> String {
        if case .loaded(let t) = state { return " \(t.count)" }
        return ""
    }
}
