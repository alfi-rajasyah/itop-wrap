import Foundation

/// The iTop server the user typed or pasted, normalised to its base URL.
///
/// Accepts a bare host ("itop.example.com", treated as https), a base URL, or a pasted
/// iTop page or REST URL, which is trimmed back to the folder iTop is installed in.
public struct ServerAddress: Sendable, Equatable {
    public static let restVersion = "1.4"

    public let baseURL: URL

    /// `false` for http://. The UI warns before sending a password over it.
    public var isSecure: Bool { baseURL.scheme == "https" }

    public var restEndpoint: URL {
        URL(string: baseURL.absoluteString + "/webservices/rest.php?version=" + Self.restVersion)!
    }

    public init(_ input: String) throws {
        var text = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { throw ITopError.invalidServerAddress }
        if !text.contains("://") { text = "https://" + text }

        guard let parts = URLComponents(string: text),
              let scheme = parts.scheme?.lowercased(), scheme == "https" || scheme == "http",
              let host = parts.host, !host.isEmpty else {
            throw ITopError.invalidServerAddress
        }

        var path = parts.path
        for marker in ["/pages/", "/webservices/"] {
            if let range = path.range(of: marker) { path = String(path[..<range.lowerBound]) }
        }
        while path.hasSuffix("/") { path.removeLast() }

        let port = parts.port.map { ":\($0)" } ?? ""
        guard let url = URL(string: "\(scheme)://\(host)\(port)\(path)") else {
            throw ITopError.invalidServerAddress
        }
        baseURL = url
    }
}

/// The username and password sent with every REST call. The password is never printed.
public struct Credentials: Sendable, Equatable, CustomStringConvertible {
    public let user: String
    public let password: String

    public init(user: String, password: String) {
        self.user = user
        self.password = password
    }

    public var description: String { "Credentials(user: \(user), password: ***)" }
}
