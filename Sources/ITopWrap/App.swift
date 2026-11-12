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
        // One main window; ⌘O or the menu bar brings it back after it's closed.
        Window("iTop Wrap", id: "main") {
            RootView()
                .environment(model)
                .task { if model.session == nil { await model.restore() } }
        }
        // The window follows its content: the small sign-in form, then the resizable shell.
        .windowResizability(.contentSize)
        .commands {
            CommandGroup(replacing: .newItem) {
                if let workspace = model.workspace {
                    NewTicketButton(workspace: workspace)
                }
                OpenMainWindowButton()
            }
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
        MenuBarScene(model: model)
    }
}

/// The menu bar item (PLAN.md Task 8c), there while you're signed in, window open or not.
struct MenuBarScene: Scene {
    let model: SessionModel

    var body: some Scene {
        MenuBarExtra(isInserted: .constant(model.workspace != nil)) {
            if let workspace = model.workspace {
                MenuBarPanelHost(workspace: workspace, model: model)
            }
        } label: {
            if let workspace = model.workspace {
                MenuBarLabel(workspace: workspace)
            } else {
                Image(nsImage: MenuBarIcon.image)
            }
        }
        .menuBarExtraStyle(.window)
    }
}

/// Opens tickets from the panel in the main window, reopening it if it was closed.
struct MenuBarPanelHost: View {
    let workspace: Workspace
    let model: SessionModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        MenuBarPanel(workspace: workspace) { ticket in
            if let ticket {
                model.selection = .myTickets
                workspace.mine.query = ""
                workspace.mine.selection = ticket.id
            }
            openWindow(id: "main")
            NSApp.activate()
        }
    }
}

struct NewTicketButton: View {
    let workspace: Workspace
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("New Ticket…") {
            openWindow(id: "main")
            NSApp.activate()
            workspace.requestNewTicket()
        }
        .keyboardShortcut("n", modifiers: .command)
    }
}

struct OpenMainWindowButton: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Open iTop Wrap") {
            openWindow(id: "main")
            NSApp.activate()
        }
        .keyboardShortcut("o", modifiers: .command)
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
        case .signedIn:
            if let workspace = model.workspace { ShellView(workspace: workspace) }
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

    /// Signed in, the menu bar item keeps working with the window closed (PLAN.md Task 8c).
    /// Signed out there's nothing left to use, so closing the window quits.
    @MainActor static var keepRunningWithoutWindows = false

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { !Self.keepRunningWithoutWindows }
}

/// Switches for checking the UI without your real account. Debug builds only; release builds ignore them.
///   ITOPWRAP_SETTINGS=<name>   keep server/username and the Keychain item apart from your real ones
///   ITOPWRAP_DEMO=1            open the shell with a made-up user; nothing is sent to any server
///   ITOPWRAP_APPEARANCE=light|dark
enum DebugLaunch {
    static var env: [String: String] { ProcessInfo.processInfo.environment }

