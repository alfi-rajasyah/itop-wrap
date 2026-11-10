import ITopKit
import SwiftUI

/// The signed-in window: sidebar plus the selected section (DESIGN.md, Screens).
struct ShellView: View {
    @Environment(SessionModel.self) private var model
    let session: Session
    @State private var mine: TicketListModel
    @State private var team: TicketListModel
    @State private var confirmSignOut = false

    init(session: Session) {
        self.session = session
        _mine = State(initialValue: TicketListModel(view: .mine, session: session, source: DebugLaunch.demoSource(.mine)))
        _team = State(initialValue: TicketListModel(view: .team, session: session, source: DebugLaunch.demoSource(.team)))
    }

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            List(selection: $model.selection) {
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
                case .myTickets: TicketListView(model: mine)
                case .teamTickets: TicketListView(model: team)
                default: SectionPlaceholder(item: model.selection, session: session)
                }
            }
            .navigationTitle(model.selection.title)
            .navigationSplitViewColumnWidth(min: 300, ideal: 360, max: 460)
        } detail: {
            Group {
                if let list = currentList {
                    TicketDetailPlaceholder(ticket: list.ticket(list.selection), webURL: list.ticket(list.selection).flatMap(list.webURL))
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

/// Stands in for each section until its task lands, and shows what sign-in loaded.
struct SectionPlaceholder: View {
    let item: SidebarItem
    let session: Session

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("\(item.title) comes in the next build. Sign-in works: here's what iTop told the app about you.")
                .foregroundStyle(Theme.secondary)
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 8) {
                GridRow { label("Signed in as"); Text("\(session.user.displayName) (\(session.user.login))") }
                GridRow { label("Server"); Text(session.client.server.baseURL.absoluteString).font(.system(size: 12, design: .monospaced)) }
                GridRow { label("Your teams"); Text(session.user.teams.isEmpty ? "None found" : session.user.teams.map(\.name).joined(separator: ", ")) }
                GridRow { label("Profiles"); Text(session.user.profiles.joined(separator: ", ")).fixedSize(horizontal: false, vertical: true) }
                GridRow { label("Changes to iTop"); Text(session.client.allowsWrites ? "On" : "Off (read-only build)") }
            }
            .padding(14)
            .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
            Spacer()
        }
        .font(.system(size: 13))
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .textSelection(.enabled)
    }

    private func label(_ text: String) -> some View {
        Text(text).foregroundStyle(Theme.secondary)
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
