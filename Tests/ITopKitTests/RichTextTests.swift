import Foundation
import Testing
@testable import ITopKit

private let base = URL(string: "https://itop.example.com/itop")!

private func client(_ stub: StubServer) throws -> ITopClient {
    ITopClient(server: try ServerAddress("https://itop.example.com/itop"), credentials: Credentials(user: "a", password: "p"), session: stub.session)
}

@Suite("Rich text")
struct RichTextTests {
    @Test("Links become runs with a URL; the rest is plain text")
    func links() {
        let blocks = RichText.parse(#"<p>See <a href="https://kb.example.com/1">the KB article</a>.</p>"#, base: base)
        #expect(blocks == [.text([RichRun(text: "See "), RichRun(text: "the KB article", link: URL(string: "https://kb.example.com/1")),
                                  RichRun(text: ".")])])
    }

    @Test("Only web and email links are kept; the text of others stays", arguments: [
        #"<a href="javascript:alert(1)">click</a>"#, #"<a href="file:///etc/passwd">click</a>"#, #"<a>click</a>"#,
    ])
    func unsafeLinks(_ html: String) {
        #expect(RichText.parse(html, base: base) == [.text([RichRun(text: "click")])])
    }

    @Test("Email links are kept")
    func mailto() {
        #expect(RichText.parse(#"<a href="mailto:helpdesk@example.com">mail us</a>"#, base: base) ==
                [.text([RichRun(text: "mail us", link: URL(string: "mailto:helpdesk@example.com"))])])
    }

    @Test("Relative links resolve against your iTop")
    func relative() {
        let blocks = RichText.parse(#"<a href="pages/UI.php?operation=details&amp;id=5">R-5</a>"#, base: base)
        #expect(blocks == [.text([RichRun(text: "R-5", link: URL(string: "https://itop.example.com/itop/pages/UI.php?operation=details&id=5"))])])
    }

    @Test("A pasted iTop image becomes an inline image by id", arguments: [
        #"<img src="https://itop.example.com/itop/pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=abc" data-img-id="42">"#,
        #"<img src="pages/ajax.document.php?operation=download_inlineimage&amp;id=42&amp;s=abc">"#,
    ])
    func inlineImage(_ html: String) {
        #expect(RichText.parse(html, base: base) == [.image(.inline(id: "42"))])
    }

    @Test("An image from another host is never loaded: it's kept as an external image")
    func external() {
        let blocks = RichText.parse(#"<img src="https://cdn.example.net/pic.png" alt="diagram">"#, base: base)
        #expect(blocks == [.image(.external(URL(string: "https://cdn.example.net/pic.png")!))])
    }

    @Test("A small image from another host (a signature icon) stays in the line as an icon, still not loaded", arguments: [
        #"<p><img src="https://lh5.googleusercontent.com/phone" width="16" height="16"> : +62 878</p>"#,
        #"<p><img src="https://lh5.googleusercontent.com/phone" style="width: 12px; height:12px"> : +62 878</p>"#,
    ])
    func smallExternalIsInline(_ html: String) {
        let url = URL(string: "https://lh5.googleusercontent.com/phone")!
        #expect(RichText.parse(html, base: base) == [.text([RichRun(icon: url), RichRun(text: " : +62 878")])])
    }

    @Test("A large or unsized outside image is still its own block", arguments: [
        #"<img src="https://cdn.example.net/pic.png" width="600" height="400">"#,
        #"<img src="https://cdn.example.net/pic.png">"#,
    ])
    func largeExternalIsBlock(_ html: String) {
        #expect(RichText.parse(html, base: base) == [.image(.external(URL(string: "https://cdn.example.net/pic.png")!))])
    }

    @Test("A data: image is decoded in place, with no network")
    func dataImage() {
        let blocks = RichText.parse(#"<img src="data:image/png;base64,QUJD">"#, base: base)
        #expect(blocks == [.image(.data(Data("ABC".utf8)))])
    }

    @Test("Text around an image stays in order, trimmed, with empty text dropped")
    func order() {
        let blocks = RichText.parse(#"<p>Before</p><figure><img src="pages/ajax.document.php?operation=download_inlineimage&amp;id=7"></figure><p>After</p>"#, base: base)
        #expect(blocks == [.text([RichRun(text: "Before")]), .image(.inline(id: "7")), .text([RichRun(text: "After")])])
    }

    @Test("Without images or links it reads the same as the plain-text version", arguments: [
        "<p>Hello<br>world</p><p>Second</p>", "<ul><li>One</li><li>Two</li></ul>", "<p>[text]<br>\u{00A0}</p>", "Fish &amp; chips",
    ])
    func matchesPlain(_ html: String) {
        let text = RichText.parse(html, base: base).compactMap { block -> String? in
            if case .text(let runs) = block { return runs.map(\.text).joined() }
            return nil
        }.joined(separator: "\n\n")
        #expect(text == HTMLText.plain(html))
    }

    @Test("Empty HTML gives no blocks")
    func empty() {
        #expect(RichText.parse("", base: base).isEmpty)
        #expect(RichText.parse("<p> </p>", base: base).isEmpty)
    }
}

@Suite("Inline images and attachments")
struct InlineImageTests {
    @Test("An inline image is read through REST by id and decoded")
    func load() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"Found: 1","objects":{"InlineImage::42":{"class":"InlineImage","key":"42","fields":{"contents":{"data":"QUJD","mimetype":"image/png","filename":"x.png"}}}}}"#))
        let loader = InlineImageLoader(client: try client(stub))

        let image = try await loader.image(id: "42")

        #expect(image.data == Data("ABC".utf8))
        #expect(image.mimetype == "image/png")
        let r = try #require(stub.requests.first).jsonData
        #expect(r["class"] as? String == "InlineImage")
        #expect(r["key"] as? String == "SELECT InlineImage WHERE id = 42")
        #expect(r["output_fields"] as? String == "contents")
    }

    @Test("A second request for the same image comes from memory")
    func cached() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"InlineImage::42":{"class":"InlineImage","key":"42","fields":{"contents":{"data":"QUJD","mimetype":"image/png"}}}}}"#))
        let loader = InlineImageLoader(client: try client(stub))
        _ = try await loader.image(id: "42")
        _ = try await loader.image(id: "42")
        #expect(stub.requests.count == 1)
    }

    @Test("A missing image or a bad id is an error, not a crash", arguments: ["42", "4 OR 1=1"])
    func missing(_ id: String) async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"Found: 0","objects":null}"#))
        let loader = InlineImageLoader(client: try client(stub))
        await #expect(throws: InlineImageLoader.Failure.self) { _ = try await loader.image(id: id) }
    }

    @Test("Attachments are listed by name without downloading them")
    func list() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"Attachment::3":{"class":"Attachment","key":"3","fields":{"friendlyname":"report.pdf"}}}}"#))
        let ticket = Ticket(key: "7", ticketClass: .userRequest, ref: "R-7", title: "", status: "assigned", priority: 3, callerName: "",
                            orgName: "", teamID: "0", teamName: "", agentID: "0", agentName: "", startDate: nil, lastUpdate: nil)

        let list = try await InlineImageLoader(client: try client(stub)).attachments(of: ticket)

        #expect(list == [AttachmentRef(id: "3", name: "report.pdf")])
        let r = try #require(stub.requests.first).jsonData
        #expect(r["key"] as? String == "SELECT Attachment WHERE item_class = 'UserRequest' AND item_id = 7")
        #expect(r["output_fields"] as? String == "friendlyname", "the list never asks for file contents")
    }

    @Test("Downloading an attachment returns its bytes, name and type")
    func download() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{"Attachment::3":{"class":"Attachment","key":"3","fields":{"contents":{"data":"QUJD","mimetype":"application/pdf","filename":"report.pdf"}}}}}"#))
        let file = try await InlineImageLoader(client: try client(stub)).download(AttachmentRef(id: "3", name: "report.pdf"))
        #expect(file.data == Data("ABC".utf8) && file.filename == "report.pdf" && file.mimetype == "application/pdf")
    }
}
