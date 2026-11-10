import Foundation

/// A piece of text, with a link when it came from an `<a>`. Images from other hosts are never
/// loaded and stay in the line: a small one (a signature icon) as an icon, a larger one as a
/// chip labelled with its host that opens it in the browser.
public struct RichRun: Sendable, Equatable {
    public let text: String
    public let link: URL?
    public let icon: URL?
    /// True for the "🖼 host ↗" chip of a larger outside image; `link` opens it.
    public let isOutsideImage: Bool

    public init(text: String, link: URL? = nil) {
        self.text = text
        self.link = link
        self.icon = nil
        self.isOutsideImage = false
    }

    public init(icon: URL) {
        self.text = ""
        self.link = nil
        self.icon = icon
        self.isOutsideImage = false
    }

    public init(outsideImage url: URL) {
        self.text = url.host() ?? "another site"
        self.link = url
        self.icon = nil
        self.isOutsideImage = true
    }
}

public enum RichImage: Sendable, Equatable {
    /// An image pasted into iTop: loaded by id through REST, never through the web link.
    case inline(id: String)
    /// A `data:` image: already in the HTML, no network needed.
    case data(Data)
}

public enum RichBlock: Sendable, Equatable {
    case text([RichRun])
    case image(RichImage)
}

/// HTML from iTop as text blocks with links, and images in between (PLAN.md Task 5c).
/// It follows `HTMLText.plain`'s rules, so the text reads the same as the plain version.
public enum RichText {
    public static func parse(_ html: String, base: URL) -> [RichBlock] {
        var source = html
        source.replace(/(?is)<(script|style)\b.*?<\/\1\s*>/, with: "")

        var blocks: [RichBlock] = []
        var buffer = ""
        var links: [URL] = []
        var icons: [(url: URL, small: Bool)] = []
        var inLink = false

        func flush() {
            let text = HTMLText.normalize(buffer)
            buffer = ""
            let runs = runs(from: text, links: links, icons: icons)
            if !runs.isEmpty { blocks.append(.text(runs)) }
        }

        for token in source.matches(of: /<[^>]*>|[^<]+/) {
            let piece = String(token.output)
            guard piece.hasPrefix("<") else {
                buffer += HTMLText.decodeEntities(piece)
                continue
            }
            let tag = piece.lowercased()
            let name = tag.firstMatch(of: /^<\s*\/?\s*([a-z0-9]+)/).map { String($0.output.1) } ?? ""
            let closing = tag.hasPrefix("</")
            switch name {
            case "img":
                if let url = outsideImage(from: piece, base: base) {
                    // From another host: stays in the line, never loaded (icon, or a host chip).
                    if inLink { buffer += end; inLink = false }
                    icons.append((url, isSmall(piece)))
                    buffer += iconMark + String(icons.count - 1) + mid
                } else if let image = image(from: piece, base: base) {
                    if inLink { buffer += end; inLink = false }
                    flush()
                    blocks.append(.image(image))
                }
            case "br": buffer += "\n"
            case "li": if !closing { buffer += "\n• " }
            case "p", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "figure", "ul", "ol", "table", "pre": buffer += "\n\n"
            case "div", "tr", "section", "article", "header", "footer": buffer += "\n"
            case "td", "th": buffer += " "
            case "a":
                if closing {
                    if inLink { buffer += end; inLink = false }
                } else if let url = link(from: piece, base: base) {
                    if inLink { buffer += end }
                    links.append(url)
                    buffer += start + String(links.count - 1) + mid
                    inLink = true
                }
            default: break
            }
        }
        if inLink { buffer += end }
        flush()
        return blocks
    }

    // Link boundaries and icons survive whitespace clean-up as private-use characters,
    // then a scanner turns them into runs.
    private static let start = "\u{E000}", mid = "\u{E001}", end = "\u{E002}", iconMark = "\u{E003}"

    private static func runs(from text: String, links: [URL], icons: [(url: URL, small: Bool)]) -> [RichRun] {
        var out: [RichRun] = []
        var plain = "", label = ""
        var link: URL?
        var reading: (icon: Bool, digits: String)?
        func emitPlain() { if !plain.isEmpty { out.append(RichRun(text: plain)); plain = "" } }

        for ch in text {
            if var r = reading {
                if String(ch) == mid {
                    let i = Int(r.digits) ?? -1
                    if r.icon {
                        if icons.indices.contains(i) {
                            emitPlain()
                            out.append(icons[i].small ? RichRun(icon: icons[i].url) : RichRun(outsideImage: icons[i].url))
                        }
                    } else if links.indices.contains(i) {
                        emitPlain(); link = links[i]; label = ""
                    }
                    reading = nil
                } else if ch.isNumber {
                    r.digits.append(ch); reading = r
                } else {
                    reading = nil
                }
                continue
            }
            switch String(ch) {
            case start: reading = (false, "")
            case iconMark: reading = (true, "")
            case end:
                if let l = link, !label.isEmpty { out.append(RichRun(text: label, link: l)) }
                link = nil; label = ""
            case mid: break
            default:
                if link != nil { label.append(ch) } else { plain.append(ch) }
            }
        }
        if let l = link, !label.isEmpty { out.append(RichRun(text: label, link: l)) } else { plain += label }
        emitPlain()
        return out
    }

