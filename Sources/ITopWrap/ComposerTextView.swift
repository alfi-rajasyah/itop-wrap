import AppKit
import ITopKit
import SwiftUI
import UniformTypeIdentifiers

/// Turns pasted, dropped or chosen files into `LogImage`s. PNG, JPEG and GIF go as they are;
/// other images macOS can read (a TIFF screenshot on the clipboard, a HEIC photo) are converted:
/// TIFF to PNG, the rest to JPEG, at most `maxConvertedSide` px on the longest side.
enum ImageIntake {
    struct Item: Sendable {
        let data: Data
        let filename: String
        /// A file too big to read at all.
        var oversize = false
    }

    static let maxConvertedSide: CGFloat = 2560
    /// Files bigger than this aren't read at all.
    static let maxFileBytes = 40 * 1_048_576

    static func logImage(_ item: Item) throws -> LogImage {
        if item.oversize { throw LogImage.Failure.tooLarge(filename: item.filename, maxMB: LogImage.maxBytes / 1_048_576) }
        do {
            return try LogImage(data: item.data, filename: item.filename)
        } catch LogImage.Failure.unsupportedType {
            guard let converted = convert(item) else { throw LogImage.Failure.unsupportedType(filename: item.filename) }
            return try LogImage(data: converted.data, filename: converted.filename)
        }
    }

    static func convert(_ item: Item) -> Item? {
        guard let source = NSBitmapImageRep(data: item.data), source.pixelsWide > 0, source.pixelsHigh > 0 else { return nil }
        let isTIFF = item.data.prefix(4) == Data([0x49, 0x49, 0x2A, 0x00]) || item.data.prefix(4) == Data([0x4D, 0x4D, 0x00, 0x2A])
        var rep = source
        let longest = CGFloat(max(source.pixelsWide, source.pixelsHigh))
        if longest > maxConvertedSide {
            let scale = maxConvertedSide / longest
            let w = Int((CGFloat(source.pixelsWide) * scale).rounded()), h = Int((CGFloat(source.pixelsHigh) * scale).rounded())
            guard let scaled = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4,
                                                hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
            else { return nil }
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: scaled)
            source.draw(in: NSRect(x: 0, y: 0, width: w, height: h))
            NSGraphicsContext.restoreGraphicsState()
            rep = scaled
        }
        let base = (item.filename as NSString).deletingPathExtension
        let name = base.isEmpty ? "Pasted image" : base
        if isTIFF, let png = rep.representation(using: .png, properties: [:]) { return Item(data: png, filename: name + ".png") }
        guard let jpeg = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.85]) else { return nil }
        return Item(data: jpeg, filename: name + ".jpg")
    }

    /// Image files first (Finder copies and drags); otherwise, only when there's no text to paste,
    /// image data (a screenshot copied to the clipboard). Nil when there are no images.
    static func items(from pasteboard: NSPasteboard, preferText: Bool) -> [Item]? {
        let urls = (pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        let images = urls.filter { (UTType(filenameExtension: $0.pathExtension)?.conforms(to: .image)) ?? false }
        if !images.isEmpty {
            return images.compactMap { url in
                let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
                guard size <= maxFileBytes, let data = try? Data(contentsOf: url) else {
                    return Item(data: Data(), filename: url.lastPathComponent, oversize: size > maxFileBytes)
                }
                return Item(data: data, filename: url.lastPathComponent)
            }
        }
        if !urls.isEmpty { return nil }
        if preferText, pasteboard.string(forType: .string) != nil { return nil }
        for type in [NSPasteboard.PasteboardType.png, .tiff] {
            if let data = pasteboard.data(forType: type) {
                return [Item(data: data, filename: type == .png ? "Pasted image.png" : "Pasted image.tiff")]
            }
        }
        return nil
    }

    static func hasImages(_ pasteboard: NSPasteboard) -> Bool {
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
           urls.contains(where: { (UTType(filenameExtension: $0.pathExtension)?.conforms(to: .image)) ?? false }) { return true }
        return pasteboard.availableType(from: [.png, .tiff]) != nil
    }
}

