//! Locale-dependent formatting, in one place (contract.md open point 5).
//!
//! The Swift app formats with the Mac's current locale through Foundation. The text itself comes
//! from [`Native`]'s `format_*` operations (Foundation's formatters in the app, en_US in tests);
//! this module only adds the rules around them that the Swift call sites apply:
//!
//! - [`Fmt::log_date`] is `LogDate.short` (K/CaseLog.swift, DET-39)
//! - [`Fmt::date_time`] is `date.formatted(date: .abbreviated, time: .shortened)` (LIST-54)
//! - [`Fmt::since`] is `TimerModel.time(_:)`: today the time only, otherwise date and time (TMR-16)
//! - [`Fmt::relative_named`] is `formatted(.relative(presentation: .named))` (MB-16)
//! - [`Fmt::byte_count`] is `ByteCountFormatter` `.file` (LOG-20, DET-33)
//! - [`Fmt::list`] is `ListFormatter.localizedString(byJoining:)` (CMDB-14)

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Offset, Utc};
use itop_kit::LogDate;

use crate::native::Native;

/// Which wall clock "today" and calendar days are counted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// The Mac's time zone (the app, like `Calendar.current`).
    Local,
    /// A fixed offset (tests; `FakeNative` formats at UTC).
    Fixed(FixedOffset),
}

/// The calendar the formatters are used with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fmt {
    pub zone: Zone,
}

impl Fmt {
    /// The Mac's time zone.
    pub fn local() -> Self {
        Fmt { zone: Zone::Local }
    }

    /// UTC, for tests.
    pub fn utc() -> Self {
        Fmt {
            zone: Zone::Fixed(Utc.fix()),
        }
    }

    fn naive(&self, d: DateTime<Utc>) -> NaiveDateTime {
        match self.zone {
            Zone::Local => d.with_timezone(&Local).naive_local(),
            Zone::Fixed(off) => d.with_timezone(&off).naive_local(),
        }
    }

    /// "Today 09:12", "Yesterday 14:20", "Fri 16:10", then the locale's "d MMM" + time, or
    /// "d MMM yyyy" for other years (DET-39).
    pub fn log_date(&self, n: &dyn Native, date: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let format = |template: &str| n.format_date_template(date, template);
        match self.zone {
            Zone::Local => LogDate::short_with(date, now, &Local, format),
            Zone::Fixed(off) => LogDate::short_with(date, now, &off, format),
        }
    }

    /// Abbreviated date and short time (LIST-54).
    pub fn date_time(&self, n: &dyn Native, d: DateTime<Utc>) -> String {
        n.format_date_time(d, true)
    }

    /// `TimerModel.time(_:)`: the time alone when `d` is today, otherwise date and time (TMR-16).
    pub fn since(&self, n: &dyn Native, d: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let today = self.naive(d).date() == self.naive(now).date();
        n.format_date_time(d, !today)
    }

    /// "now", "2 minutes ago", "yesterday", … (MB-16).
    pub fn relative_named(&self, n: &dyn Native, d: DateTime<Utc>, now: DateTime<Utc>) -> String {
        n.format_relative_named(d, now)
    }

    /// A file-style byte count (LOG-20, DET-33).
    pub fn byte_count(&self, n: &dyn Native, bytes: usize) -> String {
        n.format_byte_count(i64::try_from(bytes).unwrap_or(i64::MAX))
    }

    /// "A, B, and C" in the locale (CMDB-14).
    pub fn list(&self, n: &dyn Native, items: &[String]) -> String {
        n.format_list(items)
    }
}

/// Foundation's en_US output, by hand: what `FakeNative` answers so tests are deterministic.
/// Dates are formatted at UTC; times use ICU's narrow no-break space before AM/PM.
pub mod en_us {
    use chrono::{DateTime, Datelike, Timelike, Utc};

