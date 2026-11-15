//! Locale-dependent formatting, in one place so it can be swapped (contract.md open point 5).
//!
//! The Swift app formats with the Mac's locale through Foundation. This module reproduces
//! Foundation's en_US output by hand:
//!
//! - [`Fmt::date_time`]: `date.formatted(date: .abbreviated, time: .shortened)` → "Nov 16, 2026 at 9:12 AM"
//! - [`Fmt::time_short`]: `.formatted(date: .omitted, time: .shortened)` → "9:12 AM"
//! - [`Fmt::since`]: `TimerModel.time(_:)`: today the time only, otherwise date and time
//! - [`Fmt::relative_named`]: `formatted(.relative(presentation: .named))` → "2 minutes ago", "now"
//! - [`byte_count`]: `ByteCountFormatter.string(fromByteCount:countStyle: .file)` → "2 KB", "1.2 MB"
//!
//! Log entry dates (`LogDate.short`, 24-hour) come from `itop_kit::LogDate` with
//! `DateLocale::EnUs`. Times use Foundation's narrow no-break space before AM/PM (ICU 72+).

use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDateTime, Offset, Timelike, Utc};
use itop_kit::{DateLocale, LogDate};

/// The narrow no-break space ICU puts between the time and "AM"/"PM".
pub const AM_PM_SPACE: char = '\u{202F}';

/// Which wall clock dates are shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// The Mac's time zone (the app).
    Local,
    /// A fixed offset (tests).
    Fixed(FixedOffset),
}