    /// Signature-icon size: the width and height it declares (attributes or inline style) are at most 40 px.
    static func isSmall(_ tag: String) -> Bool {
        func px(_ name: String) -> Int? {
            if let v = attribute(name, in: tag), let n = Int(v.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: "px", with: "")) { return n }
            if let style = attribute("style", in: tag),
               let regex = try? Regex("(?i)(?:^|;)\\s*\(name)\\s*:\\s*(\\d+)\\s*px"),
               let m = style.firstMatch(of: regex), let d = m.output[1].substring { return Int(d) }
            return nil
        }
        let sizes = [px("width"), px("height")].compactMap { $0 }
        return !sizes.isEmpty && sizes.allSatisfy { $0 <= 40 }
    }

    private static func attribute(_ name: String, in tag: String) -> String? {
        guard let regex = try? Regex("(?i)\\b\(name)\\s*=\\s*(\"([^\"]*)\"|'([^']*)')"),
              let m = tag.firstMatch(of: regex) else { return nil }
        let value = m.output[2].substring ?? m.output[3].substring
        return value.map { HTMLText.decodeEntities(String($0)) }
    }

    private static func resolve(_ text: String, base: URL) -> URL? {
        let folder = base.absoluteString.hasSuffix("/") ? base : URL(string: base.absoluteString + "/")!
        return URL(string: text.trimmingCharacters(in: .whitespaces), relativeTo: folder)?.absoluteURL
    }

    /// Only web and email links: `javascript:`, `file:` and the like keep their text but lose the link.
    static func link(from tag: String, base: URL) -> URL? {
        guard let href = attribute("href", in: tag), !href.isEmpty, let url = resolve(href, base: base),
              let scheme = url.scheme?.lowercased(), ["http", "https", "mailto"].contains(scheme) else { return nil }
        return url
    }

    static func image(from tag: String, base: URL) -> RichImage? {
        guard let src = attribute("src", in: tag) else { return nil }
        if src.lowercased().hasPrefix("data:image/"), let comma = src.firstIndex(of: ","), src[..<comma].hasSuffix(";base64") {
            return Data(base64Encoded: String(src[src.index(after: comma)...])).map(RichImage.data)
        }
        guard let url = resolve(src, base: base) else { return nil }
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        if query.contains(where: { $0.name == "operation" && $0.value == "download_inlineimage" }) {
            let id = attribute("data-img-id", in: tag) ?? query.first { $0.name == "id" }?.value
            return id.flatMap(Int.init).map { .inline(id: String($0)) }
        }
        return nil
    }

    /// An http(s) image that isn't a pasted iTop image: from another host, so never loaded.
    static func outsideImage(from tag: String, base: URL) -> URL? {
        guard let src = attribute("src", in: tag), !src.lowercased().hasPrefix("data:"),
              let url = resolve(src, base: base), let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return nil }
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        return query.contains { $0.name == "operation" && $0.value == "download_inlineimage" } ? nil : url
    }
}

public struct AttachmentRef: Sendable, Equatable, Identifiable, Hashable {
    public let id: String
    public let name: String

    public init(id: String, name: String) {
        self.id = id
        self.name = name
    }
}

public struct AttachmentFile: Sendable, Equatable {
    public let data: Data
    public let filename: String
    public let mimetype: String

    public init(data: Data, filename: String, mimetype: String) {
        self.data = data
        self.filename = filename
        self.mimetype = mimetype
    }
}

/// Pasted images and attachments, read through REST with the app's own login. Your iTop
/// refuses the web image links with HTTP Basic auth (403 in the Task 5c probe), so they're
/// never used. Images are kept in memory (the last 60).
public actor InlineImageLoader {
    public enum Failure: Error, Equatable { case badID, notFound, undecodable }

    public struct Image: Sendable, Equatable {
        public let data: Data
        public let mimetype: String

        public init(data: Data, mimetype: String) {
            self.data = data
            self.mimetype = mimetype
        }
    }

    let client: ITopClient
    private var cache: [String: Image] = [:]
    private var order: [String] = []
    private static let cacheSize = 60

    public init(client: ITopClient) {
        self.client = client
    }

    public func image(id: String) async throws -> Image {
        if let hit = cache[id] { return hit }
        guard let n = Int(id) else { throw Failure.badID }
        let r = try await client.send(.get(className: "InlineImage", key: "SELECT InlineImage WHERE id = \(n)", outputFields: ["contents"]))
        guard let contents = r.objects.first?.fields["contents"] else { throw Failure.notFound }
        guard let b64 = contents["data"]?.stringValue, let data = Data(base64Encoded: b64) else { throw Failure.undecodable }
        let image = Image(data: data, mimetype: contents["mimetype"]?.stringValue ?? "")
        cache[id] = image
        order.append(id)
        if order.count > Self.cacheSize { cache[order.removeFirst()] = nil }
        return image
    }

    /// The ticket's attachments by name. The file contents are only fetched on download.
    public func attachments(of ticket: Ticket) async throws -> [AttachmentRef] {
        guard let key = Int(ticket.key) else { return [] }
        let r = try await client.send(.get(className: "Attachment",
                                           key: "SELECT Attachment WHERE item_class = \(OQL.quote(ticket.ticketClass.rawValue)) AND item_id = \(key)",
                                           outputFields: ["friendlyname"]))
        return r.objects.map { AttachmentRef(id: $0.key, name: $0.fields["friendlyname"]?.stringValue ?? "Attachment \($0.key)") }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    public func download(_ attachment: AttachmentRef) async throws -> AttachmentFile {
        guard let n = Int(attachment.id) else { throw Failure.badID }
        let r = try await client.send(.get(className: "Attachment", key: "SELECT Attachment WHERE id = \(n)", outputFields: ["contents"]))
        guard let contents = r.objects.first?.fields["contents"] else { throw Failure.notFound }
        guard let b64 = contents["data"]?.stringValue, let data = Data(base64Encoded: b64) else { throw Failure.undecodable }
        return AttachmentFile(data: data, filename: contents["filename"]?.stringValue ?? attachment.name,
                              mimetype: contents["mimetype"]?.stringValue ?? "")
    }
}
