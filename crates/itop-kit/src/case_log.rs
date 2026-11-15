//! Port of `CaseLog.swift`: `LogEntry`, `CaseLog`, `HTMLText` and `LogDate`.
//!
//! The Swift regexes run on `Character`s (grapheme clusters); the `regex` crate runs on code
//! points. They agree on everything iTop sends except contrived input such as a combining mark
//! right after `>` (one Character in Swift, so `[^>]` would swallow it) or `\r\n` line ends
//! (one Character in Swift). `\b` is Unicode-aware in both, but Swift uses the UAX #29 word
//! rules, under which `<script.x` has no boundary after "script"; such tags don't occur.

use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::is_swift_whitespace;
use crate::json::JsonValue;
use crate::ticket::Ticket;
use crate::ticket_queries::distant_past;

/// A pattern compiled once from a constant. The patterns are fixed and every one runs in the
/// tests, so compiling can't fail at run time; `None` would only mean "match nothing".
macro_rules! regex {
    ($re:expr) => {{
        static CELL: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
        CELL.get_or_init(|| regex::Regex::new($re).ok()).as_ref()
    }};
}
pub(crate) use regex;

/// Replaces every match with literal text (no `$` expansion), like Swift's `replace(_:with:)`.
pub(crate) fn replace_all(re: Option<&regex::Regex>, s: &str, with: &str) -> String {
    match re {
        Some(re) => re.replace_all(s, regex::NoExpand(with)).into_owned(),
        None => s.to_string(),
    }
}

/// Foundation's `CharacterSet.whitespacesAndNewlines`: Unicode Z* categories, U+0009–U+000D and
/// U+0085.
pub(crate) fn is_swift_whitespace_or_newline(c: char) -> bool {
    is_swift_whitespace(c)
        || matches!(
            c,
            '\n' | '\u{0B}' | '\u{0C}' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
}

/// One entry of a ticket's public or private log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct LogEntry {
    #[ts(type = "number")]
    pub id: i64,
    pub date: Option<DateTime<Utc>>,
    pub author: String,
    pub text: String,
    /// The entry's HTML when iTop sent one, for showing links and images (`RichText`).
    pub html: Option<String>,
}

impl LogEntry {
    pub fn new(
        id: i64,
        date: Option<DateTime<Utc>>,
        author: impl Into<String>,
        text: impl Into<String>,
        html: Option<String>,
    ) -> Self {
        LogEntry {
            id,
            date,
            author: author.into(),
            text: text.into(),
            html,
        }
    }
}

/// iTop's case log, as REST returns it: `{"entries":[{"date","user_login","message","message_html"}]}`.
pub struct CaseLog;

impl CaseLog {
    /// Entries oldest first. iTop lists the newest first, so entries written in the same
    /// second are ordered by their reversed position.
    pub fn entries(value: Option<&JsonValue>) -> Vec<LogEntry> {
        let Some(JsonValue::Array(raw)) = value.and_then(|v| v.get("entries")) else {
            return Vec::new();
        };
        let mut parsed: Vec<(usize, LogEntry)> = raw
            .iter()
            .enumerate()
            .filter_map(|(index, e)| {
                let html = e.get("message_html").and_then(JsonValue::string_value);
                let message = e.get("message").and_then(JsonValue::string_value);
                let text = match (html, message) {
                    (Some(h), _) => HtmlText::plain(h),
                    (None, Some(m)) => m.trim_matches(is_swift_whitespace_or_newline).to_string(),
                    (None, None) => return None,
                };
                Some((
                    index,
                    LogEntry {
                        id: index as i64,
                        date: e
                            .get("date")
                            .and_then(JsonValue::string_value)
                            .and_then(Ticket::parse_date),
                        author: e
                            .get("user_login")
                            .and_then(JsonValue::string_value)
                            .unwrap_or("")
                            .to_string(),
                        text,
                        html: html.map(str::to_string),
                    },
                ))
            })
            .collect();
        let past = distant_past();
        parsed.sort_by(|a, b| {
            let da = a.1.date.unwrap_or(past);
            let db = b.1.date.unwrap_or(past);
            da.cmp(&db).then_with(|| b.0.cmp(&a.0))
        });
        parsed.into_iter().map(|(_, e)| e).collect()
    }
}

