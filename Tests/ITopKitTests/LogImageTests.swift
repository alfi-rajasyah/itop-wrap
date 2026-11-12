import Foundation
import Testing
@testable import ITopKit

// Shapes from Fixtures/log-images-probe.json: a web-pasted log image is an InlineImage tied to the
// ticket (item_class, item_id, item_org_id) with a 32-hex secret, and the entry's HTML has
// <img src="…/pages/ajax.document.php?operation=download_inlineimage&id=…&s=…" data-img-id data-img-secret>.

private let ticket = Ticket(key: "7", ticketClass: .userRequest, ref: "R-000007", title: "t", status: "assigned", priority: 3,
                            callerName: "", orgName: "", teamID: "0", teamName: "", agentID: "2", agentName: "",
                            startDate: nil, lastUpdate: nil, orgID: "3")

/// A 1×1 PNG.
private let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==")!

private func client(_ stub: StubServer, writes: Bool = true, allow: Set<String>? = nil) throws -> ITopClient {
    ITopClient(server: try ServerAddress("itop.example.com/itop"), credentials: Credentials(user: "a", password: "p"),
               session: stub.session, allowsWrites: writes, writeAllowlist: allow)
}

private let created = #"{"code":0,"message":"","objects":{"InlineImage::55":{"class":"InlineImage","key":"55","code":0,"message":"created","fields":{"id":"55","secret":"0123456789abcdef0123456789abcdef"}}}}"#
private let updated = #"{"code":0,"message":"","objects":{"UserRequest::7":{"class":"UserRequest","key":"7","code":0,"message":"updated","fields":{"id":"7"}}}}"#

/// Answers creates with `created` and updates with `updated`.
private func server(create: String = created) -> StubServer {
    StubServer { request in
        .json(request.jsonData["operation"] as? String == "core/create" ? create : updated)
    }
}

private func addItem(_ r: RecordedRequest, _ field: String = "public_log") -> [String: Any]? {
    ((r.jsonData["fields"] as? [String: Any])?[field] as? [String: Any])?["add_item"] as? [String: Any]
}

@Suite("Images in a log entry")
struct LogImageTests {
    @Test("The image is uploaded as an InlineImage on the ticket, then the entry is added as HTML that shows it")
    func uploadThenEntry() async throws {
        let stub = server()
        let image = try LogImage(data: png, filename: "screenshot.png")
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "See the error:", images: [image], isPrivate: false)

        #expect(stub.requests.count == 2)
        let create = try #require(stub.requests.first).jsonData
        #expect(create["operation"] as? String == "core/create")
        #expect(create["class"] as? String == "InlineImage")
        #expect(create["comment"] as? String == TicketWrites.comment)
        #expect((create["output_fields"] as? String)?.split(separator: ",").contains("secret") == true)
        let fields = try #require(create["fields"] as? [String: Any])
        #expect(fields["item_class"] as? String == "UserRequest")
        #expect(fields["item_id"] as? String == "7")
        #expect(fields["item_org_id"] as? String == "3")
        let contents = try #require(fields["contents"] as? [String: Any])
        #expect(contents["mimetype"] as? String == "image/png")
        #expect(contents["filename"] as? String == "screenshot.png")
        #expect(contents["data"] as? String == png.base64EncodedString())

