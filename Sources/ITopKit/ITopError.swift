import Foundation

public enum ITopError: Error, Equatable, Sendable {
    /// The server address can't be used (empty, or not http/https).
    case invalidServerAddress
    /// iTop code 1: wrong username or password.
    case invalidCredentials
    /// iTop code 1 with a permissions message: the account lacks the "REST Services User" profile.
    case restNotAllowed
    /// Any other non-zero iTop `code`, with iTop's own message.
    case server(code: Int, message: String)
    /// The web server answered with an HTTP error status.
    case http(status: Int)
    /// The network request failed; the value is the `URLError.Code` raw value.
    case transport(Int)
    /// TLS failed because the Mac doesn't trust the server's certificate.
    case untrustedCertificate
    /// The answer wasn't iTop JSON (for example a login page or a proxy page).
    case unexpectedResponse
    /// A write was attempted while writes are switched off (SPEC.md, Boundaries).
    case writesDisabled
}

extension ITopError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .invalidServerAddress:
            return "That server address can't be used. Enter it like https://itop.example.com."
        case .invalidCredentials:
            return "Invalid credentials. Check your username and password."
        case .restNotAllowed:
            return "This account can't use the iTop REST API. Ask your iTop admin to add the \"REST Services User\" profile, then sign in again."
        case .server(let code, let message):
            return message.isEmpty ? "iTop returned error \(code)." : "iTop returned error \(code): \(message)"
        case .http(let status):
            return "The server answered with HTTP \(status)."
        case .transport:
            return "Can't reach the server. Check your connection and the server address."
        case .untrustedCertificate:
            return "This Mac doesn't trust the server's certificate. Ask your iTop admin, or trust its certificate in Keychain Access."
        case .unexpectedResponse:
            return "The server didn't answer like iTop. Check that the address points to your iTop."
        case .writesDisabled:
            return "Changes to iTop are switched off in this build."
        }
    }
}
