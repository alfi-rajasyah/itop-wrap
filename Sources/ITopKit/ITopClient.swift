import Foundation

/// The single entry point to iTop's REST API (PLAN.md, Architecture Decisions).
public struct ITopClient: Sendable {
    public let server: ServerAddress
    public let credentials: Credentials
    public let allowsWrites: Bool
    let session: URLSession

    /// `allowsWrites` is `false` by default: during development the app only reads.
    public init(server: ServerAddress, credentials: Credentials,
                session: URLSession = .shared, allowsWrites: Bool = false) {
        self.server = server
        self.credentials = credentials
        self.session = session
        self.allowsWrites = allowsWrites
    }

    /// Sends one operation and returns iTop's answer, or throws an `ITopError`.
    public func send(_ operation: RESTOperation) async throws -> ITopResponse {
        if operation.isWrite && !allowsWrites { throw ITopError.writesDisabled }

        var request = URLRequest(url: server.restEndpoint)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.httpBody = Self.formBody([
            ("auth_user", credentials.user),
            ("auth_pwd", credentials.password),
            ("json_data", Self.encode(operation.json)),
        ])

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch let error as URLError {
            throw Self.map(error)
        } catch {
            throw ITopError.transport(URLError.Code.unknown.rawValue)
        }

        if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
            throw ITopError.http(status: http.statusCode)
        }
        guard let body = try? JSONDecoder().decode(JSONValue.self, from: data),
              case .object = body,
              let code = body["code"]?.intValue else {
            throw ITopError.unexpectedResponse
        }
        let message = body["message"]?.stringValue ?? ""
        switch code {
        case 0: return ITopResponse(body: body)
        case 1: throw Self.isPermissionMessage(message) ? ITopError.restNotAllowed : ITopError.invalidCredentials
        default: throw ITopError.server(code: code, message: message)
        }
    }

    /// `true` when iTop accepts these credentials for REST.
    public func checkCredentials() async throws -> Bool {
        let response = try await send(.checkCredentials(user: credentials.user, password: credentials.password))
        return response.body["authorized"] == .bool(true)
    }

    // MARK: - Encoding

    static func encode(_ value: JSONValue) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return String(decoding: (try? encoder.encode(value)) ?? Data("{}".utf8), as: UTF8.self)
    }

    /// application/x-www-form-urlencoded, escaping everything except RFC 3986 unreserved characters.
    static func formBody(_ pairs: [(String, String)]) -> Data {
        let escape = { (s: String) in s.addingPercentEncoding(withAllowedCharacters: unreserved) ?? "" }
        return Data(pairs.map { "\(escape($0.0))=\(escape($0.1))" }.joined(separator: "&").utf8)
    }

    /// RFC 3986 unreserved characters. Foundation's `.urlQueryAllowed` keeps `&`, `=` and `+`,
    /// which would split a password like "p&ss=w+rd" (TransportTests covers this).
    static let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")

    /// iTop answers code 1 both for a wrong password and for an account without REST rights;
    /// only the message tells them apart. The exact wording should be confirmed on the real server.
    static func isPermissionMessage(_ message: String) -> Bool {
        let m = message.lowercased()
        return m.contains("permission") || m.contains("rest services") || m.contains("not allowed")
    }

    static func map(_ error: URLError) -> ITopError {
        switch error.code {
        case .serverCertificateUntrusted, .serverCertificateHasBadDate,
             .serverCertificateHasUnknownRoot, .serverCertificateNotYetValid:
            return .untrustedCertificate
        default:
            return .transport(error.code.rawValue)
        }
    }
}

/// iTop's answer to one operation.
public struct ITopResponse: Sendable, Equatable {
    /// The whole decoded JSON body, for operations with their own fields (`authorized`, `operations`).
    public let body: JSONValue

    /// The returned objects. iTop sends them as a JSON object keyed "Class::id", which has no
    /// order, so they are sorted by id here; callers sort by what they show.
    public var objects: [ITopObject] {
        guard case .object(let map)? = body["objects"] else { return [] }
        return map.compactMap { ITopObject(json: $0.value) }
            .sorted { (Int($0.key) ?? 0, $0.key) < (Int($1.key) ?? 0, $1.key) }
    }
}

public struct ITopObject: Sendable, Equatable {
    public let className: String
    public let key: String
    /// The per-object result; writes can fail per object while the call itself succeeds.
    public let code: Int
    public let message: String
    public let fields: [String: JSONValue]

    init?(json: JSONValue) {
        guard let className = json["class"]?.stringValue else { return nil }
        self.className = className
        key = json["key"]?.stringValue ?? json["key"]?.intValue.map(String.init) ?? ""
        code = json["code"]?.intValue ?? 0
        message = json["message"]?.stringValue ?? ""
        if case .object(let f)? = json["fields"] { fields = f } else { fields = [:] }
    }
}