        let update = stub.requests[1]
        #expect(update.jsonData["operation"] as? String == "core/update")
        let add = try #require(addItem(update))
        #expect(add["format"] as? String == "html")
        let html = try #require(add["message"] as? String)
        #expect(html.hasPrefix("<p>See the error:</p>"))
        #expect(html.contains(#"src="https://itop.example.com/itop/pages/ajax.document.php?operation=download_inlineimage&amp;id=55&amp;s=0123456789abcdef0123456789abcdef""#))
        #expect(html.contains(#"data-img-id="55""#))
        #expect(html.contains(#"data-img-secret="0123456789abcdef0123456789abcdef""#))
        #expect(html.contains(#"width="1""#) && html.contains(#"height="1""#))
    }

    @Test("The entry iTop Wrap sends reads back as the same text and the uploaded image")
    func roundTrip() async throws {
        let stub = server()
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "One\nTwo", images: [LogImage(data: png, filename: "a.png")], isPrivate: true)
        let html = try #require(addItem(stub.requests[1], "private_log")?["message"] as? String)
        let blocks = RichText.parse(html, base: URL(string: "https://itop.example.com/itop")!)
        #expect(blocks.contains(.image(.inline(id: "55"))))
        #expect(HTMLText.plain(html).contains("One\nTwo"))
    }

    @Test("Text is escaped and keeps its line breaks", arguments: [
        ("<b>bold</b> & \"q\" 'a'", "<p>&lt;b&gt;bold&lt;/b&gt; &amp; &quot;q&quot; &#39;a&#39;</p>"),
        ("Line one\nLine two", "<p>Line one<br>Line two</p>"),
        ("Para one\n\nPara two", "<p>Para one<br><br>Para two</p>"),
    ])
    func escaping(_ text: String, _ expected: String) {
        #expect(TicketWrites.html(text) == expected)
    }

    @Test("An entry with only an image is fine")
    func imageOnly() async throws {
        let stub = server()
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "  \n", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        let html = try #require(addItem(stub.requests[1])?["message"] as? String)
        #expect(html.hasPrefix("<p><img "))
    }

    @Test("Several images are uploaded in order and shown in that order")
    func several() async throws {
        let counter = Counter()
        let stub = StubServer { request in
            guard request.jsonData["operation"] as? String == "core/create" else { return .json(updated) }
            let n = counter.next() + 60
            return .json(#"{"code":0,"message":"","objects":{"InlineImage::\#(n)":{"class":"InlineImage","key":"\#(n)","code":0,"message":"created","fields":{"id":"\#(n)","secret":"s\#(n)"}}}}"#)
        }
        let images = try (0..<3).map { try LogImage(data: png, filename: "\($0).png") }
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "x", images: images, isPrivate: false)
        let html = try #require(addItem(stub.requests[3])?["message"] as? String)
        let ids = html.matches(of: /data-img-id="(\d+)"/).map { String($0.1) }
        #expect(ids == ["60", "61", "62"])
    }

    @Test("Without a secret in iTop's answer, the one iTop Wrap sent is used")
    func secretFallback() async throws {
        let stub = server(create: #"{"code":0,"message":"","objects":{"InlineImage::55":{"class":"InlineImage","key":"55","code":0,"message":"created","fields":{"id":"55"}}}}"#)
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        let sent = try #require((stub.requests[0].jsonData["fields"] as? [String: Any])?["secret"] as? String)
        #expect(sent.count == 32 && sent.allSatisfy(\.isHexDigit))
        let html = try #require(addItem(stub.requests[1])?["message"] as? String)
        #expect(html.contains(#"data-img-secret="\#(sent)""#))
    }

    @Test("Only PNG, JPEG and GIF are accepted, judged by the bytes, not the name")
    func types() throws {
        #expect(try LogImage(data: png, filename: "x.jpg").mimetype == "image/png")
        #expect(try LogImage(data: Data([0xFF, 0xD8, 0xFF, 0xE0]) + Data(count: 20), filename: "a").mimetype == "image/jpeg")
        #expect(try LogImage(data: Data("GIF89a".utf8) + Data(count: 20), filename: "a").mimetype == "image/gif")
        #expect(throws: LogImage.Failure.unsupportedType(filename: "notes.png")) { try LogImage(data: Data("hello".utf8), filename: "notes.png") }
    }

    @Test("An image over the size limit is refused before anything is sent")
    func tooLarge() throws {
        let big = png + Data(count: LogImage.maxBytes)
        #expect(throws: LogImage.Failure.tooLarge(filename: "big.png", maxMB: LogImage.maxBytes / 1_048_576)) { try LogImage(data: big, filename: "big.png") }
    }

    @Test("More images than the limit are refused before anything is sent")
    func tooMany() async throws {
        let stub = server()
        let images = try (0...LogImage.maxPerEntry).map { try LogImage(data: png, filename: "\($0).png") }
        await #expect(throws: TicketWrites.Failure.tooManyImages(max: LogImage.maxPerEntry)) {
            try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "x", images: images, isPrivate: false)
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("When iTop refuses the image, its message comes back and no entry is added")
    func imageRefused() async throws {
        let stub = server(create: #"{"code":100,"message":"Error: contents: the file is too big"}"#)
        await #expect(throws: ITopError.server(code: 100, message: "Error: contents: the file is too big")) {
            try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        }
        #expect(stub.requests.count == 1)
    }

    @Test("A per-object refusal of the image also stops before the entry")
    func imageRefusedPerObject() async throws {
        let stub = server(create: #"{"code":0,"message":"","objects":{"InlineImage::0":{"class":"InlineImage","key":"0","code":100,"message":"Error: not allowed to create InlineImage"}}}"#)
        await #expect(throws: ITopError.server(code: 100, message: "Error: not allowed to create InlineImage")) {
            try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        }
        #expect(stub.requests.count == 1)
    }

    @Test("With writes off nothing is uploaded")
    func writesOff() async throws {
        let stub = server()
        await #expect(throws: ITopError.writesDisabled) {
            try await TicketWrites(client: client(stub, writes: false)).addLogEntry(to: ticket, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        }
        #expect(stub.requests.isEmpty)
    }

    @Test("The write allowlist lets the upload through for an allowed ticket only")
    func allowlist() async throws {
        let stub = server()
        let writes = TicketWrites(client: try client(stub, allow: ["UserRequest::7"]))
        try await writes.addLogEntry(to: ticket, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        #expect(stub.requests.count == 2)

        let other = Ticket(key: "8", ticketClass: .userRequest, ref: "R-000008", title: "t", status: "assigned", priority: 3,
                           callerName: "", orgName: "", teamID: "0", teamName: "", agentID: "2", agentName: "",
                           startDate: nil, lastUpdate: nil, orgID: "3")
        await #expect(throws: ITopError.writesDisabled) {
            try await writes.addLogEntry(to: other, text: "x", images: [LogImage(data: png, filename: "a.png")], isPrivate: false)
        }
        #expect(stub.requests.count == 2)
    }

    @Test("Without images the entry is still sent as plain text")
    func noImages() async throws {
        let stub = server()
        try await TicketWrites(client: client(stub)).addLogEntry(to: ticket, text: "Hello", images: [], isPrivate: false)
        #expect(stub.requests.count == 1)
        #expect(addItem(stub.requests[0])?["format"] as? String == "text")
    }
}

private final class Counter: Sendable {
    private let value = Mutex(0)
    func next() -> Int { value.withLock { v in defer { v += 1 }; return v } }
}

import Synchronization
