import AppKit
import SwiftUI

/// Colour tokens from DESIGN.md, each with its light and dark value.
enum Theme {
    static let text = dynamic(light: 0x1D1D1F, dark: 0xF2F2F7)
    static let secondary = dynamic(light: 0x5E5E63, dark: 0xA1A1A6)
    static let separator = dynamic(light: 0xD8D8DE, dark: 0x3A3A3C)
    static let warning = dynamic(light: 0xC4291C, dark: 0xFF6B5E)
    /// The neutral primary button: near-black in light mode, near-white in dark.
    static let primaryFill = dynamic(light: 0x1D1D1F, dark: 0xF2F2F7)
    static let onPrimary = dynamic(light: 0xFFFFFF, dark: 0x1D1D1F)
    static let chip = dynamic(light: 0xEEEEF1, dark: 0x333336)
    /// Teal: only ever "this ticket needs you" (DESIGN.md).
    static let accent = dynamic(light: 0x0F6E56, dark: 0x5DCAA5)
    static let accentTint = dynamic(light: 0xE1F5EE, dark: 0x085041)
    static let accentText = dynamic(light: 0x0F6E56, dark: 0x9FE1CB)

    private static func dynamic(light: UInt32, dark: UInt32) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let hex = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                           blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
        })
    }
}

/// The one filled button style: neutral, never teal (teal only means "needs you").
struct PrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .frame(maxWidth: .infinity, minHeight: 36)
            .foregroundStyle(Theme.onPrimary)
            .background(Theme.primaryFill.opacity(configuration.isPressed ? 0.85 : (isEnabled ? 1 : 0.75)),
                        in: RoundedRectangle(cornerRadius: 7))
            .contentShape(RoundedRectangle(cornerRadius: 7))
    }
}

extension Binding where Value: Equatable {
    /// A List selection that's applied after the click has been handled.
    ///
    /// Selecting a row here swaps out another column straight away (the sidebar replaces the
    /// middle list; a row rebuilds the detail pane). Done inside AppKit's selection callback,
    /// that makes it log "Application performed a reentrant operation in its NSTableView
    /// delegate", which it says will become an assert. Deferring one run-loop turn avoids it.
    @MainActor var deferred: Binding<Value> {
        let base = self
        return Binding(get: { base.wrappedValue }, set: { new in
            DispatchQueue.main.async { if base.wrappedValue != new { base.wrappedValue = new } }
        })
    }
}
