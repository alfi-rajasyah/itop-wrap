import Foundation

/// The server address and username, remembered between launches. Never the password.
public struct SessionSettings: @unchecked Sendable {
    // UserDefaults is thread-safe; @unchecked because older SDKs don't mark it Sendable.
    let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public var server: String? {
        get { defaults.string(forKey: "server") }
        nonmutating set { defaults.set(newValue, forKey: "server") }
    }

    public var user: String? {
        get { defaults.string(forKey: "user") }
        nonmutating set { defaults.set(newValue, forKey: "user") }
    }
}

/// A signed-in client and who it belongs to.
public struct Session: Sendable {
    public let client: ITopClient
    public let user: CurrentUser

    public init(client: ITopClient, user: CurrentUser) {
        self.client = client
        self.user = user
    }
}

/// Whether the session may change anything in iTop (SPEC.md, Boundaries: read-only until you say so).
public enum WriteAccess: Sendable, Equatable {
    case off
    /// Writes only on these tickets, given by ref ("R-079999"), for testing on a real iTop;
    /// `newTickets` also allows creating tickets.
    case tickets([String], newTickets: Bool = false)
}

/// Sign in, restore after a relaunch, and sign out (PLAN.md, Task 2).
public struct SessionService: Sendable {
    let secrets: SecretStore
    let settings: SessionSettings
    let writeAccess: WriteAccess
    let makeClient: @Sendable (ServerAddress, Credentials) -> ITopClient

    public init(secrets: SecretStore = KeychainStore(), settings: SessionSettings = SessionSettings(), writeAccess: WriteAccess = .off,
                makeClient: @escaping @Sendable (ServerAddress, Credentials) -> ITopClient = { ITopClient(server: $0, credentials: $1) }) {
        self.secrets = secrets
        self.settings = settings
        self.writeAccess = writeAccess
        self.makeClient = makeClient
    }

    /// The Keychain account for a user on a server.
    public static func account(user: String, server: ServerAddress) -> String {
        "\(user)@\(server.baseURL.absoluteString)"
    }

    public var savedServer: String? { settings.server }
    public var savedUser: String? { settings.user }

    /// Checks the credentials, loads who you are, then saves the password to the Keychain.
    /// Nothing is saved if any step fails.
    public func signIn(server: String, user: String, password: String) async throws -> Session {
        let address = try ServerAddress(server)
        let login = user.trimmingCharacters(in: .whitespacesAndNewlines)
        let session = try await open(address, Credentials(user: login, password: password))
        try secrets.setPassword(password, for: Self.account(user: login, server: address))
        settings.server = address.baseURL.absoluteString
        settings.user = login
        return session
    }

    /// The saved session, or `nil` when nothing is saved. Throws when the saved password
    /// no longer works; the server and username stay saved for the sign-in form.
    public func restore() async throws -> Session? {
        guard let server = settings.server, let user = settings.user,
              let address = try? ServerAddress(server),
              let password = try secrets.password(for: Self.account(user: user, server: address)) else {
            return nil
        }
        return try await open(address, Credentials(user: user, password: password))
    }

    /// Removes the password from the Keychain. The server and username stay filled in.
    public func signOut(_ session: Session) throws {
        try secrets.deletePassword(for: Self.account(user: session.client.credentials.user, server: session.client.server))
    }

    private func open(_ address: ServerAddress, _ credentials: Credentials) async throws -> Session {
        let client = makeClient(address, credentials)
        guard try await client.checkCredentials() else { throw ITopError.invalidCredentials }
        let me = try await CurrentUser.load(login: credentials.user, client: client)
        guard case .tickets(let refs, let newTickets) = writeAccess else { return Session(client: client, user: me) }
        // Resolve each ref to its exact object; refs that don't exist stay out of the allowlist.
        let service = TicketService(client: client)
        var allowed: Set<String> = []
        for ref in refs {
            if let t = try? await service.lookup(ref) { allowed.insert(TicketWrites.allowlistKey(for: t)) }
        }
        return Session(client: client.allowingWrites(only: allowed, newTickets: newTickets), user: me)
    }
}
