import Foundation

public enum OQL {
    /// A string literal for an OQL query: backslashes and single quotes are escaped.
    public static func quote(_ value: String) -> String {
        "'" + value.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "'", with: "\\'") + "'"
    }
}
