import AppKit

/// The Stacked stubs mark (DESIGN.md: Menu bar item): one ticket stub in front, with the
/// notches of a ticket on its sides, and two stubs peeking out behind it. Drawn in code as a
/// template image, so macOS tints it like every other menu bar icon.
enum MenuBarIcon {
    static let image: NSImage = {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: false) { _ in
            draw()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "iTop Wrap"
        return image
    }()

    private static let gap: CGFloat = 4

    private static func text(_ title: String) -> NSAttributedString {
        let font = NSFont.monospacedDigitSystemFont(ofSize: NSFont.menuBarFont(ofSize: 0).pointSize, weight: .regular)
        return NSAttributedString(string: title, attributes: [.font: font, .foregroundColor: NSColor.black])
    }

    /// The room the title takes right of the icon, gap included; 0 with no title.
    static func textWidth(_ title: String) -> CGFloat {
        title.isEmpty ? 0 : gap + ceil(text(title).size().width)
    }

    /// The icon, then the title, drawn as one template image. A MenuBarExtra label drops font
    /// modifiers, so a Text title used proportional digits and shifted every second; drawn here
    /// with fixed-width digits it stays put. `visible` (0 up to `textWidth`) shows only the part
    /// of the title nearest the icon, so the time can slide out of the icon and back in.
    static func label(_ title: String, visible: CGFloat? = nil) -> NSImage {
        let full = textWidth(title)
        let shown = min(max(visible ?? full, 0), full)
        guard shown > 0 else { return image }
        let text = text(title)
        let textSize = text.size()
        let height = max(image.size.height, ceil(textSize.height))
        let size = NSSize(width: image.size.width + shown, height: height)
        let label = NSImage(size: size, flipped: false) { _ in
            image.draw(in: NSRect(x: 0, y: (height - image.size.height) / 2, width: image.size.width, height: image.size.height))
            // The text's start sits against the icon; what doesn't fit yet is past the right edge.
            text.draw(at: NSPoint(x: image.size.width + gap, y: (height - textSize.height) / 2))
            return true
        }
        label.isTemplate = true
        return label
    }

    /// A stub: a rounded rectangle with a half-circle notch on each short side.
    static func stubRect(_ rect: NSRect) -> NSBezierPath {
        NSBezierPath(roundedRect: rect, xRadius: 1.4, yRadius: 1.4)
    }

    static func notches(_ rect: NSRect, radius: CGFloat) -> [NSBezierPath] {
        [rect.minX, rect.maxX].map { x in
            NSBezierPath(ovalIn: NSRect(x: x - radius, y: rect.midY - radius, width: radius * 2, height: radius * 2))
        }
    }

    private static func clear(_ path: NSBezierPath) {
        NSGraphicsContext.current?.compositingOperation = .clear
        path.fill()
        NSGraphicsContext.current?.compositingOperation = .sourceOver
    }

    /// Back to front: two outlined stubs, each nudged up and right, then the filled front stub.
    /// Each stub clears what's under it first, so the stack reads as separate tickets.
    static func draw() {
        NSColor.black.set()
        let size = NSSize(width: 12, height: 7.5)
        let frames = [NSRect(origin: NSPoint(x: 5, y: 9.5), size: size),
                      NSRect(origin: NSPoint(x: 3, y: 5.75), size: size),
                      NSRect(origin: NSPoint(x: 1, y: 2), size: size)]
        for (i, frame) in frames.enumerated() {
            let shape = stubRect(frame)
            clear(stubRect(frame.insetBy(dx: -0.8, dy: -0.8)))
            if i < frames.count - 1 {
                let outline = stubRect(frame.insetBy(dx: 0.6, dy: 0.6))
                outline.lineWidth = 1.2
                outline.stroke()
            } else {
                shape.fill()
            }
            notches(frame, radius: 1.7).forEach(clear)
            if i < frames.count - 1 {
                // Keep the notch outline on the back stubs.
                for n in notches(frame, radius: 1.7) {
                    NSGraphicsContext.saveGraphicsState()
                    shape.addClip()
                    n.lineWidth = 1.2
                    n.stroke()
                    NSGraphicsContext.restoreGraphicsState()
                }
            }
        }
    }
}
