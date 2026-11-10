import Foundation

/// One entry of a ticket's public or private log.
public struct LogEntry: Sendable, Equatable, Identifiable {
    public let id: Int
    public let date: Date?
    public let author: String
    public let text: String

    public init(id: Int, date: Date?, author: String, text: String) {
        self.id = id
        self.date = date
        self.author = author
        self.text = text
    }
}

/// iTop's case log, as REST returns it: `{"entries":[{"date","user_login","message","message_html"}]}`.
public enum CaseLog {
    /// Entries oldest first. iTop lists the newest first, so entries written in the same
    /// second are ordered by their reversed position.
    public static func entries(from value: JSONValue?) -> [LogEntry] {
        guard case .array(let raw)? = value?["entries"] else { return [] }
        let parsed: [(Int, LogEntry)] = raw.enumerated().compactMap { index, e in
            let html = e["message_html"]?.stringValue
            let message = e["message"]?.stringValue
            guard html != nil || message != nil else { return nil }
            let text = html.map(HTMLText.plain) ?? message!.trimmingCharacters(in: .whitespacesAndNewlines)
            return (index, LogEntry(id: index, date: e["date"]?.stringValue.flatMap(Ticket.parseDate),
                                    author: e["user_login"]?.stringValue ?? "", text: text))
        }
        return parsed.sorted { a, b in
            let da = a.1.date ?? .distantPast, db = b.1.date ?? .distantPast
            return da != db ? da < db : a.0 > b.0
        }.map(\.1)
    }
}

/// HTML from iTop (descriptions, log entries) to plain text with its line breaks.
///
/// Deliberately not `NSAttributedString(html:)`: that runs WebKit, which fetches images and
/// other resources referenced in the HTML. Images become "[Image]"; links keep their text.
public enum HTMLText {
    public static func plain(_ html: String) -> String {
        var s = html
        s.replace(/(?is)<(script|style)\b.*?<\/\1\s*>/, with: "")
        s.replace(/(?i)<img\b[^>]*>/, with: "[Image]")
        s.replace(/(?i)<br\s*\/?>/, with: "\n")
        s.replace(/(?i)<li\b[^>]*>/, with: "\n• ")
        s.replace(/(?i)<\/?(p|h[1-6]|blockquote|figure|ul|ol|table|pre)\b[^>]*>/, with: "\n\n")
        s.replace(/(?i)<\/?(div|tr|section|article|header|footer)\b[^>]*>/, with: "\n")
        s.replace(/(?i)<\/?(td|th)\b[^>]*>/, with: " ")
        s.replace(/<[^>]*>/, with: "")
        s = decodeEntities(s)
        s = s.replacingOccurrences(of: "\u{00A0}", with: " ")
        let lines = s.components(separatedBy: "\n").map { line in
            line.replacing(/[ \t]+$/, with: "").replacing(/^[ \t]+/, with: "")
        }
        s = lines.joined(separator: "\n")
        s.replace(/\n{3,}/, with: "\n\n")
        return s.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func decodeEntities(_ s: String) -> String {
        s.replacing(/&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);/) { match in
            let name = String(match.output.1)
            if name.hasPrefix("#x") || name.hasPrefix("#X"), let n = UInt32(name.dropFirst(2), radix: 16), let u = Unicode.Scalar(n) {
                return String(Character(u))
            }
            if name.hasPrefix("#"), let n = UInt32(name.dropFirst()), let u = Unicode.Scalar(n) {
                return String(Character(u))
            }
            return named[name] ?? String(match.output.0)
        }
    }

    static let named: [String: String] = [
        "amp": "&", "lt": "<", "gt": ">", "quot": "\"", "apos": "'", "nbsp": " ",
        "eacute": "é", "egrave": "è", "ecirc": "ê", "euml": "ë", "agrave": "à", "aacute": "á", "acirc": "â",
        "auml": "ä", "ccedil": "ç", "iuml": "ï", "icirc": "î", "ocirc": "ô", "ouml": "ö", "ugrave": "ù",
        "uuml": "ü", "ucirc": "û", "Eacute": "É", "Agrave": "À", "szlig": "ß", "ntilde": "ñ",
        "euro": "€", "copy": "©", "reg": "®", "deg": "°", "times": "×", "hellip": "…",
        "ndash": "–", "mdash": "—", "lsquo": "‘", "rsquo": "’", "ldquo": "“", "rdquo": "”", "laquo": "«", "raquo": "»",
    ]
}

/// Log entry dates as the wireframe shows them: "Today 09:12", "Yesterday 14:20", "Fri 16:10",
/// "2 Nov 09:00", and "24 Dec 2025" for other years.
public enum LogDate {
    public static func short(_ date: Date, now: Date = Date(), calendar: Calendar = .current) -> String {
        let days = calendar.dateComponents([.day], from: calendar.startOfDay(for: date), to: calendar.startOfDay(for: now)).day ?? 99
        let time = format("HH:mm", date, calendar)
        switch days {
        case 0: return "Today \(time)"
        case 1: return "Yesterday \(time)"
        case 2..<7: return "\(format("EEE", date, calendar)) \(time)"
        default:
            let sameYear = calendar.component(.year, from: date) == calendar.component(.year, from: now)
            return sameYear ? "\(format("d MMM", date, calendar)) \(time)" : format("d MMM yyyy", date, calendar)
        }
    }

    private static func format(_ pattern: String, _ date: Date, _ calendar: Calendar) -> String {
        let f = DateFormatter()
        f.calendar = calendar
        f.timeZone = calendar.timeZone
        f.locale = calendar.locale ?? .current
        f.setLocalizedDateFormatFromTemplate(pattern)
        if pattern == "HH:mm" { f.dateFormat = "HH:mm" }
        return f.string(from: date)
    }
}