/// HTML from iTop (descriptions, log entries) to plain text with its line breaks.
///
/// Deliberately not a web engine: that would fetch images and other resources referenced in the
/// HTML. Images become "[Image]"; links keep their text.
pub struct HtmlText;

/// `<(script|style)\b.*?<\/\1\s*>` in Swift. The `regex` crate has no backreferences; as a
/// tag can only start one of the two names, one alternative per name matches the same text.
pub(crate) const SCRIPT_STYLE: &str = r"(?is)<script\b.*?</script\s*>|<style\b.*?</style\s*>";

impl HtmlText {
    pub fn plain(html: &str) -> String {
        let mut s = replace_all(regex!(SCRIPT_STYLE), html, "");
        s = replace_all(regex!(r"(?i)<img\b[^>]*>"), &s, "[Image]");
        s = replace_all(regex!(r"(?i)<br\s*/?>"), &s, "\n");
        s = replace_all(regex!(r"(?i)<li\b[^>]*>"), &s, "\n• ");
        s = replace_all(
            regex!(r"(?i)</?(p|h[1-6]|blockquote|figure|ul|ol|table|pre)\b[^>]*>"),
            &s,
            "\n\n",
        );
        s = replace_all(
            regex!(r"(?i)</?(div|tr|section|article|header|footer)\b[^>]*>"),
            &s,
            "\n",
        );
        s = replace_all(regex!(r"(?i)</?(td|th)\b[^>]*>"), &s, " ");
        s = replace_all(regex!(r"<[^>]*>"), &s, "");
        Self::normalize(&Self::decode_entities(&s))
    }

    /// Whitespace clean-up shared with `RichText`, so both read the same: non-breaking spaces
    /// become spaces, lines are trimmed, at most one blank line in a row, no outer whitespace.
    pub(crate) fn normalize(text: &str) -> String {
        let s = text.replace('\u{00A0}', " ");
        // `components(separatedBy: "\n")` then `[ \t]+$` and `^[ \t]+` on each line.
        let lines: Vec<&str> = s
            .split('\n')
            .map(|line| {
                line.trim_end_matches([' ', '\t'])
                    .trim_start_matches([' ', '\t'])
            })
            .collect();
        let joined = lines.join("\n");
        let collapsed = replace_all(regex!(r"\n{3,}"), &joined, "\n\n");
        collapsed
            .trim_matches(is_swift_whitespace_or_newline)
            .to_string()
    }

    /// Numeric (`&#39;`, `&#x41;`) and a fixed list of named entities; anything else stays as is.
    pub(crate) fn decode_entities(s: &str) -> String {
        let Some(re) = regex!(r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);") else {
            return s.to_string();
        };
        re.replace_all(s, |caps: &regex::Captures| {
            let whole = caps.get(0).map_or("", |m| m.as_str());
            let name = caps.get(1).map_or("", |m| m.as_str());
            if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X"))
                && let Ok(n) = u32::from_str_radix(hex, 16)
                && let Some(c) = char::from_u32(n)
            {
                return c.to_string();
            }
            // `UInt32(_:)` takes an optional sign; the pattern only lets digits through.
            if let Some(dec) = name.strip_prefix('#')
                && let Ok(n) = dec.parse::<u32>()
                && let Some(c) = char::from_u32(n)
            {
                return c.to_string();
            }
            named_entity(name).map_or_else(|| whole.to_string(), str::to_string)
        })
        .into_owned()
    }
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        // A plain space in the Swift table (a literal U+00A0 is turned into one by `normalize`).
        "nbsp" => " ",
        "eacute" => "é",
        "egrave" => "è",
        "ecirc" => "ê",
        "euml" => "ë",
        "agrave" => "à",
        "aacute" => "á",
        "acirc" => "â",
        "auml" => "ä",
        "ccedil" => "ç",
        "iuml" => "ï",
        "icirc" => "î",
        "ocirc" => "ô",
        "ouml" => "ö",
        "ugrave" => "ù",
        "uuml" => "ü",
        "ucirc" => "û",
        "Eacute" => "É",
        "Agrave" => "À",
        "szlig" => "ß",
        "ntilde" => "ñ",
        "euro" => "€",
        "copy" => "©",
        "reg" => "®",
        "deg" => "°",
        "times" => "×",
        "hellip" => "…",
        "ndash" => "–",
        "mdash" => "—",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "laquo" => "«",
        "raquo" => "»",
        _ => return None,
    })
}