/// The formatters, for one locale and time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fmt {
    pub zone: Zone,
    pub log_locale: DateLocale,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl Fmt {
    /// en_US in the Mac's time zone.
    pub fn local() -> Self {
        Fmt {
            zone: Zone::Local,
            log_locale: DateLocale::EnUs,
        }
    }

    /// en_US at UTC, for tests.
    pub fn utc() -> Self {
        Fmt {
            zone: Zone::Fixed(Utc.fix()),
            log_locale: DateLocale::EnUs,
        }
    }

    fn naive(&self, d: DateTime<Utc>) -> NaiveDateTime {
        match self.zone {
            Zone::Local => d.with_timezone(&Local).naive_local(),
            Zone::Fixed(off) => d.with_timezone(&off).naive_local(),
        }
    }

    /// "Today 09:12", "Yesterday 14:20", "Fri 16:10", "Nov 2 09:00", "Dec 24, 2025" (DET-39).
    pub fn log_date(&self, date: DateTime<Utc>, now: DateTime<Utc>) -> String {
        match self.zone {
            Zone::Local => LogDate::short(date, now, &Local, self.log_locale),
            Zone::Fixed(off) => LogDate::short(date, now, &off, self.log_locale),
        }
    }

    /// "9:12 AM".
    pub fn time_short(&self, d: DateTime<Utc>) -> String {
        let n = self.naive(d);
        let (pm, hour) = n.hour12();
        format!(
            "{hour}:{:02}{AM_PM_SPACE}{}",
            n.minute(),
            if pm { "PM" } else { "AM" }
        )
    }

    /// "Nov 16, 2026".
    pub fn date_abbreviated(&self, d: DateTime<Utc>) -> String {
        let n = self.naive(d);
        let month = MONTHS.get(n.month0() as usize).copied().unwrap_or("");
        format!("{month} {}, {}", n.day(), n.year())
    }

    /// "Nov 16, 2026 at 9:12 AM".
    pub fn date_time(&self, d: DateTime<Utc>) -> String {
        format!("{} at {}", self.date_abbreviated(d), self.time_short(d))
    }

    /// `TimerModel.time(_:)`: the time alone when `d` is today, otherwise date and time (TMR-16).
    pub fn since(&self, d: DateTime<Utc>, now: DateTime<Utc>) -> String {
        if self.naive(d).date() == self.naive(now).date() {
            self.time_short(d)
        } else {
            self.date_time(d)
        }
    }

    /// "now", "45 seconds ago", "2 minutes ago", "3 hours ago", "yesterday", "4 days ago",
    /// "last week", "3 weeks ago", "last month", "last year" (MB-16). The largest non-zero
    /// calendar unit, truncated.
    pub fn relative_named(&self, d: DateTime<Utc>, now: DateTime<Utc>) -> String {
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
            (
                s / 86_400,
                "day",
                Some(if past { "yesterday" } else { "tomorrow" }),
            )
        } else if s < 30 * 86_400 {
            (
                s / (7 * 86_400),
                "week",
                Some(if past { "last week" } else { "next week" }),
            )
        } else if s < 365 * 86_400 {
            (
                s / (30 * 86_400),
                "month",
                Some(if past { "last month" } else { "next month" }),
            )
        } else {
            (
                s / (365 * 86_400),
                "year",
                Some(if past { "last year" } else { "next year" }),
            )
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
}

/// `ByteCountFormatter` with `.file` style in en_US: decimal units, "Zero KB", "1 byte",
/// "512 bytes", whole KB, one decimal for MB, two for GB and above, trailing zeros dropped.
pub fn byte_count(bytes: u64) -> String {
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
        let decimals = units[i].1;
        let factor = 10f64.powi(decimals as i32);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .expect("valid date")
    }

    #[test]
    fn times_are_12_hour_with_narrow_space() {
        let f = Fmt::utc();
        assert_eq!(f.time_short(at(2026, 11, 16, 9, 12)), "9:12\u{202F}AM");
        assert_eq!(f.time_short(at(2026, 11, 16, 0, 5)), "12:05\u{202F}AM");
        assert_eq!(f.time_short(at(2026, 11, 16, 13, 0)), "1:00\u{202F}PM");
        assert_eq!(
            f.date_time(at(2026, 11, 16, 9, 12)),
            "Nov 16, 2026 at 9:12\u{202F}AM"
        );
    }

    #[test]
    fn tmr_16_since_today_is_time_only() {
        let f = Fmt::utc();
        let now = at(2026, 11, 16, 15, 0);
        assert_eq!(f.since(at(2026, 11, 16, 10, 42), now), "10:42\u{202F}AM");
        assert_eq!(
            f.since(at(2026, 11, 15, 10, 42), now),
            "Nov 15, 2026 at 10:42\u{202F}AM"
        );
    }

    #[test]
    fn mb_16_relative_named() {
        let f = Fmt::utc();
        let now = at(2026, 11, 16, 15, 0);
        assert_eq!(f.relative_named(now, now), "now");
        assert_eq!(
            f.relative_named(now - chrono::Duration::seconds(45), now),
            "45 seconds ago"
        );
        assert_eq!(
            f.relative_named(now - chrono::Duration::seconds(60), now),
            "1 minute ago"
        );
        assert_eq!(
            f.relative_named(now - chrono::Duration::minutes(2), now),
            "2 minutes ago"
        );
        assert_eq!(
            f.relative_named(now - chrono::Duration::hours(3), now),
            "3 hours ago"
        );
        assert_eq!(
            f.relative_named(now - chrono::Duration::days(1), now),
            "yesterday"
        );
        assert_eq!(
            f.relative_named(now - chrono::Duration::days(4), now),
            "4 days ago"
        );
    }

    #[test]
    fn byte_counts_like_the_file_style() {
        assert_eq!(byte_count(0), "Zero KB");
        assert_eq!(byte_count(1), "1 byte");
        assert_eq!(byte_count(512), "512 bytes");
        assert_eq!(byte_count(2048), "2 KB");
        assert_eq!(byte_count(1_200_000), "1.2 MB");
        assert_eq!(byte_count(2_000_000), "2 MB");
        assert_eq!(byte_count(999_600), "1 MB");
        assert_eq!(byte_count(3_456_000_000), "3.46 GB");
    }

    #[test]
    fn det_39_log_dates_are_us_24_hour() {
        let f = Fmt::utc();
        let now = at(2026, 11, 16, 15, 0);
        assert_eq!(f.log_date(at(2026, 11, 16, 9, 12), now), "Today 09:12");
        assert_eq!(f.log_date(at(2026, 11, 2, 9, 0), now), "Nov 2 09:00");
    }
}