    ///   ITOPWRAP_WRITE_TICKETS=R-079999,R-080001   switch writes on for these tickets only (testing on a real iTop)
    ///   ITOPWRAP_WRITE_NEW=1                       also allow creating tickets, and writes on the ones created
    @MainActor static var service: SessionService {
        #if DEBUG
        let refs = (env["ITOPWRAP_WRITE_TICKETS"] ?? "").split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).uppercased() }.filter { !$0.isEmpty }
        let newTickets = env["ITOPWRAP_WRITE_NEW"] == "1"
        let access: WriteAccess = refs.isEmpty && !newTickets ? .off : .tickets(refs, newTickets: newTickets)
        if let suite = env["ITOPWRAP_SETTINGS"], let defaults = UserDefaults(suiteName: "ITopWrap.debug.\(suite)") {
            return SessionService(secrets: KeychainStore(service: "iTop Wrap debug \(suite)"), settings: SessionSettings(defaults: defaults), writeAccess: access)
        }
        return SessionService(writeAccess: access)
        #else
        // Release: writes on (your decision, 2026-11-11). Debug builds stay read-only unless a
        // test switch above names tickets.
        return SessionService(writeAccess: .all)
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
                """,
                serviceID: "3", subcategoryID: "12"),
        ]
        let demoMe = demoSession?.user
        @Sendable func list(_ scope: TicketScope, assigned: [String: DemoWrites.Assignment], statuses: [String: (String, Date)],
                            created: [Ticket] = []) -> [Ticket] {
            var all = (base + closed + created).map { DemoWrites.apply(assigned[$0.ref], statuses[$0.ref], to: $0) }
                .filter { scope.includeClosed || !["resolved", "closed"].contains($0.status) }
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
        let demoServices = [ServiceRef(id: "3", name: "Network access"), ServiceRef(id: "4", name: "Workplace"), ServiceRef(id: "5", name: "Collaboration")]
        let demoSubcategories: [String: [ServiceRef]] = [
            "3": [ServiceRef(id: "12", name: "VPN"), ServiceRef(id: "13", name: "Wi-Fi")],
            "4": [ServiceRef(id: "20", name: "Laptop"), ServiceRef(id: "21", name: "Printer")],
            "5": [ServiceRef(id: "30", name: "Teams calls")],
        ]
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
                                default: try? await Task.sleep(for: .milliseconds(300)); return list(scope, assigned: await mock.assignments, statuses: await mock.statuses, created: await mock.created)
                                }
                            },
                            lookup: { ref in
                                if ref == "R-000512" { return found }
                                // Any demo ticket, including ones made with New ticket in this run, with their changes.
                                guard let t = (base + closed + (await mock.created)).first(where: { $0.ref == ref }) else { return nil }
                                return DemoWrites.apply(await mock.assignments[ref], await mock.statuses[ref], to: t)
                            },
                            detail: { t in
                                try? await Task.sleep(for: .milliseconds(300))
                                let base = details[t.ref] ?? TicketDetail(description: "Demo ticket without a description.", publicLog: [], privateLog: [], linkedCIs: [])
                                return await mock.merged(base, ref: t.ref)
                            },
                            image: { id in
                                try? await Task.sleep(for: .milliseconds(400))
                                if id == "900" { return InlineImageLoader.Image(data: demoPNG, mimetype: "image/png") }
                                guard let uploaded = await mock.image(id) else { throw InlineImageLoader.Failure.notFound }
                                return InlineImageLoader.Image(data: uploaded.data, mimetype: uploaded.mimetype)
                            },
                            attachments: { t in t.ref == "R-000412" ? [AttachmentRef(id: "1", name: "vpn-client.log"), AttachmentRef(id: "2", name: "gateway-config.txt")] : [] },
                            download: { a in AttachmentFile(data: Data("Demo attachment \(a.name)\n".utf8), filename: a.name, mimetype: "text/plain") },
                            addEntry: { t, text, images, isPrivate in
                                // A fake upload: about a second per image. "fail" in the text fails the entry;
                                // "refuse" fails the image upload with an iTop-style message.
                                try? await Task.sleep(for: .milliseconds(400 + 900 * images.count))
                                if text.localizedCaseInsensitiveContains("fail") { throw ITopError.server(code: 100, message: "Error: demo failure on request") }
                                if !images.isEmpty, text.localizedCaseInsensitiveContains("refuse") {
                                    throw ITopError.server(code: 100, message: "Error: contents: demo refusal, the image was not stored")
                                }
                                guard !images.isEmpty else {
                                    await mock.add(text.trimmingCharacters(in: .whitespacesAndNewlines), ref: t.ref, isPrivate: isPrivate)
                                    return
                                }
                                var uploaded: [TicketWrites.UploadedImage] = []
                                for image in images { uploaded.append(await mock.upload(image)) }
                                let html = TicketWrites.entryHTML(text: text, images: uploaded, base: URL(string: "https://itop.example.com")!)
                                await mock.add(text.trimmingCharacters(in: .whitespacesAndNewlines), html: html, ref: t.ref, isPrivate: isPrivate)
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
                            agents: { id in try? await Task.sleep(for: .milliseconds(200)); return demoAgents[id] ?? [] },
                            change: { t, change in
                                try? await Task.sleep(for: .milliseconds(400))
                                guard TicketLifecycle.actions(for: t.status).contains(change.action) else { throw TicketWrites.Failure.notAllowed(status: t.status) }
                                let next: String
                                switch change {
                                case .pending(let reason):
                                    if reason.localizedCaseInsensitiveContains("fail") { throw ITopError.server(code: 100, message: "Error: demo failure on pending") }
                                    next = "pending"
                                case .resolve(let r):
                                    if r.solution.localizedCaseInsensitiveContains("fail") { throw ITopError.server(code: 100, message: "Error: servicesubcategory_id: the subcategory doesn't belong to this service") }
                                    next = "resolved"
                                case .reopen: next = "assigned"
                                case .close: next = "closed"
                                }
                                await mock.setStatus(t.ref, next)
                            },
                            services: { _ in try? await Task.sleep(for: .milliseconds(200)); return demoServices },
                            subcategories: { id in try? await Task.sleep(for: .milliseconds(200)); return demoSubcategories[id] ?? [] })
        #else
        return nil
        #endif
    }

    /// Demo time tracking: totals in memory, a web stopwatch running on R-000405.
    ///   ITOPWRAP_DEMO_TIMEFAIL=1   saving time fails, to check Retry
    static func demoTimer() -> TimerSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1" else { return nil }
        let mock = DemoWrites.shared
        let fail = env["ITOPWRAP_DEMO_TIMEFAIL"] == "1"
        let webSince = Date().addingTimeInterval(-25 * 60)
        return TimerSource(
            save: { entry, t in
                try? await Task.sleep(for: .milliseconds(400))
                if fail { throw ITopError.server(code: 100, message: "Error: You don't have the Time Tracker profile") }
                await mock.addTime(entry.duration, to: t.id)
            },
            webStopwatch: { t in t.ref == "R-000405" ? webSince : nil },
            totals: { tickets in
                let saved = await mock.time
                let base = ["UserRequest::412": 4500, "UserRequest::405": 1800, "UserRequest::367": 9000]
                return Dictionary(uniqueKeysWithValues: tickets.map { ($0.id, (base[$0.id] ?? 0) + (saved[$0.id] ?? 0)) })
            })
        #else
        return nil
        #endif
    }

    /// Demo New ticket: the wireframe's orgs, callers and services; created tickets get R-0006xx.
    static func demoNewTickets() -> NewTicketSource? {
        #if DEBUG
        guard env["ITOPWRAP_DEMO"] == "1" else { return nil }
        let orgs = [ServiceRef(id: "1", name: "Org A"), ServiceRef(id: "2", name: "Org B"), ServiceRef(id: "3", name: "Org C")]
        let callers: [String: [ServiceRef]] = [
            "1": [ServiceRef(id: "21", name: "Caller One"), ServiceRef(id: "23", name: "Caller Three")],
            "2": [ServiceRef(id: "22", name: "Caller Two"), ServiceRef(id: "27", name: "Caller Seven")],
            "3": [ServiceRef(id: "24", name: "Caller Four")],
        ]
        let services = [ServiceRef(id: "3", name: "Network access"), ServiceRef(id: "4", name: "Workplace"), ServiceRef(id: "5", name: "Collaboration")]
        let subcategories: [String: [ServiceRef]] = [
            "3": [ServiceRef(id: "12", name: "VPN"), ServiceRef(id: "13", name: "Wi-Fi")],
            "4": [ServiceRef(id: "20", name: "Laptop"), ServiceRef(id: "21", name: "Printer")],
            "5": [ServiceRef(id: "30", name: "Teams calls")],
        ]
        let counter = DemoWrites.shared
        return NewTicketSource(
            create: { t in
                try? await Task.sleep(for: .milliseconds(500))
                if t.title.localizedCaseInsensitiveContains("fail") {
                    throw ITopError.server(code: 100, message: "Error: servicesubcategory_id: not allowed for this request type")
                }
                let n = await counter.nextTicketNumber()
                let org = orgs.first { $0.id == t.orgID }?.name ?? ""
                let caller = callers[t.orgID]?.first { $0.id == t.callerID }?.name ?? ""
                return await counter.remember(Ticket(key: String(n), ticketClass: t.ticketClass, ref: (t.ticketClass == .incident ? "I-" : "R-") + String(format: "%06d", n),
                              title: t.title.trimmingCharacters(in: .whitespacesAndNewlines), status: "new", priority: 4, callerName: caller,
                              orgName: org, teamID: "0", teamName: "", agentID: "0", agentName: "", startDate: Date(), lastUpdate: Date(), orgID: t.orgID))
            },
            organizations: { try? await Task.sleep(for: .milliseconds(200)); return orgs },
            callers: { id in try? await Task.sleep(for: .milliseconds(200)); return callers[id] ?? [] },
            services: { _ in try? await Task.sleep(for: .milliseconds(200)); return services },
            subcategories: { id in try? await Task.sleep(for: .milliseconds(200)); return subcategories[id] ?? [] },
            canCreate: true)
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

    private var added: [String: [(text: String, html: String?, isPrivate: Bool, date: Date)]] = [:]
    /// Images "uploaded" in the demo, by InlineImage id (901 on; 900 is the canned one).
    private var images: [String: LogImage] = [:]
    private var lastImage = 900

    func upload(_ image: LogImage) -> TicketWrites.UploadedImage {
        lastImage += 1
        images[String(lastImage)] = image
        return TicketWrites.UploadedImage(id: String(lastImage), secret: String(repeating: "0", count: 32), image: image)
    }

    func image(_ id: String) -> LogImage? { images[id] }
    private(set) var assignments: [String: Assignment] = [:]
    /// Status changes from lifecycle actions; they win over an assignment's status when newer.
    private(set) var statuses: [String: (String, Date)] = [:]

    func assign(_ ref: String, team: Team, agent: Agent, status: String) {
        assignments[ref] = Assignment(team: team, agent: agent, status: status, date: Date())
        statuses[ref] = nil
    }

    private(set) var time: [String: Int] = [:]
    private var lastTicket = 600

    /// Tickets made with New ticket, so lists and ref search find them like iTop would.
    private(set) var created: [Ticket] = []

    func remember(_ t: Ticket) -> Ticket {
        created.append(t)
        return t
    }

    func nextTicketNumber() -> Int {
        lastTicket += 1
        return lastTicket
    }

    func addTime(_ seconds: Int, to id: String) {
        time[id, default: 0] += seconds
    }

    func setStatus(_ ref: String, _ status: String) {
        statuses[ref] = (status, Date())
    }

    nonisolated static func apply(_ a: Assignment?, _ s: (String, Date)?, to t: Ticket) -> Ticket {
        guard a != nil || s != nil else { return t }
        return Ticket(key: t.key, ticketClass: t.ticketClass, ref: t.ref, title: t.title, status: s?.0 ?? a?.status ?? t.status,
                      priority: t.priority, callerName: t.callerName, orgName: t.orgName, teamID: a?.team.id ?? t.teamID,
                      teamName: a?.team.name ?? t.teamName, agentID: a?.agent.id ?? t.agentID, agentName: a?.agent.name ?? t.agentName,
                      startDate: t.startDate, lastUpdate: max(s?.1 ?? .distantPast, a?.date ?? .distantPast), orgID: t.orgID)
    }

    func add(_ text: String, html: String? = nil, ref: String, isPrivate: Bool) {
        added[ref, default: []].append((text, html, isPrivate, Date()))
    }

    func merged(_ base: TicketDetail, ref: String) -> TicketDetail {
        let extra = added[ref] ?? []
        func entries(_ priv: Bool, from start: Int) -> [LogEntry] {
            extra.filter { $0.isPrivate == priv }.enumerated().map { i, e in
                LogEntry(id: start + i, date: e.date, author: "Demo Agent", text: e.html.map(HTMLText.plain) ?? e.text, html: e.html)
            }
        }
        return TicketDetail(description: base.description, publicLog: base.publicLog + entries(false, from: 1000),
                            privateLog: base.privateLog + entries(true, from: 2000), linkedCIs: base.linkedCIs,
                            serviceName: base.serviceName, subcategoryName: base.subcategoryName, descriptionHTML: base.descriptionHTML,
                            serviceID: base.serviceID, subcategoryID: base.subcategoryID)
    }
}