/// The locale log dates are written in (Swift reads it from the `Calendar`). English only: the
/// app's text is English, and these are the two English layouts the formats differ in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum DateLocale {
    /// en_US: "Nov 2 09:00", "Dec 24, 2025".
    #[default]
    EnUs,
    /// en_GB: "2 Nov 09:00", "24 Dec 2025" (and "Sept").
    EnGb,
}

/// Log entry dates as the wireframe shows them: "Today 09:12", "Yesterday 14:20", "Fri 16:10",
/// "2 Nov 09:00", and "24 Dec 2025" for other years.
pub struct LogDate;

impl LogDate {
    /// `zone` is the calendar's time zone (`&chrono::Local` in the app).
    ///
    /// Output is what `DateFormatter.setLocalizedDateFormatFromTemplate` gives for the
    /// templates "HH:mm" (forced to 24-hour), "EEE", "d MMM" and "d MMM yyyy" in the locale.
    pub fn short<Tz: TimeZone>(
        date: DateTime<Utc>,
        now: DateTime<Utc>,
        zone: &Tz,
        locale: DateLocale,
    ) -> String {
        let d = date.with_timezone(zone).naive_local();
        let n = now.with_timezone(zone).naive_local();
        // Days between the two local midnights, counted in calendar days like `Calendar` does.
        let days = (n.date() - d.date()).num_days();
        let time = d.format("%H:%M").to_string();
        match days {
            0 => format!("Today {time}"),
            1 => format!("Yesterday {time}"),
            2..=6 => format!("{} {time}", d.format("%a")),
            _ => {
                let month = month_abbreviation(d.month(), locale);
                let day = d.day();
                let same_year = d.year() == n.year();
                match (same_year, locale) {
                    (true, DateLocale::EnUs) => format!("{month} {day} {time}"),
                    (true, DateLocale::EnGb) => format!("{day} {month} {time}"),
                    (false, DateLocale::EnUs) => format!("{month} {day}, {:04}", d.year()),
                    (false, DateLocale::EnGb) => format!("{day} {month} {:04}", d.year()),
                }
            }
        }
    }
}

/// CLDR's abbreviated month names: en_GB writes September "Sept" (CLDR 38 and later, which every
/// supported macOS ships).
fn month_abbreviation(month: u32, locale: DateLocale) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if month == 9 && locale == DateLocale::EnGb {
        return "Sept";
    }
    NAMES
        .get((month as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDateTime};

    fn at(text: &str, zone: &FixedOffset) -> DateTime<Utc> {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
            .unwrap()
            .and_local_timezone(*zone)
            .unwrap()
            .with_timezone(&Utc)
    }

    /// en_US orders month before day ("MMM d", "MMM d, y"); en_GB writes September "Sept".
    #[test]
    fn locales() {
        let zone = FixedOffset::east_opt(0).unwrap();
        let now = at("2026-11-10 12:00", &zone);
        let short = |t: &str, l| LogDate::short(at(t, &zone), now, &zone, l);
        assert_eq!(short("2026-11-02 09:00", DateLocale::EnUs), "Nov 2 09:00");
        assert_eq!(short("2025-12-24 10:00", DateLocale::EnUs), "Dec 24, 2025");
        assert_eq!(short("2026-09-03 08:05", DateLocale::EnGb), "3 Sept 08:05");
        assert_eq!(short("2026-11-08 23:59", DateLocale::EnUs), "Sun 23:59");
        // A date after now isn't "Today": it falls through to the date.
        assert_eq!(short("2026-11-12 09:00", DateLocale::EnUs), "Nov 12 09:00");
    }

    /// The backreference-free script/style pattern still pairs each tag with its own end tag.
    #[test]
    fn script_and_style_pairs() {
        assert_eq!(
            HtmlText::plain("<SCRIPT>a</style>b</script>c<style>d</STYLE >e"),
            "ce"
        );
    }

    #[test]
    fn entities_left_alone() {
        assert_eq!(
            HtmlText::decode_entities("&bogus; &#xD800; &#99999999999;"),
            "&bogus; &#xD800; &#99999999999;"
        );
        assert_eq!(HtmlText::decode_entities("&nbsp;&#65;&#x42;"), " AB");
    }
}
