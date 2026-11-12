import Foundation

/// One iTop REST operation, encoded as the `json_data` form field.
public enum RESTOperation: Sendable, Equatable {
    case listOperations
    case checkCredentials(user: String, password: String)
    /// `key` is an OQL query, an object id, or a search criteria string.
    case get(className: String, key: String, outputFields: [String], limit: Int? = nil)
    /// Objects linked through a relation, e.g. "impacts" up (what this depends on) or down (what uses it).
    case getRelated(className: String, key: String, relation: String, depth: Int, direction: String)
    /// `outputFields` are the fields iTop sends back for the new object (an InlineImage's `secret`).
    case create(className: String, fields: [String: JSONValue], comment: String, outputFields: [String] = ["id"])
    case update(className: String, key: String, fields: [String: JSONValue], comment: String)
    case applyStimulus(className: String, key: String, stimulus: String, fields: [String: JSONValue], comment: String)

    /// Writes are refused by `ITopClient` unless it was created with `allowsWrites: true`.
    public var isWrite: Bool {
        switch self {
        case .listOperations, .checkCredentials, .get, .getRelated: return false
        case .create, .update, .applyStimulus: return true
        }
    }

    public var name: String {
        switch self {
        case .listOperations: return "list_operations"
        case .checkCredentials: return "core/check_credentials"
        case .get: return "core/get"
        case .getRelated: return "core/get_related"
        case .create: return "core/create"
        case .update: return "core/update"
        case .applyStimulus: return "core/apply_stimulus"
        }
    }

    var json: JSONValue {
        var o: [String: JSONValue] = ["operation": .string(name)]
        switch self {
        case .listOperations:
            break
        case .checkCredentials(let user, let password):
            o["user"] = .string(user)
            o["password"] = .string(password)
        case .get(let className, let key, let outputFields, let limit):
            o["class"] = .string(className)
            o["key"] = .string(key)
            o["output_fields"] = .string(outputFields.joined(separator: ","))
            if let limit { o["limit"] = .string(String(limit)) }
        case .getRelated(let className, let key, let relation, let depth, let direction):
            o["class"] = .string(className)
            o["key"] = .string(key)
            o["relation"] = .string(relation)
            o["depth"] = .number(Double(depth))
            o["direction"] = .string(direction)
            o["redundancy"] = .bool(false)
        case .create(let className, let fields, let comment, let outputFields):
            o["class"] = .string(className)
            o["fields"] = .object(fields)
            o["comment"] = .string(comment)
            o["output_fields"] = .string(outputFields.joined(separator: ","))
        case .update(let className, let key, let fields, let comment):
            o["class"] = .string(className)
            o["key"] = .string(key)
            o["fields"] = .object(fields)
            o["comment"] = .string(comment)
            o["output_fields"] = .string("id")
        case .applyStimulus(let className, let key, let stimulus, let fields, let comment):
            o["class"] = .string(className)
            o["key"] = .string(key)
            o["stimulus"] = .string(stimulus)
            o["fields"] = .object(fields)
            o["comment"] = .string(comment)
            o["output_fields"] = .string("id")
        }
        return .object(o)
    }
}
