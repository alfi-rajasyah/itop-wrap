// Draws the app icon: the Stacked stubs mark (MenuBarIcon.draw) in white on a rounded tile in the
// app's dark teal, at every size macOS wants, then builds Resources/AppIcon.icns with iconutil.
// Usage: scripts/make-app-icon.sh   (or run this with an output folder and an optional preview path)
import AppKit

let teal = NSColor(srgbRed: 0x0F / 255, green: 0x6E / 255, blue: 0x56 / 255, alpha: 1)

/// The mark alone, white on transparent, at `side` pixels.
func mark(_ side: CGFloat) -> NSImage {
    let glyph = NSImage(size: NSSize(width: side, height: side), flipped: false) { _ in
        NSGraphicsContext.current?.cgContext.scaleBy(x: side / 18, y: side / 18)
        MenuBarIcon.draw()
        return true
    }
    return NSImage(size: glyph.size, flipped: false) { rect in
        glyph.draw(in: rect)
        NSColor.white.set()
        rect.fill(using: .sourceAtop)
        return true
    }
}

/// macOS icon grid: an 824 pt tile centred on a 1024 pt canvas, corner radius about 185.
func icon(_ pixels: Int) -> NSBitmapImageRep {
    let s = CGFloat(pixels)
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4,
                               hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let tile = NSRect(x: s * 100 / 1024, y: s * 100 / 1024, width: s * 824 / 1024, height: s * 824 / 1024)
    let shadow = NSShadow()
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.28)
    shadow.shadowOffset = NSSize(width: 0, height: -s * 8 / 1024)
    shadow.shadowBlurRadius = s * 18 / 1024
    NSGraphicsContext.saveGraphicsState()
    shadow.set()
    teal.setFill()
    NSBezierPath(roundedRect: tile, xRadius: s * 185 / 1024, yRadius: s * 185 / 1024).fill()
    NSGraphicsContext.restoreGraphicsState()
    let side = tile.width * 0.62
    mark(side).draw(in: NSRect(x: tile.midX - side / 2, y: tile.midY - side / 2, width: side, height: side))
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

let args = CommandLine.arguments
let iconset = URL(fileURLWithPath: args[1])
try? FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for (points, scale) in [(16, 1), (16, 2), (32, 1), (32, 2), (128, 1), (128, 2), (256, 1), (256, 2), (512, 1), (512, 2)] {
    let name = "icon_\(points)x\(points)" + (scale == 2 ? "@2x" : "") + ".png"
    try icon(points * scale).representation(using: .png, properties: [:])!.write(to: iconset.appendingPathComponent(name))
}
if args.count > 2 {
    try icon(512).representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
}
