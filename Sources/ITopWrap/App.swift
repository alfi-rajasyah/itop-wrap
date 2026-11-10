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
        DebugLaunch.redirectStderr()
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

    ///   ITOPWRAP_WRITE_TICKETS=R-079999,R-080001   switch writes on for these tickets only (testing on a real iTop)
    @MainActor static var service: SessionService {
        #if DEBUG
        let refs = (env["ITOPWRAP_WRITE_TICKETS"] ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).uppercased() }.filter { !$0.isEmpty }
        let access: WriteAccess = refs.isEmpty ? .off : .tickets(refs)
        if let suite = env["ITOPWRAP_SETTINGS"], let defaults = UserDefaults(suiteName: "ITopWrap.debug.\(suite)") {
            return SessionService(secrets: KeychainStore(service: "iTop Wrap debug \(suite)"), settings: SessionSettings(defaults: defaults), writeAccess: access)
        }
        return SessionService(writeAccess: access)
        #else
        return SessionService()
        #endif
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

    /// Where list view settings live: apart from your real settings when ITOPWRAP_SETTINGS is set.
    static var listDefaults: UserDefaults {
        #if DEBUG
        if let suite = env["ITOPWRAP_SETTINGS"], let d = UserDefaults(suiteName: "ITopWrap.debug.\(suite)") { return d }
        #endif
        return .standard
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
                   agentID: agent, agentName: agentName, startDate: now.addingTimeInterval(-hours * 3600), lastUpdate: nil,
                   orgID: ["Org A": "1", "Org B": "2", "Org C": "3"][org] ?? "0")
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
        let closed = [
            t("R-000355", "Monitor arm for desk 14", "resolved", agent: "2", team: "11", teamName: "Service desk", prio: 4, hours: 200, caller: "Caller 12"),
            t("R-000349", "Guest Wi-Fi password for the event", "closed", agent: "2", prio: 3, hours: 420, caller: "Caller 13", org: "Org B"),
        ]
        // ITOPWRAP_DEMO_MANY=1: a big Org A, to check long lists.
        let many = env["ITOPWRAP_DEMO_MANY"] == "1"
            ? (1...60).map { i in t(String(format: "R-%06d", 500 + i), "Bulk demo ticket \(i)", ["assigned", "pending", "new"][i % 3],
                                    agent: i % 4 == 0 ? "0" : "7", agentName: "Agent 7", prio: 1 + i % 4, hours: Double(i * 5), caller: "Caller \(i)") }
            : []
        let base = (view == .mine ? mine : mine + others) + (view == .team ? many : [])
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
                serviceName: "Network access", subcategoryName: "VPN",
                descriptionHTML: """
                <p>Since Monday the VPN disconnects at the top of every hour, sometimes in the middle of a call. Reconnecting works straight away.<br>Happens both in the office and at home. Laptop: [model], VPN client: [version].</p>
                <p>The error the client shows (see the <a href="https://kb.example.com/vpn-timeouts">VPN timeout article</a>):</p>
                <p><img src="pages/ajax.document.php?operation=download_inlineimage&amp;id=900&amp;s=demo" data-img-id="900"></p>
                <p>A screenshot the caller linked from elsewhere:</p>
                <p><img src="https://images.example.net/screenshot.png"></p>
                <p><b>Caller 1</b><br>Sales<br><img src="https://lh5.googleusercontent.com/phone-icon" width="16" height="16"> : +00 0000 0001<br><img src="https://lh6.googleusercontent.com/mail-icon" width="16" height="16"> : <a href="mailto:caller.one@example.com">caller.one@example.com</a></p>
                """),
        ]
        let demoMe = demoSession?.user
        @Sendable func list(_ scope: TicketScope, assigned: [String: DemoWrites.Assignment]) -> [Ticket] {
            var all = (base + (scope.includeClosed ? closed : [])).map { DemoWrites.apply(assigned[$0.ref], to: $0) }
            // My tickets after an assign: mine, or unassigned TTO in my teams (the real query's rule).
            if view == .mine, let me = demoMe {
                let myTeams = Set(me.teams.map(\.id))
                all = all.filter { t in t.agentID == me.contactID || (t.status == "escalated_tto" && t.isUnassigned && myTeams.contains(t.teamID)) }
            }
            guard let days = scope.openedWithinDays else { return all }
            return all.filter { ($0.startDate ?? now) >= now.addingTimeInterval(-Double(days) * 86400) }
        }
        // A drawn PNG stands in for a pasted iTop image.
        let demoPNG: Data = {
            // Wide on purpose: the full-size viewer has to fit screenshots wider than the window.
            let img = NSImage(size: NSSize(width: 2400, height: 900))
            img.lockFocus()
            NSColor(calibratedWhite: 0.92, alpha: 1).setFill(); NSRect(x: 0, y: 0, width: 2400, height: 900).fill()
            NSColor(calibratedWhite: 0.6, alpha: 1).setFill(); NSRect(x: 2300, y: 0, width: 100, height: 900).fill()
            NSColor(calibratedRed: 0.77, green: 0.16, blue: 0.11, alpha: 1).setFill(); NSRect(x: 24, y: 120, width: 200, height: 28).fill()
            ("VPN disconnected: session timeout (60 min)" as NSString).draw(at: NSPoint(x: 24, y: 70), withAttributes: [.font: NSFont.systemFont(ofSize: 18)])
            img.unlockFocus()
            return NSBitmapImageRep(data: img.tiffRepresentation ?? Data())?.representation(using: .png, properties: [:]) ?? Data()
        }()
        // The demo's mock iTop for writes: added entries are kept in memory and show in the logs.
        let mock = DemoWrites.shared
        let demoTeams = [Team(id: "10", name: "Network"), Team(id: "11", name: "Service desk"), Team(id: "12", name: "Collaboration")]
        let demoAgents: [String: [Agent]] = [
            "10": [Agent(id: "2", name: "Demo Agent"), Agent(id: "4", name: "Agent 4")],
            "11": [Agent(id: "2", name: "Demo Agent"), Agent(id: "3", name: "Agent 3")],
            "12": [Agent(id: "5", name: "Agent 2")],
        ]
        // ITOPWRAP_DEMO_STATE=empty|loading|error forces a list state for the Delivery Gate.
        let state = env["ITOPWRAP_DEMO_STATE"] ?? ""
        return TicketSource(load: { scope in
                                switch state {
                                case "empty": return []
                                case "loading": try? await Task.sleep(for: .seconds(3600)); return []
                                case "error": throw ITopError.transport(URLError.Code.notConnectedToInternet.rawValue)
                                default: try? await Task.sleep(for: .milliseconds(300)); return list(scope, assigned: await mock.assignments)
                                }
                            },
                            lookup: { $0 == "R-000512" ? found : nil },
                            detail: { t in
                                try? await Task.sleep(for: .milliseconds(300))
                                let base = details[t.ref] ?? TicketDetail(description: "Demo ticket without a description.", publicLog: [], privateLog: [], linkedCIs: [])
                                return await mock.merged(base, ref: t.ref)
                            },
                            image: { id in
                                try? await Task.sleep(for: .milliseconds(400))
                                guard id == "900" else { throw InlineImageLoader.Failure.notFound }
                                return InlineImageLoader.Image(data: demoPNG, mimetype: "image/png")
                            },
                            attachments: { t in t.ref == "R-000412" ? [AttachmentRef(id: "1", name: "vpn-client.log"), AttachmentRef(id: "2", name: "gateway-config.txt")] : [] },
                            download: { a in AttachmentFile(data: Data("Demo attachment \(a.name)\n".utf8), filename: a.name, mimetype: "text/plain") },
                            addEntry: { t, text, isPrivate in
                                try? await Task.sleep(for: .milliseconds(400))
                                if text.localizedCaseInsensitiveContains("fail") { throw ITopError.server(code: 100, message: "Error: demo failure on request") }
                                await mock.add(text.trimmingCharacters(in: .whitespacesAndNewlines), ref: t.ref, isPrivate: isPrivate)
                            },
                            canWrite: { _ in true },
                            assign: { t, teamID, agentID in
                                try? await Task.sleep(for: .milliseconds(400))
                                guard let stimulus = TicketWrites.assignStimulus(for: t.status) else { throw TicketWrites.Failure.cannotAssign(status: t.status) }
                                let team = demoTeams.first { $0.id == teamID }
                                let agent = demoAgents[teamID]?.first { $0.id == agentID }
                                guard let team, let agent else { throw ITopError.server(code: 100, message: "Error: agent_id: the agent is not in the team") }
                                await mock.assign(t.ref, team: team, agent: agent, status: stimulus == "ev_assign" || t.status == "pending" ? "assigned" : t.status)
                            },
                            assignToMe: { t in
                                try? await Task.sleep(for: .milliseconds(400))
                                // R-000436 plays the race: a colleague took it a moment ago.
                                if t.ref == "R-000436" { throw TicketWrites.Failure.alreadyTaken(by: "Agent 3") }
                                let team = demoTeams.first { $0.id == t.teamID } ?? demoTeams[0]
                                await mock.assign(t.ref, team: team, agent: Agent(id: "2", name: "Demo Agent"), status: "assigned")
                            },
                            teams: { try? await Task.sleep(for: .milliseconds(200)); return demoTeams },
                            agents: { id in try? await Task.sleep(for: .milliseconds(200)); return demoAgents[id] ?? [] })
        #else
        return nil
        #endif
    }

    /// The wireframe's three CIs for the demo session; `nil` outside it.
    static func demoCMDB() -> CMDBSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1" else { return nil }
        let gw1 = ConfigItem(key: "55", className: "NetworkDevice", name: "vpn-gw-01", orgName: "Org A")
        let gw2 = ConfigItem(key: "56", className: "NetworkDevice", name: "vpn-gw-02", orgName: "Org A")
        let app = ConfigItem(key: "70", className: "ApplicationSolution", name: "Remote access VPN", orgName: "Org A")
        let device = [CIField(label: "Status", value: "Production"), CIField(label: "Organization", value: "Org A"),
                      CIField(label: "Location", value: "[Location]"), CIField(label: "Brand", value: "[Brand]"),
                      CIField(label: "Model", value: "[Model]"), CIField(label: "Serial number", value: "[Serial]"),
                      CIField(label: "Management IP", value: "[IP address]"), CIField(label: "Contacts", value: "Team Network")]
        func ticket(_ ref: String, _ title: String, _ status: String, agent: String, name: String) -> Ticket {
            Ticket(key: String(ref.suffix(3)), ticketClass: .userRequest, ref: ref, title: title, status: status, priority: 3,
                   callerName: "Caller", orgName: "Org A", teamID: "10", teamName: "Network", agentID: agent, agentName: name,
                   startDate: Date().addingTimeInterval(-7200), lastUpdate: nil)
        }
        let vpnTicket = ticket("R-000412", "VPN drops every hour", "assigned", agent: "2", name: "Demo Agent")
        let details: [String: ConfigItemDetail] = [
            gw1.id: ConfigItemDetail(item: gw1, fields: device, description: "", dependsOn: [],
                                     usedBy: [CIRef(id: app.key, name: app.name, className: app.className)], openTickets: [vpnTicket]),
            gw2.id: ConfigItemDetail(item: gw2, fields: [CIField(label: "Status", value: "Standby")] + device.dropFirst(), description: "",
                                     dependsOn: [], usedBy: [CIRef(id: app.key, name: app.name, className: app.className)], openTickets: []),
            app.id: ConfigItemDetail(item: app, fields: [CIField(label: "Status", value: "Active"), CIField(label: "Organization", value: "Org A"),
                                                         CIField(label: "Business criticality", value: "High"), CIField(label: "Contacts", value: "Team Network")],
                                     description: "Remote access for staff working away from the office.",
                                     dependsOn: [CIRef(id: gw1.key, name: gw1.name, className: gw1.className),
                                                 CIRef(id: gw2.key, name: gw2.name, className: gw2.className)],
                                     usedBy: [],
                                     openTickets: [vpnTicket, ticket("R-000498", "VPN profile for a contractor", "escalated_ttr", agent: "5", name: "Agent 2")]),
        ]
        let state = env["ITOPWRAP_DEMO_STATE"] ?? ""
        return CMDBSource(
            search: { q in
                if state == "error" { throw ITopError.transport(URLError.Code.notConnectedToInternet.rawValue) }
                if state == "empty" { return [] }
                try? await Task.sleep(for: .milliseconds(200))
                return [app, gw1, gw2].filter { $0.name.localizedCaseInsensitiveContains(q) }
                    .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
            },
            detail: { item in
                try? await Task.sleep(for: .milliseconds(200))
                return details[item.id] ?? ConfigItemDetail(item: item, fields: [], description: "", dependsOn: [], usedBy: [], openTickets: [])
            },
            inOrg: { id in id == "1" ? [app, gw1, gw2] : [] })
        #else
        return nil
        #endif
    }

    /// Demo orgs: the wireframe's Org A/B/C, with tickets from every agent.
    static func demoOrgs() -> OrgSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1", let tickets = demoSource(.team) else { return nil }
        let ids = ["Org A": "1", "Org B": "2", "Org C": "3"]
        return OrgSource(
            list: {
                let all = try await tickets.load(.open)
                return ids.sorted { $0.key < $1.key }.map { name, id in
                    Organization(id: id, name: name, code: String(name.suffix(1)), openCount: all.filter { $0.orgName == name }.count)
                }
            },
            tickets: { id, scope in try await tickets.load(scope).filter { ids[$0.orgName] == id } },
            counts: { id in (id == "1" ? 6 : 3, id == "1" ? 3 : 0) })
        #else
        return nil
        #endif
    }

    /// Demo people: callers and agents from the wireframe.
    static func demoPeople() -> PeopleSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1", let tickets = demoSource(.team) else { return nil }
        let people = [
            Person(id: "2", displayName: "Demo Agent", orgID: "1", orgName: "Org A", email: "agent.demo@example.com", phone: "",
                   function: "IT support", teams: [Team(id: "10", name: "Network"), Team(id: "11", name: "Service desk")]),
            Person(id: "4", displayName: "Agent Four", orgID: "1", orgName: "Org A", email: "agent.four@example.com", phone: "",
                   function: "Facilities", teams: [Team(id: "10", name: "Network")]),
            Person(id: "21", displayName: "Caller One", orgID: "1", orgName: "Org A", email: "caller.one@example.com",
                   phone: "+00 0000 0001", function: "Sales", teams: []),
            Person(id: "22", displayName: "Caller Two", orgID: "2", orgName: "Org B", email: "caller.two@example.com", phone: "", function: "", teams: []),
        ]
        let callerOf = ["21": "Caller 1", "22": "Caller 2"]
        return PeopleSource(
            search: { q in
                let found = people.filter { p in q.split(separator: " ").allSatisfy { p.displayName.localizedCaseInsensitiveContains($0) } }
                return (found, found.count)
            },
            inOrg: { id in let found = people.filter { $0.orgID == id }; return (found, found.count) },
            assigned: { id, scope in try await tickets.load(scope).filter { $0.agentID == id } },
            reported: { id, scope in try await tickets.load(scope).filter { $0.callerName == callerOf[id] } })
        #else
        return nil
        #endif
    }

    /// ITOPWRAP_STDERR=<file>: copy AppKit's warnings to a file, for a debug app opened from Finder.
    static func redirectStderr() {
        #if DEBUG
        if let path = env["ITOPWRAP_STDERR"] { freopen(path, "a", stderr) }
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

/// The demo's mock iTop for writes (debug only): log entries and assignments, shared by every list.
actor DemoWrites {
    static let shared = DemoWrites()

    struct Assignment: Sendable {
        let team: Team
        let agent: Agent
        let status: String
        let date: Date
    }

    private var added: [String: [(text: String, isPrivate: Bool, date: Date)]] = [:]
    private(set) var assignments: [String: Assignment] = [:]

    func assign(_ ref: String, team: Team, agent: Agent, status: String) {
        assignments[ref] = Assignment(team: team, agent: agent, status: status, date: Date())
    }

    nonisolated static func apply(_ a: Assignment?, to t: Ticket) -> Ticket {
        guard let a else { return t }
        return Ticket(key: t.key, ticketClass: t.ticketClass, ref: t.ref, title: t.title, status: a.status, priority: t.priority,
                      callerName: t.callerName, orgName: t.orgName, teamID: a.team.id, teamName: a.team.name, agentID: a.agent.id,
                      agentName: a.agent.name, startDate: t.startDate, lastUpdate: a.date, orgID: t.orgID)
    }

    func add(_ text: String, ref: String, isPrivate: Bool) {
        added[ref, default: []].append((text, isPrivate, Date()))
    }

    func merged(_ base: TicketDetail, ref: String) -> TicketDetail {
        let extra = added[ref] ?? []
        func entries(_ priv: Bool, from start: Int) -> [LogEntry] {
            extra.filter { $0.isPrivate == priv }.enumerated().map { i, e in
                LogEntry(id: start + i, date: e.date, author: "Demo Agent", text: e.text)
            }
        }
        return TicketDetail(description: base.description, publicLog: base.publicLog + entries(false, from: 1000),
                            privateLog: base.privateLog + entries(true, from: 2000), linkedCIs: base.linkedCIs,
                            serviceName: base.serviceName, subcategoryName: base.subcategoryName, descriptionHTML: base.descriptionHTML)
    }
}
