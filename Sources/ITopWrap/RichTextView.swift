import AppKit
import ITopKit
import SwiftUI

/// A description or log entry with clickable links and its images (PLAN.md Task 5c).
/// Pasted iTop images load through REST; images on other hosts never load on their own.
struct RichTextView: View {
    let html: String?
    let plain: String
    let base: URL
    let source: TicketSource
    /// "Open in iTop on the web" for when an image can't be loaded.
    let ticketURL: URL?

    var body: some View {
        let blocks = html.map { RichText.parse($0, base: base) } ?? []
        if blocks.isEmpty {
            Text(plain).fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
        } else {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                    switch block {
                    case .text(let runs):
                        text(runs)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                            .tint(Theme.text)
                    case .image(let image):
                        RichImageView(image: image, source: source, ticketURL: ticketURL)
                    }
                }
            }
        }
    }

    /// Text with links; small outside images (signature icons) become a grey photo symbol
    /// in the line. They're never loaded.
    private func text(_ runs: [RichRun]) -> Text {
        var result = Text(verbatim: "")
        var pending = AttributedString()
        func flushPending() {
            if !pending.characters.isEmpty { result = Text("\(result)\(Text(pending))"); pending = AttributedString() }
        }
        for run in runs {
            if run.icon != nil {
                flushPending()
                result = Text("\(result)\(Text(Image(systemName: "photo")).foregroundStyle(Theme.secondary))")
            } else if run.isOutsideImage, let url = run.link {
                // "🖼 host ↗": opens the image in the browser; it's never loaded in the app.
                flushPending()
                var label = AttributedString("\(run.text) ↗")
                label.link = url
                label.foregroundColor = Theme.secondary
                result = Text("\(result)\(Text(Image(systemName: "photo")).foregroundStyle(Theme.secondary)) \(Text(label))")
            } else {
                var piece = AttributedString(run.text)
                if let link = run.link {
                    piece.link = link
                    piece.underlineStyle = .single
                }
                pending += piece
            }
        }
        flushPending()
        return result
    }
}

struct RichImageView: View {
    let image: RichImage
    let source: TicketSource
    let ticketURL: URL?

    @State private var loaded: NSImage?
    @State private var failed = false
    @State private var fullSize = false

    var body: some View {
        Group {
            switch image {
            case .inline, .data:
                if let loaded {
                    Button { fullSize = true } label: {
                        Image(nsImage: loaded)
                            .resizable()
                            .scaledToFit()
                            .frame(maxWidth: min(loaded.size.width, 640), maxHeight: 480, alignment: .leading)
                            .clipShape(RoundedRectangle(cornerRadius: 4))
                    }
                    .buttonStyle(.plain)
                    .help("Open full size")
                    .accessibilityLabel("Image, open full size")
                } else if failed {
                    HStack(spacing: 8) {
                        Image(systemName: "exclamationmark.triangle").foregroundStyle(Theme.secondary)
                        Text("Couldn't load this image.").foregroundStyle(Theme.secondary)
                        if let ticketURL { Link("Open in iTop on the web", destination: ticketURL).foregroundStyle(Theme.text) }
                    }
                    .font(.system(size: 12))
                } else {
                    HStack(spacing: 8) { ProgressView().controlSize(.small); Text("Loading image").foregroundStyle(Theme.secondary) }
                        .font(.system(size: 12))
                }
            }
        }
        .task(id: image) { await load() }
        .sheet(isPresented: $fullSize) {
            if let loaded { ImageViewer(image: loaded) }
        }
    }

    private func load() async {
        switch image {
        case .data(let data):
            loaded = NSImage(data: data)
            failed = loaded == nil
        case .inline(let id):
            do {
                let result = try await source.image(id)
                loaded = NSImage(data: result.data)
                failed = loaded == nil
            } catch {
                failed = true
            }
        }
    }
}

/// The ticket's attachments: names, and a download through REST into a Save panel.
struct AttachmentsView: View {
    let ticket: Ticket
    let source: TicketSource

    @State private var items: [AttachmentRef]?
    @State private var error: String?
    @State private var saving: AttachmentRef.ID?
    @State private var saved: String?

    var body: some View {
        Group {
            if let items, !items.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Attachments").font(.system(size: 12, weight: .semibold))
                    FlowLayout(spacing: 6, lineSpacing: 6) {
                        ForEach(items) { a in
                            Button { Task { await save(a) } } label: {
                                HStack(spacing: 5) {
                                    if saving == a.id { ProgressView().controlSize(.mini) } else { Image(systemName: "paperclip").foregroundStyle(Theme.secondary) }
                                    Text(a.name).lineLimit(1)
                                }
                                .font(.system(size: 12))
                                .padding(.horizontal, 9).frame(height: 26)
                                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.separator))
                                .contentShape(RoundedRectangle(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                            .disabled(saving != nil)
                            .help("Download \(a.name)")
                        }
                    }
                    if let saved { Text(saved).font(.system(size: 12)).foregroundStyle(Theme.secondary) }
                    if let error { Text(error).font(.system(size: 12)).foregroundStyle(Theme.warning) }
                }
            } else if let error, items == nil {
                Text("Couldn't list attachments. \(error)").font(.system(size: 12)).foregroundStyle(Theme.secondary)
            } else {
                // An empty Group has no lifecycle, so its .task would never run: keep a zero-size view.
                Color.clear.frame(height: 0)
            }
        }
        .task(id: ticket.id) {
            do { items = try await source.attachments(ticket) } catch { self.error = error.localizedDescription }
        }
    }

    @MainActor
    private func save(_ a: AttachmentRef) async {
        saving = a.id
        error = nil
        saved = nil
        defer { saving = nil }
        do {
            let file = try await source.download(a)
            let panel = NSSavePanel()
            panel.nameFieldStringValue = file.filename
            panel.canCreateDirectories = true
            guard panel.runModal() == .OK, let url = panel.url else { return }
            try file.data.write(to: url)
            saved = "Saved \(file.filename) (\(ByteCountFormatter.string(fromByteCount: Int64(file.data.count), countStyle: .file)))."
        } catch {
            self.error = "Couldn't download \(a.name). \(error.localizedDescription)"
        }
    }
}

/// Full-size image: the whole image fits the window by default; "Actual size" scrolls at 1:1.
struct ImageViewer: View {
    let image: NSImage
    @State private var actualSize = false
    @Environment(\.dismiss) private var dismiss

    /// Sized to the image, within a comfortable window (the user found wide screenshots cropped).
    private var frame: CGSize {
        let w = max(image.size.width, 1), h = max(image.size.height, 1)
        let scale = min(1, 1100 / w, 720 / h)
        return CGSize(width: max(480, w * scale), height: max(300, h * scale))
    }

    var body: some View {
        VStack(spacing: 12) {
            Group {
                if actualSize {
                    ScrollView([.horizontal, .vertical]) { Image(nsImage: image) }
                } else {
                    Image(nsImage: image).resizable().scaledToFit()
                }
            }
            .frame(width: frame.width, height: frame.height)
            HStack {
                Toggle("Actual size", isOn: $actualSize).toggleStyle(.checkbox)
                    .help("Show the image at 100%, scrolling if it's bigger than the window")
                Text("\(Int(image.size.width)) × \(Int(image.size.height))").font(.system(size: 11)).foregroundStyle(Theme.secondary)
                Spacer()
                Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(16)
    }
}
