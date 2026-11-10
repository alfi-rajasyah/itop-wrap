import Foundation

/// Serves canned responses to a URLSession and records what it was sent.
/// Each `StubServer` has its own id, so tests can run in parallel.
final class StubURLProtocol: URLProtocol, @unchecked Sendable {
    static let header = "X-Stub-ID"

    override class func canInit(with request: URLRequest) -> Bool {
        request.value(forHTTPHeaderField: header) != nil
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let id = request.value(forHTTPHeaderField: Self.header),
              let server = StubRegistry.shared.server(id) else {
            client?.urlProtocol(self, didFailWithError: URLError(.unsupportedURL))
            return
        }
        let recorded = RecordedRequest(request: request, body: Self.body(of: request))
        server.record(recorded)
        switch server.reply {
        case .failure(let error):
            client?.urlProtocol(self, didFailWithError: error)
        case .response(let status, let contentType, let body):
            let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1",
                                           headerFields: ["Content-Type": contentType])!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        }
    }

    override func stopLoading() {}

    /// URLSession moves the body into a stream before a URLProtocol sees it.
    private static func body(of request: URLRequest) -> Data {
        if let data = request.httpBody { return data }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let n = stream.read(&buffer, maxLength: buffer.count)
            if n <= 0 { break }
            data.append(buffer, count: n)
        }
        return data
    }
}

struct RecordedRequest: Sendable {
    let request: URLRequest
    let body: Data

    /// The form fields of an application/x-www-form-urlencoded body.
    var form: [String: String] {
        var c = URLComponents()
        c.percentEncodedQuery = String(decoding: body, as: UTF8.self)
        var out: [String: String] = [:]
        for item in c.queryItems ?? [] { out[item.name] = item.value ?? "" }
        return out
    }

    /// `json_data` parsed back into a dictionary.
    var jsonData: [String: Any] {
        let data = Data((form["json_data"] ?? "").utf8)
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }
}

enum StubReply: Sendable {
    case response(status: Int, contentType: String, body: Data)
    case failure(URLError)

    static func json(_ text: String, status: Int = 200) -> StubReply {
        .response(status: status, contentType: "application/json", body: Data(text.utf8))
    }
}

final class StubServer: @unchecked Sendable {
    let id = UUID().uuidString
    let reply: StubReply
    private let lock = NSLock()
    private var _requests: [RecordedRequest] = []

    init(_ reply: StubReply) {
        self.reply = reply
        StubRegistry.shared.add(self)
    }

    deinit { StubRegistry.shared.remove(id) }

    var requests: [RecordedRequest] { lock.withLock { _requests } }
    func record(_ r: RecordedRequest) { lock.withLock { _requests.append(r) } }

    /// A session that routes every request to this stub.
    var session: URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        config.httpAdditionalHeaders = [StubURLProtocol.header: id]
        return URLSession(configuration: config)
    }
}

final class StubRegistry: @unchecked Sendable {
    static let shared = StubRegistry()
    private let lock = NSLock()
    private var servers: [String: StubServer] = [:]

    func add(_ s: StubServer) { lock.withLock { servers[s.id] = s } }
    func remove(_ id: String) { lock.withLock { _ = servers.removeValue(forKey: id) } }
    func server(_ id: String) -> StubServer? { lock.withLock { servers[id] } }
}

enum Fixture {
    static func data(_ name: String) throws -> Data {
        guard let url = Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Fixtures") else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try Data(contentsOf: url)
    }
}
