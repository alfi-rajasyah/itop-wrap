import Foundation
import ImageIO

/// An image to add to a log entry, checked before anything is sent: a PNG, JPEG or GIF (judged by
/// its bytes, not its name) of at most `maxBytes`.
///
/// It's uploaded as an `InlineImage` on the ticket, the way the iTop web UI stores a pasted image
/// (Fixtures/log-images-probe.json), so it shows inside the entry on the web and in the app.
public struct LogImage: Sendable, Equatable, Identifiable {
    public let id: UUID
    public let data: Data
    public let filename: String
    public let mimetype: String
    public let pixelWidth: Int?
    public let pixelHeight: Int?

    /// 4 MB. The upload is sent base64 in a form field, which makes it about 1.4 times bigger;
    /// this keeps it under PHP's default 8 MB `post_max_size`.
    public static let maxBytes = 4 * 1_048_576
    public static let maxPerEntry = 5

    public enum Failure: Error, Equatable {
        case unsupportedType(filename: String)
        case tooLarge(filename: String, maxMB: Int)
    }

    public init(data: Data, filename: String) throws {
        let name = filename.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let type = Self.sniff(data) else { throw Failure.unsupportedType(filename: name) }
        guard data.count <= Self.maxBytes else { throw Failure.tooLarge(filename: name, maxMB: Self.maxBytes / 1_048_576) }
        id = UUID()
        self.data = data
        mimetype = type.mimetype
        self.filename = name.isEmpty ? "image.\(type.ext)" : name
        let size = Self.pixelSize(data)
        pixelWidth = size?.width
        pixelHeight = size?.height
    }

    /// The type from the file's first bytes.
    static func sniff(_ data: Data) -> (mimetype: String, ext: String)? {
        let b = [UInt8](data.prefix(8))
        if b.starts(with: [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) { return ("image/png", "png") }
        if b.starts(with: [0xFF, 0xD8, 0xFF]) { return ("image/jpeg", "jpg") }
        if b.starts(with: Array("GIF87a".utf8)) || b.starts(with: Array("GIF89a".utf8)) { return ("image/gif", "gif") }
        return nil
    }

    static func pixelSize(_ data: Data) -> (width: Int, height: Int)? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int,
              w > 0, h > 0 else { return nil }
        return (w, h)
    }

    /// The size the entry shows the image at: its own, narrowed to `maxShownWidth` like a web paste.
    var shownSize: (width: Int, height: Int)? {
        guard let w = pixelWidth, let h = pixelHeight else { return nil }
        guard w > Self.maxShownWidth else { return (w, h) }
        return (Self.maxShownWidth, max(1, Int((Double(h) * Double(Self.maxShownWidth) / Double(w)).rounded())))
    }

    static let maxShownWidth = 800
}

extension TicketWrites {
    /// Uploads the images as InlineImages on the ticket, then appends one HTML entry with the text
    /// (escaped, line breaks kept) followed by the images. Without images, the text goes as before.
    /// Everything is checked before the first upload; if iTop refuses an image, no entry is added.
    public func addLogEntry(to ticket: Ticket, text: String, images: [LogImage], isPrivate: Bool) async throws {
        guard !images.isEmpty else { return try await addLogEntry(to: ticket, text: text, isPrivate: isPrivate) }
        guard images.count <= LogImage.maxPerEntry else { throw Failure.tooManyImages(max: LogImage.maxPerEntry) }
        guard client.canWrite(className: ticket.ticketClass.rawValue, key: ticket.key) else { throw ITopError.writesDisabled }

        var uploaded: [UploadedImage] = []
        for image in images {
            let (id, secret) = try await upload(image, to: ticket)
            uploaded.append(UploadedImage(id: id, secret: secret, image: image))
        }
        let message = Self.entryHTML(text: text, images: uploaded, base: client.server.baseURL)
        let field = isPrivate ? "private_log" : "public_log"
        try await update(ticket, fields: [field: .object(["add_item": .object(["message": .string(message), "format": .string("html")])])])
    }

    /// The new InlineImage's id and secret. iTop Wrap sends a secret of its own and uses iTop's
    /// when the answer has one.
    func upload(_ image: LogImage, to ticket: Ticket) async throws -> (id: String, secret: String) {
        let ownSecret = Self.newSecret()
        let response = try await client.send(.create(className: "InlineImage", fields: [
            "item_class": .string(ticket.ticketClass.rawValue),
            "item_id": .string(ticket.key),
            "item_org_id": .string(ticket.orgID),
            "secret": .string(ownSecret),
            "contents": .object([
                "data": .string(image.data.base64EncodedString()),
                "mimetype": .string(image.mimetype),
                "filename": .string(image.filename),
            ]),
        ], comment: Self.comment, outputFields: ["id", "secret"]), onTicket: Self.allowlistKey(for: ticket))
        try Self.check(response)
        guard let object = response.objects.first else { throw ITopError.unexpectedResponse }
        let id = object.fields["id"]?.stringValue ?? object.key
        guard Self.isID(id), id != "0" else { throw ITopError.unexpectedResponse }
        let secret = object.fields["secret"]?.stringValue.flatMap { $0.isEmpty ? nil : $0 } ?? ownSecret
        return (id, secret)
    }

    /// 32 hex characters, the shape iTop's own secrets have.
    static func newSecret() -> String {
        var rng = SystemRandomNumberGenerator()
        return (0..<16).map { _ in String(format: "%02x", UInt8.random(in: 0...255, using: &rng)) }.joined()
    }

    /// An image already stored in iTop as an InlineImage.
    public struct UploadedImage: Sendable, Equatable {
        public let id: String
        public let secret: String
        public let image: LogImage

        public init(id: String, secret: String, image: LogImage) {
            self.id = id
            self.secret = secret
            self.image = image
        }
    }

    /// The entry's HTML: the text, then each image the way a web paste shows it.
    public static func entryHTML(text: String, images: [UploadedImage], base: URL) -> String {
        html(text) + images.map { imageHTML(id: $0.id, secret: $0.secret, image: $0.image, base: base) }.joined()
    }

    /// The text as one HTML paragraph, escaped, with `<br>` for each line break. Empty for blank text.
    static func html(_ text: String) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        let lines = trimmed.replacingOccurrences(of: "\r\n", with: "\n").split(separator: "\n", omittingEmptySubsequences: false)
        return "<p>" + lines.map { escape(String($0)) }.joined(separator: "<br>") + "</p>"
    }

    /// The `<img>` the web UI writes for a pasted image (Fixtures/log-images-probe.json).
    static func imageHTML(id: String, secret: String, image: LogImage, base: URL) -> String {
        let src = "\(base.absoluteString)/pages/ajax.document.php?operation=download_inlineimage&id=\(id)&s=\(secret)"
        var attrs = [#"src="\#(escape(src))""#]
        if let size = image.shownSize {
            attrs.append(#"width="\#(size.width)" height="\#(size.height)" style="aspect-ratio:\#(size.width)/\#(size.height)""#)
        }
        attrs.append(#"data-img-id="\#(escape(id))" data-img-secret="\#(escape(secret))""#)
        return "<p><img \(attrs.joined(separator: " "))></p>"
    }

    static func escape(_ s: String) -> String {
        var out = ""
        out.reserveCapacity(s.count)
        for c in s {
            switch c {
            case "&": out += "&amp;"
            case "<": out += "&lt;"
            case ">": out += "&gt;"
            case "\"": out += "&quot;"
            case "'": out += "&#39;"
            default: out.append(c)
            }
        }
        return out
    }
}
