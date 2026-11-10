import Foundation

public struct Team: Sendable, Equatable, Hashable, Identifiable {
    public let id: String
    public let name: String

    public init(id: String, name: String) {
        self.id = id
        self.name = name
    }
}

/// Who is signed in: their iTop User, the Person behind it, and their teams.
/// "My tickets" filter on `contactID`; "needs you" and Team tickets use `teams`.
public struct CurrentUser: Sendable, Equatable {
    public let userID: String
    public let contactID: String
    public let login: String
    public let displayName: String
    public let profiles: [String]
    public let teams: [Team]

    public init(userID: String, contactID: String, login: String, displayName: String, profiles: [String], teams: [Team]) {
        self.userID = userID
        self.contactID = contactID
        self.login = login
        self.displayName = displayName
        self.profiles = profiles
        self.teams = teams
    }

    /// Two letters for the account button, e.g. "Jane van Doe" → "JD".
    public var initials: String {
        let words = displayName.split(separator: " ").filter { $0.first?.isLetter == true }
        let picked = words.count > 1 ? [words.first!, words.last!] : Array(words.prefix(1))
        let letters = picked.compactMap { $0.first.map { String($0).uppercased() } }.joined()
        return letters.isEmpty ? String(login.prefix(2)).uppercased() : letters
    }

    /// Reads the User with this login, then its Person and team links. Read-only.
    public static func load(login: String, client: ITopClient) async throws -> CurrentUser {
        let users = try await client.send(.get(className: "User", key: "SELECT User WHERE login = \(OQL.quote(login))",
                                               outputFields: ["login", "contactid", "first_name", "last_name", "profile_list"]))
        guard let user = users.objects.first else { throw ITopError.invalidCredentials }
        let f = user.fields
        let contactID = f["contactid"]?.intValue.map(String.init) ?? "0"
        let profiles: [String] = {
            guard case .array(let list)? = f["profile_list"] else { return [] }
            return list.compactMap { $0["profile"]?.stringValue }
        }()

        var teams: [Team] = []
        var name = [f["first_name"]?.stringValue, f["last_name"]?.stringValue].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " ")
        if contactID != "0" {
            let people = try await client.send(.get(className: "Person", key: "SELECT Person WHERE id = \(contactID)",
                                                    outputFields: ["name", "first_name", "team_list"]))
            if let person = people.objects.first {
                let p = person.fields
                let personName = [p["first_name"]?.stringValue, p["name"]?.stringValue].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " ")
                if !personName.isEmpty { name = personName }
                if case .array(let links)? = p["team_list"] {
                    teams = links.compactMap { link in
                        guard let id = link["team_id"]?.intValue else { return nil }
                        return Team(id: String(id), name: link["team_name"]?.stringValue ?? "Team \(id)")
                    }
                }
            }
        }
        return CurrentUser(userID: user.key, contactID: contactID, login: login,
                           displayName: name.isEmpty ? login : name, profiles: profiles, teams: teams)
    }
}
