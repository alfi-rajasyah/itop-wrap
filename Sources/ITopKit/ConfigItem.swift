import Foundation

/// A CI in search results. `className` is the real class (Server, NetworkDevice…), not FunctionalCI.
public struct ConfigItem: Sendable, Equatable, Identifiable, Hashable {
    public var id: String { "\(className)::\(key)" }
    public let key: String
    public let className: String
    public let name: String
    public let orgName: String

    public init(key: String, className: String, name: String, orgName: String) {
        self.key = key
        self.className = className
        self.name = name
        self.orgName = orgName
    }

    /// "ApplicationSolution" → "Application solution", "PC" stays "PC".
    public var classLabel: String { ConfigItem.label(forClass: className) }

    static func label(forClass name: String) -> String {
        var words: [String] = []
        var current = ""
        let chars = Array(name)
        for (i, c) in chars.enumerated() {
            let nextIsLower = i + 1 < chars.count && chars[i + 1].isLowercase
            if c.isUppercase, !current.isEmpty, current.last!.isLowercase || nextIsLower {
                words.append(current)
                current = ""
            }
            current.append(c)
        }
        if !current.isEmpty { words.append(current) }
        return words.enumerated().map { i, w in
            i == 0 || w.allSatisfy(\.isUppercase) ? w : w.lowercased()
        }.joined(separator: " ")
    }
}

/// A CI on the other end of a relationship.
public struct CIRef: Sendable, Equatable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let className: String

    public init(id: String, name: String, className: String) {
        self.id = id
        self.name = name
        self.className = className
    }

    public var item: ConfigItem { ConfigItem(key: id, className: className, name: name, orgName: "") }
}

public struct CIField: Sendable, Equatable, Identifiable {
    public var id: String { label }
    public let label: String
    public let value: String

    public init(label: String, value: String) {
        self.label = label
        self.value = value
    }
}

/// The CI detail pane: main fields, relationships and open tickets (DESIGN.md, Config item detail).
public struct ConfigItemDetail: Sendable, Equatable {
    public let item: ConfigItem
    public let fields: [CIField]
    public let description: String
    public let dependsOn: [CIRef]
    public let usedBy: [CIRef]
    public let openTickets: [Ticket]

    public init(item: ConfigItem, fields: [CIField], description: String, dependsOn: [CIRef], usedBy: [CIRef], openTickets: [Ticket]) {
        self.item = item
        self.fields = fields
        self.description = description
        self.dependsOn = dependsOn
        self.usedBy = usedBy
        self.openTickets = openTickets
    }

    /// The fields worth showing, in this order, when the class has them and they're filled in.
    static let shown: [(keys: [String], label: String, humanize: Bool)] = [
        (["status"], "Status", true),
        (["org_id_friendlyname", "organization_name"], "Organization", false),
        (["business_criticity"], "Business criticality", true),
        (["location_id_friendlyname", "location_name"], "Location", false),
        (["brand_id_friendlyname", "brand_name"], "Brand", false),
        (["model_id_friendlyname", "model_name"], "Model", false),
        (["osfamily_id_friendlyname", "osfamily_name"], "OS family", false),
        (["osversion_id_friendlyname", "osversion_name"], "OS version", false),
        (["serialnumber"], "Serial number", false),
        (["asset_number"], "Asset number", false),
        (["managementip"], "Management IP", false),
        (["cpu"], "CPU", false),
        (["ram"], "RAM", false),
        (["move2production"], "In production since", false),
    ]

    public static func fields(from f: [String: JSONValue]) -> [CIField] {
        var out: [CIField] = shown.compactMap { entry in
            guard let value = entry.keys.lazy.compactMap({ f[$0]?.stringValue }).first(where: { !$0.isEmpty && $0 != "0" }) else { return nil }
            return CIField(label: entry.label, value: entry.humanize ? humanize(value) : value)
        }
        if case .array(let contacts)? = f["contacts_list"] {
            let names = contacts.compactMap { $0["contact_id_friendlyname"]?.stringValue }.filter { !$0.isEmpty }
            if !names.isEmpty { out.append(CIField(label: "Contacts", value: names.joined(separator: ", "))) }
        }
        return out
    }

    /// iTop codes as words: "not_applicable" → "Not applicable".
    public static func humanize(_ code: String) -> String {
        let s = code.replacingOccurrences(of: "_", with: " ")
        return s.prefix(1).uppercased() + s.dropFirst()
    }
}

