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

    /// Canned tickets from the wireframe sample data for the demo session; `nil` outside it.
    static func demoSource(_ view: TicketList.View) -> TicketSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1" else { return nil }
        let now = Date()
        func t(_ ref: String, _ title: String, _ status: String, agent: String, agentName: String = "", team: String = "10",
               teamName: String = "Network", prio: Int = 3, hours: Double, caller: String, org: String = "Org A") -> Ticket {
            Ticket(key: String(ref.suffix(3)), ticketClass: ref.hasPrefix("I") ? .incident : .userRequest, ref: ref, title: title,
                   status: status, priority: prio, callerName: caller, orgName: org, teamID: team, teamName: teamName,
                   agentID: agent, agentName: agentName, startDate: now.addingTimeInterval(-hours * 3600), lastUpdate: nil)
        }
        let mine = [
            t("R-000367", "Badge reader offline at the entrance", "escalated_ttr", agent: "2", team: "11", teamName: "Service desk", prio: 2, hours: 96, caller: "Caller 6"),
            t("I-000088", "Mail delivery delayed", "escalated_tto", agent: "0", team: "11", teamName: "Service desk", prio: 2, hours: 5, caller: "Caller 2", org: "Org B"),
            t("R-000412", "VPN drops every hour", "assigned", agent: "2", prio: 1, hours: 2, caller: "Caller 1"),
            t("R-000405", "Shared drive access for a new project", "assigned", agent: "2", team: "11", teamName: "Service desk", hours: 26, caller: "Caller 3"),
            t("R-000398", "Laptop setup for a new starter", "pending", agent: "2", team: "11", teamName: "Service desk", hours: 50, caller: "Caller 4", org: "Org C"),
            t("R-000371", "Printer queue stuck on floor 3", "pending", agent: "2", team: "11", teamName: "Service desk", prio: 4, hours: 74, caller: "Caller 5"),
        ]
        let others = [
            t("R-000421", "Email signature update for the sales team", "assigned", agent: "3", agentName: "Agent 3", team: "11", teamName: "Service desk", prio: 4, hours: 24, caller: "Caller 8"),
            t("R-000433", "Projector in the boardroom shows no signal", "escalated_ttr", agent: "4", agentName: "Agent 4", prio: 2, hours: 72, caller: "Caller 10", org: "Org C"),
            t("R-000436", "New phone for the reception desk", "new", agent: "0", team: "11", teamName: "Service desk", prio: 4, hours: 0.7, caller: "Caller 11", org: "Org B"),
        ]
        let found = t("R-000512", "Teams calls drop on the Finance floor", "assigned", agent: "5", agentName: "Agent 2", team: "12", teamName: "Collaboration", hours: 6, caller: "Caller 7", org: "Org B")
        let list = view == .mine ? mine : mine + others
        func entry(_ id: Int, _ who: String, hoursAgo: Double, _ text: String) -> LogEntry {
            LogEntry(id: id, date: now.addingTimeInterval(-hoursAgo * 3600), author: who, text: text)
        }
        let details: [String: TicketDetail] = [
            "R-000412": TicketDetail(
                description: "Since Monday the VPN disconnects at the top of every hour, sometimes in the middle of a call. Reconnecting works straight away.\nHappens both in the office and at home. Laptop: [model], VPN client: [version].",
                publicLog: [entry(1, "Caller 1", hoursAgo: 2.8, "The VPN disconnects at the top of every hour and I have to sign in again."),
                            entry(2, "Demo Agent", hoursAgo: 1.3, "Thanks, checking the gateway session timeout now."),
                            entry(3, "Caller 1", hoursAgo: 0.9, "It happened again at 11:00.")],
                privateLog: [entry(4, "Demo Agent", hoursAgo: 1.25, "Gateway timeout looks like 60 min. Confirm with Team Network before changing.")],
                linkedCIs: [LinkedCI(id: "55", name: "vpn-gw-01", className: "NetworkDevice")],
                serviceName: "Network access", subcategoryName: "VPN"),
        ]
        return TicketSource(load: { try? await Task.sleep(for: .milliseconds(300)); return list },
                            lookup: { $0 == "R-000512" ? found : nil },
                            detail: { t in
                                try? await Task.sleep(for: .milliseconds(300))
                                return details[t.ref] ?? TicketDetail(description: "Demo ticket without a description.", publicLog: [], privateLog: [], linkedCIs: [])
                            })
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
