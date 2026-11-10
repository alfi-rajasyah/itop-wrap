import AppKit
import ITopKit
import SwiftUI

@main
struct ITopWrapApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var model = SessionModel()

    var body: some Scene {
        WindowGroup("iTop Wrap") {
            RootView()
                .environment(model)
                .task { await model.restore() }
        }
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
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
