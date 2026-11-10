import AppKit
import ITopKit
import SwiftUI

@main
struct ITopWrapApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model: SessionModel = {
        let model = SessionModel(service: DebugLaunch.service)
        if let demo = DebugLaunch.demoSession { model.showDemo(demo) }
        return model
    }()

    var body: some Scene {
        WindowGroup("iTop Wrap") {
            RootView()
                .environment(model)
                .task { if model.session == nil { await model.restore() } }
        }
        // The window follows its content: the small sign-in form, then the resizable shell.
        .windowResizability(.contentSize)
        .commands {
            CommandGroup(after: .sidebar) {
                if model.session != nil {
                    Divider()
                    ForEach(SidebarItem.allCases) { item in
                        Button(item.title) { model.selection = item }
                            .keyboardShortcut(item.shortcut, modifiers: .command)
                    }
                }
            }
        }
    }
}

struct RootView: View {
    @Environment(SessionModel.self) private var model

    var body: some View {
        switch model.state {
        case .restoring:
            ProgressView("Signing in…")
                .controlSize(.small)
                .frame(width: 480, height: 320)
        case .signedOut(let notice):
            LoginView(notice: notice)
        case .unreachable(let message):
            UnreachableView(message: message)
        case .signedIn(let session):
            ShellView(session: session)
        }
    }
}

/// `swift run` starts a bare executable; make it a regular app with a Dock icon and a front window.
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        DebugLaunch.applyAppearance()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

/// Switches for checking the UI without your real account. Debug builds only; release builds ignore them.
///   ITOPWRAP_SETTINGS=<name>   keep server/username and the Keychain item apart from your real ones
///   ITOPWRAP_DEMO=1            open the shell with a made-up user; nothing is sent to any server
///   ITOPWRAP_APPEARANCE=light|dark
enum DebugLaunch {
    static var env: [String: String] { ProcessInfo.processInfo.environment }

    @MainActor static var service: SessionService {
        #if DEBUG
        if let suite = env["ITOPWRAP_SETTINGS"], let defaults = UserDefaults(suiteName: "ITopWrap.debug.\(suite)") {
            return SessionService(secrets: KeychainStore(service: "iTop Wrap debug \(suite)"), settings: SessionSettings(defaults: defaults))
        }
        #endif
        return SessionService()
    }

    static var demoSession: Session? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1", let server = try? ServerAddress("https://itop.example.com") else { return nil }
        let client = ITopClient(server: server, credentials: Credentials(user: "agent.demo", password: ""))
        let user = CurrentUser(userID: "1", contactID: "2", login: "agent.demo", displayName: "Demo Agent",
                               profiles: ["Support Agent", "REST Services User", "Time Tracker"],
                               teams: [Team(id: "10", name: "Network"), Team(id: "11", name: "Service desk")])
        return Session(client: client, user: user)
        #else
        return nil
        #endif
    }

    @MainActor static func applyAppearance() {
        #if DEBUG
        switch env["ITOPWRAP_APPEARANCE"] {
        case "light": NSApp.appearance = NSAppearance(named: .aqua)
        case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
        default: break
        }
        #endif
    }
}
