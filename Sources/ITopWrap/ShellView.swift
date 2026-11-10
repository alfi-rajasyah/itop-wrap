import ITopKit
import SwiftUI

/// The signed-in window: sidebar plus the selected section (DESIGN.md, Screens).
struct ShellView: View {
    @Environment(SessionModel.self) private var model
    let session: Session
    @State private var options: ListOptions
    @State private var mine: TicketListModel
    @State private var team: TicketListModel
    @State private var cmdb: CMDBModel
    @State private var orgs: OrgsModel
    @State private var people: PeopleModel
    @State private var confirmSignOut = false

    init(session: Session) {
        self.session = session
        ErrorText.host = session.client.server.baseURL.host() ?? "the server"
        let options = ListOptions(defaults: DebugLaunch.listDefaults)
        _options = State(initialValue: options)
        _mine = State(initialValue: TicketListModel(view: .mine, session: session, options: options, source: DebugLaunch.demoSource(.mine)))
        _team = State(initialValue: TicketListModel(view: .team, session: session, options: options, source: DebugLaunch.demoSource(.team)))
        _cmdb = State(initialValue: CMDBModel(session: session, source: DebugLaunch.demoCMDB()))
        _orgs = State(initialValue: OrgsModel(session: session, options: options, source: DebugLaunch.demoOrgs()))
        _people = State(initialValue: PeopleModel(session: session, options: options, source: DebugLaunch.demoPeople()))
    }

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            List(selection: $model.selection.deferred("sidebar")) {
                Section("Tickets") {
                    row(.myTickets, count: mine.needCount)
                    row(.teamTickets)
                }
                Section("CMDB") {
                    row(.configItems)
                    row(.organizations)
                    row(.people)
                }
            }
            .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 260)
            .safeAreaInset(edge: .bottom) { account }
        } content: {
            Group {
                switch model.selection {
                case .myTickets: TicketListView(model: mine, openOrg: openOrg)
                case .teamTickets: TicketListView(model: team)
                case .configItems: CMDBListView(model: cmdb)
                case .organizations: OrgsListView(model: orgs)
                case .people: PeopleListView(model: people)
                }
            }
            .navigationTitle(model.selection.title)
            .navigationSplitViewColumnWidth(min: 300, ideal: 360, max: 460)
        } detail: {
            Group {
                if let list = currentList {
                    if let t = list.ticket(list.selection) {
                        TicketDetailView(ticket: t, list: list, openCI: openCI).id(t.id)
                    } else {
                        Text("Select a ticket to see its details.")
                            .foregroundStyle(Theme.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                } else if model.selection == .configItems {
                    if let item = cmdb.current {
                        CIDetailView(model: cmdb, item: item, openTicket: openTicket).id(item.id)
                    } else {
                        Text("Select a config item to see its details.")
                            .foregroundStyle(Theme.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                } else if model.selection == .organizations {
                    if let org = orgs.selected {
                        OrgDetailView(model: orgs, org: org, openTicket: openTicket,
                                      openPeople: { o in people.showOrg(id: o.id, name: o.name); model.selection = .people },
                                      openConfigItems: { o in cmdb.showOrg(id: o.id, name: o.name); model.selection = .configItems })
                            .id(org.id)
                    } else {
                        placeholder("Select an organization to see its open tickets.")
                    }
                } else if model.selection == .people {
                    if let p = people.selected {
                        PersonDetailView(model: people, person: p, openTicket: openTicket, openOrg: openOrg).id(p.id)
                    } else {
                        placeholder("Select a person to see their details and tickets.")
                    }
                } else {
                    Color.clear
                }
            }
            // The minimum size goes on a column: a frame on the split view itself
            // pushes its content under the toolbar and past the bottom edge (Checkpoint A).
            .frame(minWidth: 380, minHeight: 520)
        }
        .confirmationDialog("Sign out of \(host)?", isPresented: $confirmSignOut) {
            Button("Sign out", role: .destructive) { model.signOut() }
        } message: {
            Text("Your password is removed from the Keychain. Your username stays filled in.")
        }
    }

    private func placeholder(_ text: String) -> some View {
        Text(text).foregroundStyle(Theme.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// "See all of its tickets" and a person's organization open the org view.
    private func openOrg(_ id: String, _ name: String) {
        orgs.query = ""
        orgs.selectedID = id
        model.selection = .organizations
    }

    /// A ticket's linked CI opens in Config items.
    private func openCI(_ ci: LinkedCI) {
        cmdb.open(ConfigItem(key: ci.id, className: ci.className, name: ci.name, orgName: ""))
        model.selection = .configItems
    }

    /// A CI's open ticket opens in the list that has it, or as a search in My tickets.
    private func openTicket(_ t: Ticket) {
        if mine.tickets.contains(where: { $0.id == t.id }) {
            mine.query = ""
            mine.selection = t.id
            model.selection = .myTickets
        } else if team.tickets.contains(where: { $0.id == t.id }) {
            team.query = ""
            team.selection = t.id
            model.selection = .teamTickets
        } else {
            mine.query = t.ref
            model.selection = .myTickets
        }
    }

    private var currentList: TicketListModel? {
        switch model.selection {
        case .myTickets: mine
        case .teamTickets: team
        default: nil
        }
    }

    private var host: String { session.client.server.baseURL.host() ?? "iTop" }

    private func row(_ item: SidebarItem, count: Int = 0) -> some View {
        HStack {
            Label(item.title, systemImage: item.symbol)
            Spacer()
            if count > 0 {
                Text("\(count)")
                    .font(.system(size: 11, weight: .semibold)).monospacedDigit()
                    .padding(.horizontal, 7).padding(.vertical, 1)
                    .foregroundStyle(Theme.accentText)
                    .background(Theme.accentTint, in: Capsule())
                    .accessibilityLabel("\(count) need you")
                    .help("\(count) need you")
            }
        }
        .tag(item)
    }

    private var account: some View {
        HStack {
            Menu {
                Text("\(session.user.displayName) · \(host)")
                Divider()
                Button("Sign out…") { confirmSignOut = true }
            } label: {
                Text(session.user.initials)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Theme.secondary)
                    .frame(width: 28, height: 28)
                    .background(Theme.chip, in: Circle())
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel("Account: signed in as \(session.user.displayName) on \(host)")
            .help("\(session.user.displayName) · \(host)")
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .overlay(alignment: .top) { Divider() }
    }
}

/// When the saved session can't be checked because the server is out of reach.
struct UnreachableView: View {
    @Environment(SessionModel.self) private var model
    let message: String

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Can't reach iTop").font(.system(size: 20, weight: .semibold))
            Text(message).foregroundStyle(Theme.secondary).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            Button("Try again") { Task { await model.restore() } }
                .buttonStyle(PrimaryButtonStyle())
                .keyboardShortcut(.defaultAction)
            Button("Sign in with another account") { model.forgetAndSignIn() }
                .buttonStyle(.link)
                .foregroundStyle(Theme.text)
                .frame(maxWidth: .infinity)
        }
        .font(.system(size: 13))
        .padding(.horizontal, 48).padding(.top, 20).padding(.bottom, 32)
        .frame(width: 480, height: 320)
    }
}