    /// The narrow no-break space ICU puts between the time and "AM"/"PM".
    pub const AM_PM_SPACE: char = '\u{202F}';

    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    fn month(d: DateTime<Utc>) -> &'static str {
        MONTHS.get(d.month0() as usize).copied().unwrap_or("")
    }

    /// The templates `LogDate` uses: "HH:mm", "EEE", "d MMM" → "Nov 2", "d MMM yyyy" → "Nov 2, 2026".
    pub fn date_template(d: DateTime<Utc>, template: &str) -> String {
        match template {
            "HH:mm" => d.format("%H:%M").to_string(),
            "EEE" => d.format("%a").to_string(),
            "d MMM" => format!("{} {}", month(d), d.day()),
            _ => format!("{} {}, {:04}", month(d), d.day(), d.year()),
        }
    }

    /// "9:12 AM", or "Nov 16, 2026 at 9:12 AM" with the date.
    pub fn date_time(d: DateTime<Utc>, with_date: bool) -> String {
        let (pm, hour) = d.hour12();
        let time = format!(
            "{hour}:{:02}{AM_PM_SPACE}{}",
            d.minute(),
            if pm { "PM" } else { "AM" }
        );
        if with_date {
            format!("{} {}, {} at {time}", month(d), d.day(), d.year())
        } else {
            time
        }
    }

    /// "now", "45 seconds ago", "2 minutes ago", "3 hours ago", "yesterday", "4 days ago",
    /// "last week", "3 weeks ago", "last month", "last year": the largest non-zero unit.
    pub fn relative_named(d: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let secs = (now - d).num_seconds();
        let past = secs >= 0;
        let s = secs.unsigned_abs();
        let (value, unit, named_one) = if s < 60 {
            (s, "second", None)
        } else if s < 3600 {
            (s / 60, "minute", None)
        } else if s < 86_400 {
            (s / 3600, "hour", None)
        } else if s < 7 * 86_400 {
            let n = if past { "yesterday" } else { "tomorrow" };
            (s / 86_400, "day", Some(n))
        } else if s < 30 * 86_400 {
            let n = if past { "last week" } else { "next week" };
            (s / (7 * 86_400), "week", Some(n))
        } else if s < 365 * 86_400 {
            let n = if past { "last month" } else { "next month" };
            (s / (30 * 86_400), "month", Some(n))
        } else {
            let n = if past { "last year" } else { "next year" };
            (s / (365 * 86_400), "year", Some(n))
        };
        if value == 0 {
            return "now".to_owned();
        }
        if value == 1
            && let Some(named) = named_one
        {
            return named.to_owned();
        }
        let plural = if value == 1 { "" } else { "s" };
        if past {
            format!("{value} {unit}{plural} ago")
        } else {
            format!("in {value} {unit}{plural}")
        }
    }

    /// `.file` style: decimal units, "Zero KB", "1 byte", "512 bytes", whole KB, one decimal for
    /// MB, two for GB and above, trailing zeros dropped.
    pub fn byte_count(bytes: i64) -> String {
        let bytes = bytes.max(0) as u64;
        if bytes == 0 {
            return "Zero KB".to_owned();
        }
        if bytes < 1000 {
            return if bytes == 1 {
                "1 byte".to_owned()
            } else {
                format!("{bytes} bytes")
            };
        }
        let units = [("KB", 0usize), ("MB", 1), ("GB", 2), ("TB", 2), ("PB", 2)];
        let mut value = bytes as f64 / 1000.0;
        let mut i = 0;
        while i + 1 < units.len() {
            let factor = 10f64.powi(units[i].1 as i32);
            if (value * factor).round() / factor < 1000.0 {
                break;
            }
            value /= 1000.0;
            i += 1;
        }
        let (unit, decimals) = units[i];
        let text = format!("{value:.decimals$}");
        let text = if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        };
        format!("{} {unit}", group_thousands(&text))
    }

    /// "1234.5" → "1,234.5".
    fn group_thousands(number: &str) -> String {
        let (int, frac) = number
            .split_once('.')
            .map_or((number, None), |(a, b)| (a, Some(b)));
        let mut out = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        if let Some(f) = frac {
            out.push('.');
            out.push_str(f);
        }
        out
    }

    /// "A", "A and B", "A, B, and C".
    pub fn list(items: &[String]) -> String {
        match items {
            [] => String::new(),
            [one] => one.clone(),
            [a, b] => format!("{a} and {b}"),
            [init @ .., last] => format!("{}, and {last}", init.join(", ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::fake::FakeNative;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .expect("valid date")
    }

    fn native() -> FakeNative {
        FakeNative::new(at(2026, 11, 16, 15, 0))
    }

    #[test]
    fn times_are_12_hour_with_narrow_space() {
        let (f, n) = (Fmt::utc(), native());
        assert_eq!(
            en_us::date_time(at(2026, 11, 16, 9, 12), false),
            "9:12\u{202F}AM"
        );
        assert_eq!(
            en_us::date_time(at(2026, 11, 16, 0, 5), false),
            "12:05\u{202F}AM"
        );
        assert_eq!(
            en_us::date_time(at(2026, 11, 16, 13, 0), false),
            "1:00\u{202F}PM"
        );
        assert_eq!(
            f.date_time(&n, at(2026, 11, 16, 9, 12)),
            "Nov 16, 2026 at 9:12\u{202F}AM"
        );
    }

    #[test]
    fn tmr_16_since_today_is_time_only() {
        let (f, n) = (Fmt::utc(), native());
        let now = at(2026, 11, 16, 15, 0);
        assert_eq!(
            f.since(&n, at(2026, 11, 16, 10, 42), now),
            "10:42\u{202F}AM"
        );
        assert_eq!(
            f.since(&n, at(2026, 11, 15, 10, 42), now),
            "Nov 15, 2026 at 10:42\u{202F}AM"
        );
    }

    #[test]
    fn mb_16_relative_named() {
        let (f, n) = (Fmt::utc(), native());
        let now = at(2026, 11, 16, 15, 0);
        let ago = |d: chrono::Duration| f.relative_named(&n, now - d, now);
        assert_eq!(ago(chrono::Duration::zero()), "now");
        assert_eq!(ago(chrono::Duration::seconds(45)), "45 seconds ago");
        assert_eq!(ago(chrono::Duration::seconds(60)), "1 minute ago");
        assert_eq!(ago(chrono::Duration::minutes(2)), "2 minutes ago");
        assert_eq!(ago(chrono::Duration::hours(3)), "3 hours ago");
        assert_eq!(ago(chrono::Duration::days(1)), "yesterday");
        assert_eq!(ago(chrono::Duration::days(4)), "4 days ago");
    }

    #[test]
    fn byte_counts_like_the_file_style() {
        let (f, n) = (Fmt::utc(), native());
        assert_eq!(f.byte_count(&n, 0), "Zero KB");
        assert_eq!(f.byte_count(&n, 1), "1 byte");
        assert_eq!(f.byte_count(&n, 512), "512 bytes");
        assert_eq!(f.byte_count(&n, 2048), "2 KB");
        assert_eq!(f.byte_count(&n, 1_200_000), "1.2 MB");
        assert_eq!(f.byte_count(&n, 2_000_000), "2 MB");
        assert_eq!(f.byte_count(&n, 999_600), "1 MB");
        assert_eq!(f.byte_count(&n, 3_456_000_000), "3.46 GB");
    }

    #[test]
    fn det_39_log_dates_use_the_template_formatter() {
        let (f, n) = (Fmt::utc(), native());
        let now = at(2026, 11, 16, 15, 0);
        assert_eq!(f.log_date(&n, at(2026, 11, 16, 9, 12), now), "Today 09:12");
        assert_eq!(
            f.log_date(&n, at(2026, 11, 15, 23, 59), now),
            "Yesterday 23:59"
        );
        assert_eq!(f.log_date(&n, at(2026, 11, 13, 16, 10), now), "Fri 16:10");
        assert_eq!(f.log_date(&n, at(2026, 11, 2, 9, 0), now), "Nov 2 09:00");
        assert_eq!(f.log_date(&n, at(2025, 12, 24, 10, 0), now), "Dec 24, 2025");
    }

    #[test]
    fn cmdb_14_list_join() {
        let (f, n) = (Fmt::utc(), native());
        let s = |v: &[&str]| f.list(&n, &v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&["a"]), "a");
        assert_eq!(s(&["a", "b"]), "a and b");
        assert_eq!(s(&["a", "b", "c"]), "a, b, and c");
    }

    /// Prints what the real Mac formatters give in the current locale, to compare with the
    /// Swift app's output (`swift locale.swift`, see the parity report). Same sample instants.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "prints the Mac's locale output; run by hand"]
    fn mac_locale_parity() {
        use crate::native::mac::locale as mac;
        let now = Utc.timestamp_opt(1_794_830_400, 0).single().expect("now");
        let f = Fmt::local();
        let samples = [3 * 3600, 26 * 3600, 4 * 86_400, 40 * 86_400, 400 * 86_400];
        let fmt_log =
            |d: DateTime<Utc>| LogDate::short_with(d, now, &Local, |t| mac::date_template(d, t));
        for back in samples {
            let d = now - chrono::Duration::seconds(back);
            let ts = d.timestamp();
            let today = f.naive(d).date() == f.naive(now).date();
            println!("log[{ts}]={}", fmt_log(d));
            println!("opened[{ts}]=Opened {}", mac::date_time(d, true));
            println!("since[{ts}]={}", mac::date_time(d, !today));
        }
        let real_now = Utc::now();
        for secs in [
            0, 45, 60, 89, 90, 150, 5400, 10800, 82800, 93600, 172800, 345600, 518400, 604800,
            864000, 1209600, 2592000, 3456000, 5184000, 31536000, 34560000, 63072000,
        ] {
            let d = real_now - chrono::Duration::seconds(secs);
            println!(
                "relative[{secs}]=Updated {}",
                mac::relative_named(d, real_now)
            );
        }
        for b in [0i64, 1, 512, 2048, 999_600, 1_200_000, 3_456_000_000] {
            println!("bytes[{b}]={}", mac::byte_count(b));
        }
        for l in [vec!["vpn-gw-01"], vec!["a", "b"], vec!["a", "b", "c"]] {
            let items: Vec<String> = l.iter().map(|s| s.to_string()).collect();
            println!("list[{}]={}", l.len(), mac::list(&items));
        }
    }
}
