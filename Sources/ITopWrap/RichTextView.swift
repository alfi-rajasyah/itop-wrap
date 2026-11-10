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
                        Text(attributed(runs))
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

    private func attributed(_ runs: [RichRun]) -> AttributedString {
        var out = AttributedString()
        for run in runs {
            var piece = AttributedString(run.text)
            if let link = run.link {
                piece.link = link
                piece.underlineStyle = .single
            }
            out += piece
        }
        return out
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
            case .external(let url):
                HStack(spacing: 8) {
                    Image(systemName: "photo").foregroundStyle(Theme.secondary)
                    Text("Image from \(url.host() ?? "another site")").foregroundStyle(Theme.secondary)
                    Link("Open in browser", destination: url).foregroundStyle(Theme.text)
                }
                .font(.system(size: 12))
                .padding(10)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.separator))
                .help("Not loaded automatically: it's on another site")
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
            if let loaded {
                VStack(alignment: .trailing, spacing: 10) {
                    ScrollView([.horizontal, .vertical]) { Image(nsImage: loaded) }
                    Button("Close") { fullSize = false }.keyboardShortcut(.cancelAction)
                }
                .padding(16)
                .frame(minWidth: 480, idealWidth: min(loaded.size.width + 40, 1200), minHeight: 360, idealHeight: min(loaded.size.height + 80, 900))
            }
        }
    }

    private func load() async {
        switch image {
        case .external: return
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