/// The composer's text field: a plain NSTextView that also takes images by ⌘V (or Edit ▸ Paste)
/// and by drag and drop. A plain SwiftUI TextEditor ignores pasted images and turns a dropped file
/// into its path.
struct ComposerTextView: NSViewRepresentable {
    @Binding var text: String
    var isEditable: Bool
    var accessibilityLabel: String
    var onImages: ([ImageIntake.Item]) -> Void
    var onFocus: (Bool) -> Void
    var onDropTarget: (Bool) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        let view = ImageTextView(frame: .zero)
        view.isRichText = false
        view.importsGraphics = false
        view.allowsUndo = true
        view.drawsBackground = false
        view.font = .systemFont(ofSize: 13)
        view.textColor = .labelColor
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.isAutomaticDashSubstitutionEnabled = false
        view.textContainerInset = NSSize(width: 0, height: 1)
        view.isVerticallyResizable = true
        view.isHorizontallyResizable = false
        view.autoresizingMask = [.width]
        view.textContainer?.widthTracksTextView = true
        view.delegate = context.coordinator
        view.registerForDraggedTypes([.fileURL, .png, .tiff])
        scroll.documentView = view
        context.coordinator.view = view
        update(view, coordinator: context.coordinator)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let view = scroll.documentView as? ImageTextView else { return }
        if view.string != text { view.string = text }
        update(view, coordinator: context.coordinator)
    }

    private func update(_ view: ImageTextView, coordinator: Coordinator) {
        view.isEditable = isEditable
        view.isSelectable = true
        view.setAccessibilityLabel(accessibilityLabel)
        view.onImages = { coordinator.parent.onImages($0) }
        view.onFocus = { coordinator.parent.onFocus($0) }
        view.onDropTarget = { coordinator.parent.onDropTarget($0) }
    }

    @MainActor final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: ComposerTextView
        weak var view: ImageTextView?

        init(_ parent: ComposerTextView) { self.parent = parent }

        func textDidChange(_ notification: Notification) {
            guard let view = notification.object as? NSTextView else { return }
            parent.text = view.string
        }
    }
}

final class ImageTextView: NSTextView {
    var onImages: (([ImageIntake.Item]) -> Void)?
    var onFocus: ((Bool) -> Void)?
    var onDropTarget: ((Bool) -> Void)?

    override func paste(_ sender: Any?) {
        if isEditable, let items = ImageIntake.items(from: .general, preferText: true) {
            onImages?(items)
            return
        }
        super.paste(sender)
    }

    override func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(paste(_:)), isEditable, ImageIntake.hasImages(.general) { return true }
        return super.validateMenuItem(item)
    }

    /// Tab moves on to the composer's buttons, as in a text field; an entry doesn't need tab characters.
    override func insertTab(_ sender: Any?) {
        window?.selectNextKeyView(sender)
    }

    override func insertBacktab(_ sender: Any?) {
        window?.selectPreviousKeyView(sender)
    }

    override func becomeFirstResponder() -> Bool {
        let ok = super.becomeFirstResponder()
        if ok { onFocus?(true) }
        return ok
    }

    override func resignFirstResponder() -> Bool {
        let ok = super.resignFirstResponder()
        if ok { onFocus?(false) }
        return ok
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard isEditable, ImageIntake.hasImages(sender.draggingPasteboard) else { return super.draggingEntered(sender) }
        onDropTarget?(true)
        return .copy
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard isEditable, ImageIntake.hasImages(sender.draggingPasteboard) else { return super.draggingUpdated(sender) }
        return .copy
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        onDropTarget?(false)
        super.draggingExited(sender)
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        onDropTarget?(false)
        if isEditable, let items = ImageIntake.items(from: sender.draggingPasteboard, preferText: false) {
            onImages?(items)
            return true
        }
        return super.performDragOperation(sender)
    }
}