public enum CMDBQueries {
    public static let searchLimit = 50

    /// CIs whose name contains the text, or `nil` for fewer than 2 characters.
    public static func search(_ text: String) -> String? {
        let q = text.trimmingCharacters(in: .whitespaces)
        guard q.count >= 2 else { return nil }
        return "SELECT FunctionalCI WHERE name LIKE \(OQL.quote("%\(q)%"))"
    }

    /// Open tickets linked to a CI through lnkFunctionalCIToTicket.
    public static func openTickets(_ cls: TicketClass, ciID: String) -> String? {
        guard let id = Int(ciID) else { return nil }
        return "SELECT \(cls.rawValue) AS t JOIN lnkFunctionalCIToTicket AS l ON l.ticket_id = t.id WHERE l.functionalci_id = \(id) AND t.status NOT IN ('resolved','closed')"
    }
}

/// CMDB reads (PLAN.md Task 5). Read-only.
public struct CMDBService: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    public enum Direction: String, Sendable { case up, down }

    public func search(_ text: String) async throws -> [ConfigItem] {
        guard let oql = CMDBQueries.search(text) else { return [] }
        let response = try await client.send(.get(className: "FunctionalCI", key: oql,
                                                  outputFields: ["name", "finalclass", "org_id_friendlyname"],
                                                  limit: CMDBQueries.searchLimit))
        return response.objects.prefix(CMDBQueries.searchLimit).map { o in
            ConfigItem(key: o.key, className: o.fields["finalclass"]?.stringValue ?? o.className,
                       name: o.fields["name"]?.stringValue ?? "", orgName: o.fields["org_id_friendlyname"]?.stringValue ?? "")
        }
        .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    /// The CI with its fields, both directions of "impacts" and its open tickets, loaded together.
    public func detail(_ item: ConfigItem) async throws -> ConfigItemDetail {
        async let full = client.send(.get(className: item.className, key: item.key, outputFields: ["*"]))
        async let up = related(className: item.className, key: item.key, direction: .up)
        async let down = related(className: item.className, key: item.key, direction: .down)
        async let tickets = openTickets(ciID: item.key)

        let object = try await full.objects.first
        let f = object?.fields ?? [:]
        let resolved = ConfigItem(key: item.key, className: item.className,
                                  name: f["name"]?.stringValue ?? item.name,
                                  orgName: f["org_id_friendlyname"]?.stringValue ?? item.orgName)
        return ConfigItemDetail(item: resolved, fields: ConfigItemDetail.fields(from: f),
                                description: HTMLText.plain(f["description"]?.stringValue ?? ""),
                                dependsOn: (try? await up) ?? [], usedBy: (try? await down) ?? [],
                                openTickets: try await tickets)
    }

    /// Direct "impacts" neighbours: `.up` gives what this CI depends on, `.down` what uses it.
    public func related(className: String, key: String, direction: Direction) async throws -> [CIRef] {
        let response = try await client.send(.getRelated(className: className, key: key, relation: "impacts",
                                                         depth: 1, direction: direction.rawValue))
        return Self.relatedItems(response, excluding: className, key: key)
    }

    static let notCIs: Set<String> = ["Person", "Team", "Contact", "Organization", "Location", "UserRequest", "Incident",
                                      "Problem", "Change", "NormalChange", "RoutineChange", "EmergencyChange", "Document"]

    static func relatedItems(_ response: ITopResponse, excluding className: String, key: String) -> [CIRef] {
        response.objects
            .filter { !($0.className == className && $0.key == key) && !notCIs.contains($0.className) && !$0.className.hasPrefix("Document") }
            .map { o in
                CIRef(id: o.key, name: o.fields["friendlyname"]?.stringValue ?? o.fields["name"]?.stringValue ?? "\(o.className) \(o.key)",
                      className: o.className)
            }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    public func openTickets(ciID: String) async throws -> [Ticket] {
        var all: [Ticket] = []
        for cls in TicketClass.allCases {
            guard let oql = CMDBQueries.openTickets(cls, ciID: ciID) else { continue }
            let response = try await client.send(.get(className: cls.rawValue, key: oql, outputFields: Ticket.listFields))
            all += response.objects.compactMap(Ticket.init(object:))
        }
        return all
    }
}
