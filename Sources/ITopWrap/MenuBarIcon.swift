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
