import Foundation
import Testing
@testable import ITopKit

/// A fake iTop that answers like your real server did in the Task 0 probe.
private func fakeITop(rejectPassword: Bool = false, restMessage: String? = nil) -> StubServer {
    StubServer { request in
        if rejectPassword { return .json(#"{"code":1,"message":"Error: Invalid login"}"#) }
        if let restMessage { return .json(#"{"code":1,"message":"\#(restMessage)"}"#) }
        switch (request.jsonData["operation"] as? String, request.jsonData["class"] as? String) {
        case ("core/check_credentials", _): return .fixture("check_credentials")
        case ("core/get", "User"): return .fixture("user")
        case ("core/get", "Person"): return .fixture("person")
        default: return .json(#"{"code":11,"message":"Error: Unknown operation"}"#)
        }
    }
}

private final class MemorySecrets: SecretStore, @unchecked Sendable {
    private let lock = NSLock()
    private var items: [String: String] = [:]
    func password(for account: String) throws -> String? { lock.withLock { items[account] } }
    func setPassword(_ password: String, for account: String) throws { lock.withLock { items[account] = password } }
    func deletePassword(for account: String) throws { lock.withLock { _ = items.removeValue(forKey: account) } }
    var count: Int { lock.withLock { items.count } }
}

private func service(_ stub: StubServer, secrets: MemorySecrets = MemorySecrets(),
                     settings: SessionSettings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)) -> SessionService {
    SessionService(secrets: secrets, settings: settings) { server, credentials in
        ITopClient(server: server, credentials: credentials, session: stub.session)
    }
}

// MARK: - Current user

@Suite("Current user")
struct CurrentUserTests {
    @Test("Loads your User, then your Person and teams, from the real response shape")
    func loadsFromFixtures() async throws {
        let stub = fakeITop()
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        let me = try await CurrentUser.load(login: "agent", client: client)

        #expect(me.userID == "1001")
        #expect(me.contactID == "1002")
        #expect(me.profiles.count == 3)
        #expect(me.teams == [Team(id: "1009", name: "[text]")])
        #expect(me.displayName == "[text] [text]")
    }

    @Test("The login is quoted safely in the OQL query")
    func loginIsQuoted() async throws {
        let stub = fakeITop()
        let client = ITopClient(server: try ServerAddress("itop.example.com"),
                                credentials: Credentials(user: "o'brien", password: "pw"), session: stub.session)

        _ = try await CurrentUser.load(login: #"o'brien\x"#, client: client)

        let key = stub.requests.first { $0.jsonData["class"] as? String == "User" }?.jsonData["key"] as? String
        #expect(key == #"SELECT User WHERE login = 'o\'brien\\x'"#)
    }

    @Test("Initials come from the display name")
    func initials() {
        let me = CurrentUser(userID: "1", contactID: "2", login: "jdoe", displayName: "Jane van Doe", profiles: [], teams: [])
        #expect(me.initials == "JD")
    }
}

// MARK: - Session: sign in, relaunch, sign out

@Suite("Session")
struct SessionTests {
    @Test("Signing in saves the password to the secret store and the server and username to settings")
    func signInSaves() async throws {
        let stub = fakeITop()
        let secrets = MemorySecrets()
        let settings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)

        let session = try await service(stub, secrets: secrets, settings: settings)
            .signIn(server: "itop.example.com", user: "agent", password: "pw")

        #expect(session.user.contactID == "1002")
        #expect(settings.server == "https://itop.example.com")
        #expect(settings.user == "agent")
        #expect(try secrets.password(for: SessionService.account(user: "agent", server: session.client.server)) == "pw")
    }

    @Test("A wrong password shows as invalid credentials and saves nothing")
    func wrongPassword() async throws {
        let stub = fakeITop(rejectPassword: true)
        let secrets = MemorySecrets()
        let settings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)

        await #expect(throws: ITopError.invalidCredentials) {
            _ = try await service(stub, secrets: secrets, settings: settings)
                .signIn(server: "itop.example.com", user: "agent", password: "wrong")
        }
        #expect(secrets.count == 0)
        #expect(settings.user == nil)
    }

    @Test("An account without REST rights gets its own error, not 'invalid credentials'")
    func noRESTProfile() async throws {
        let stub = fakeITop(restMessage: "Error: The current user does not have enough permissions for using the REST services")
        await #expect(throws: ITopError.restNotAllowed) {
            _ = try await service(stub).signIn(server: "itop.example.com", user: "agent", password: "pw")
        }
    }

    @Test("After a relaunch the saved session comes back without asking for anything")
    func restore() async throws {
        let stub = fakeITop()
        let secrets = MemorySecrets()
        let settings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        _ = try await service(stub, secrets: secrets, settings: settings)
            .signIn(server: "itop.example.com", user: "agent", password: "pw")

        let restored = try await service(stub, secrets: secrets, settings: settings).restore()

        #expect(restored?.user.contactID == "1002")
        #expect(restored?.client.credentials == Credentials(user: "agent", password: "pw"))
    }

    @Test("Nothing saved means nothing to restore")
    func restoreEmpty() async throws {
        #expect(try await service(fakeITop()).restore() == nil)
    }

    @Test("If the saved password stops working, restore fails and keeps the server and username for the sign-in form")
    func restoreRejected() async throws {
        let secrets = MemorySecrets()
        let settings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        _ = try await service(fakeITop(), secrets: secrets, settings: settings)
            .signIn(server: "itop.example.com", user: "agent", password: "old")

        await #expect(throws: ITopError.invalidCredentials) {
            _ = try await service(fakeITop(rejectPassword: true), secrets: secrets, settings: settings).restore()
        }
        #expect(settings.server == "https://itop.example.com")
        #expect(settings.user == "agent")
    }

    @Test("Signing out removes the password, so the next launch shows sign-in")
    func signOut() async throws {
        let stub = fakeITop()
        let secrets = MemorySecrets()
        let settings = SessionSettings(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let svc = service(stub, secrets: secrets, settings: settings)
        let session = try await svc.signIn(server: "itop.example.com", user: "agent", password: "pw")

        try svc.signOut(session)

        #expect(secrets.count == 0)
        #expect(try await svc.restore() == nil)
        #expect(settings.user == "agent", "the username stays filled in for next time")
    }
}

// MARK: - Keychain

@Suite("Keychain")
struct KeychainTests {
    @Test("A password round-trips through the macOS Keychain and is gone after delete")
    func roundTrip() throws {
        let store = KeychainStore(service: "iTop Wrap tests \(UUID().uuidString)")
        let account = "agent@itop.example.com"
        defer { try? store.deletePassword(for: account) }

        try store.setPassword("first", for: account)
        try store.setPassword("second", for: account)
        #expect(try store.password(for: account) == "second")

        try store.deletePassword(for: account)
        #expect(try store.password(for: account) == nil)
    }
}
