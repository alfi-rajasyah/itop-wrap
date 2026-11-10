import Foundation

/// Changes to tickets (PLAN.md Phase 3). Every one goes through `ITopClient`, which refuses it
/// unless writes are on and, with an allowlist, the ticket is on it.
public struct TicketWrites: Sendable {
    let client: ITopClient

    public init(client: ITopClient) {
        self.client = client
    }

    /// Shown in the ticket's history next to the change, so it's traceable (SPEC.md, Boundaries).
    public static let comment = "iTop Wrap"

    public enum Failure: Error, Equatable {
        case emptyEntry
    }

    public static func allowlistKey(for ticket: Ticket) -> String {
        "\(ticket.ticketClass.rawValue)::\(ticket.key)"
    }

    /// Appends an entry to the public or private log. Text format keeps the line breaks.
    public func addLogEntry(to ticket: Ticket, text: String, isPrivate: Bool) async throws {
        let message = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { throw Failure.emptyEntry }
        let field = isPrivate ? "private_log" : "public_log"
        try await update(ticket, fields: [field: .object(["add_item": .object(["message": .string(message), "format": .string("text")])])])
    }

    /// Runs a `core/update` and checks iTop's verdict on the object itself: the call can
    /// succeed while the object was refused.
    func update(_ ticket: Ticket, fields: [String: JSONValue]) async throws {
        let response = try await client.send(.update(className: ticket.ticketClass.rawValue, key: ticket.key,
                                                     fields: fields, comment: Self.comment))
        if let object = response.objects.first, object.code != 0 {
            throw ITopError.server(code: object.code, message: object.message)
        }
    }
}
