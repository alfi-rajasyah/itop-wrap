import ITopKit
import Observation
import SwiftUI

enum SidebarItem: String, CaseIterable, Identifiable, Hashable {
    case myTickets, teamTickets, configItems, organizations, people

    var id: String { rawValue }

    var title: String {
        switch self {
        case .myTickets: "My tickets"
        case .teamTickets: "Team tickets"
        case .configItems: "Config items"
        case .organizations: "Organizations"
        case .people: "People"
        }
    }

    var symbol: String {
        switch self {
        case .myTickets: "tray"
        case .teamTickets: "person.2"
        case .configItems: "server.rack"
        case .organizations: "building.2"
        case .people: "person.text.rectangle"
        }
    }

    /// ⌘1 to ⌘5, in sidebar order (DESIGN.md, Keyboard).
    var shortcut: KeyEquivalent { KeyEquivalent(Character(String(Self.allCases.firstIndex(of: self)! + 1))) }
}

/// The app's sign-in state. Views read it; only this model talks to `SessionService`.
@MainActor @Observable
final class SessionModel {
    enum State {
        case restoring
        case signedOut(notice: String?)
        case signedIn(Session)
        case unreachable(String)
    }

    private(set) var state: State = .restoring {
        didSet { updateWorkspace() }
    }
    /// The lists and the timer for the signed-in session, shared by the window and the menu bar.
    private(set) var workspace: Workspace?
    var selection: SidebarItem = .myTickets
    var server: String
    var user: String

    private let service: SessionService

    init(service: SessionService = SessionService()) {
        self.service = service
        server = service.savedServer ?? "https://"
        user = service.savedUser ?? ""
    }

    var session: Session? {
        if case .signedIn(let s) = state { return s }
        return nil
    }

    /// On launch: use the saved password if there is one.
    func restore() async {
        do {
            if let session = try await service.restore() {
                state = .signedIn(session)
            } else {
                state = .signedOut(notice: nil)
            }
        } catch ITopError.invalidCredentials {
            state = .signedOut(notice: "Your password was rejected. Sign in again.")
        } catch {
            state = .unreachable(error.localizedDescription)
        }
    }

    /// Throws `ITopError` for the form to show; on success the window switches to the app.
    func signIn(password: String) async throws {
        let session = try await service.signIn(server: server, user: user, password: password)
        server = session.client.server.baseURL.absoluteString
        selection = .myTickets
        state = .signedIn(session)
    }

    /// Debug only: show the shell for a made-up session (see `DebugLaunch`).
    func showDemo(_ session: Session) {
        state = .signedIn(session)
    }

    private func updateWorkspace() {
        if let session {
            guard workspace?.session.user.userID != session.user.userID || workspace?.session.client.server != session.client.server else { return }
            workspace?.stop()
            workspace = Workspace(session: session)
            workspace?.start()
            AppDelegate.keepRunningWithoutWindows = true
        } else {
            workspace?.stop()
            workspace = nil
            AppDelegate.keepRunningWithoutWindows = false
        }
    }

    func signOut() {
        if let session { try? service.signOut(session) }
        state = .signedOut(notice: nil)
    }

    /// From the "can't reach the server" screen.
    func forgetAndSignIn() {
        state = .signedOut(notice: nil)
    }
}
