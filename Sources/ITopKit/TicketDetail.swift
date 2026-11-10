import Foundation

public struct LinkedCI: Sendable, Equatable, Identifiable, Hashable {
    public let id: String
    public let name: String
    /// The CI's real class (NetworkDevice, Server…), for opening it in iTop.
    public let className: String

    public init(id: String, name: String, className: String) {
        self.id = id
        self.name = name
        self.className = className
    }
}

/// What the detail pane adds to a list row: description, both logs and linked CIs.
public struct TicketDetail: Sendable, Equatable {
    public let description: String
    /// The description's HTML, for links and images (`RichText`).
    public let descriptionHTML: String
    public let publicLog: [LogEntry]
    public let privateLog: [LogEntry]
    public let linkedCIs: [LinkedCI]
    public let serviceName: String
    public let subcategoryName: String
    /// Ids behind the names, to prefill Resolve. "0" or empty when not set.
    public let serviceID: String
    public let subcategoryID: String

    public init(description: String, publicLog: [LogEntry], privateLog: [LogEntry], linkedCIs: [LinkedCI],
                serviceName: String = "", subcategoryName: String = "", descriptionHTML: String = "",
                serviceID: String = "", subcategoryID: String = "") {
        self.description = description
        self.descriptionHTML = descriptionHTML
        self.publicLog = publicLog
        self.privateLog = privateLog
        self.linkedCIs = linkedCIs
        self.serviceName = serviceName
        self.subcategoryName = subcategoryName
        self.serviceID = serviceID
        self.subcategoryID = subcategoryID
    }

    static let fields = ["description", "public_log", "private_log", "functionalcis_list",
                         "service_id", "service_id_friendlyname", "servicesubcategory_id",
                         "servicesubcategory_id_friendlyname", "last_update"]

    init(object: ITopObject) {
        let f = object.fields
        var cis: [LinkedCI] = []
        if case .array(let links)? = f["functionalcis_list"] {
            cis = links.compactMap { link in
                guard let id = link["functionalci_id"]?.intValue else { return nil }
                let name = link["functionalci_id_friendlyname"]?.stringValue ?? link["functionalci_name"]?.stringValue ?? "CI \(id)"
                return LinkedCI(id: String(id), name: name,
                                className: link["functionalci_id_finalclass_recall"]?.stringValue ?? "FunctionalCI")
            }
        }
        self.init(description: HTMLText.plain(f["description"]?.stringValue ?? ""),
                  publicLog: CaseLog.entries(from: f["public_log"]),
                  privateLog: CaseLog.entries(from: f["private_log"]),
                  linkedCIs: cis,
                  serviceName: f["service_id_friendlyname"]?.stringValue ?? "",
                  subcategoryName: f["servicesubcategory_id_friendlyname"]?.stringValue ?? "",
                  descriptionHTML: f["description"]?.stringValue ?? "",
                  serviceID: f["service_id"]?.stringValue ?? f["service_id"]?.intValue.map(String.init) ?? "",
                  subcategoryID: f["servicesubcategory_id"]?.stringValue ?? f["servicesubcategory_id"]?.intValue.map(String.init) ?? "")
    }
}

extension TicketService {
    /// The full ticket for the detail pane. Read-only.
    public func detail(for ticket: Ticket) async throws -> TicketDetail {
        let response = try await client.send(.get(className: ticket.ticketClass.rawValue, key: ticket.key,
                                                  outputFields: TicketDetail.fields))
        guard let object = response.objects.first else {
            throw ITopError.server(code: 0, message: "\(ticket.ref) wasn't returned. It may have been deleted, or your account can't see it any more.")
        }
        return TicketDetail(object: object)
    }
}
