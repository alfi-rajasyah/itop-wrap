//! Locale-dependent text from the same Foundation formatters the Swift app uses, with the Mac's
//! current locale and time zone (contract.md open point 5). Foundation's formatters are
//! thread-safe, so these run on any thread.

use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2_foundation::{
    NSArray, NSByteCountFormatter, NSByteCountFormatterCountStyle, NSCalendar, NSCalendarOptions,
    NSCalendarUnit, NSDate, NSDateComponentUndefined, NSDateComponents, NSDateFormatter,
    NSDateFormatterStyle, NSListFormatter, NSLocale, NSNotFound, NSRelativeDateTimeFormatter,
    NSRelativeDateTimeFormatterStyle, NSRelativeDateTimeFormatterUnitsStyle, NSString,
};

fn ns_date(d: DateTime<Utc>) -> Retained<NSDate> {
    let secs = d.timestamp() as f64 + f64::from(d.timestamp_subsec_millis()) / 1000.0;
    NSDate::dateWithTimeIntervalSince1970(secs)
}

/// `LogDate.format(_:_:_:)` (K/CaseLog.swift): `setLocalizedDateFormatFromTemplate`, except
/// "HH:mm", which is used as the fixed format.
pub fn date_template(d: DateTime<Utc>, template: &str) -> String {
    let f = NSDateFormatter::new();
    f.setLocale(Some(&NSLocale::currentLocale()));
    let pattern = NSString::from_str(template);
    f.setLocalizedDateFormatFromTemplate(&pattern);
    if template == "HH:mm" {
        f.setDateFormat(Some(&pattern));
    }
    f.stringFromDate(&ns_date(d)).to_string()
}

/// `date.formatted(date: .abbreviated | .omitted, time: .shortened)`.
pub fn date_time(d: DateTime<Utc>, with_date: bool) -> String {
    let f = NSDateFormatter::new();
    f.setLocale(Some(&NSLocale::currentLocale()));
    f.setDateStyle(if with_date {
        NSDateFormatterStyle::MediumStyle
    } else {
        NSDateFormatterStyle::NoStyle
    });
    f.setTimeStyle(NSDateFormatterStyle::ShortStyle);
    f.stringFromDate(&ns_date(d)).to_string()
}

/// The units `Date.RelativeFormatStyle` picks from, largest first
/// (`ICURelativeDateFormatter.sortedAllowedComponents`).
const UNITS: [NSCalendarUnit; 7] = [
    NSCalendarUnit::Year,
    NSCalendarUnit::Month,
    NSCalendarUnit::WeekOfMonth,
    NSCalendarUnit::Day,
    NSCalendarUnit::Hour,
    NSCalendarUnit::Minute,
    NSCalendarUnit::Second,
];

fn all_units() -> NSCalendarUnit {
    UNITS.iter().fold(NSCalendarUnit(0), |a, &u| a | u)
}

fn first_non_zero(
    c: &NSDateComponents,
    units: &[NSCalendarUnit],
) -> Option<(NSCalendarUnit, isize)> {
    units.iter().find_map(|&u| {
        let v = c.valueForComponent(u);
        (v != 0 && v != NSDateComponentUndefined).then_some((u, v))
    })
}

fn value(c: &NSDateComponents, u: NSCalendarUnit) -> isize {
    let v = c.valueForComponent(u);
    if v == NSDateComponentUndefined { 0 } else { v }
}

/// `_alignedComponentValue` (swift-foundation, Date+RelativeFormatStyle.swift).
fn aligned(
    cal: &NSCalendar,
    unit: NSCalendarUnit,
    dest: &NSDate,
    reference: &NSDate,
) -> Option<(NSCalendarUnit, isize)> {
    let mut start: Option<Retained<NSDate>> = None;
    let mut interval: f64 = 0.0;
    // SAFETY: valid out-pointers for the start date and the interval.
    let ok = unsafe {
        cal.rangeOfUnit_startDate_interval_forDate(unit, Some(&mut start), &mut interval, reference)
    };
    let start = start.filter(|_| ok)?;
    let from = if reference.timeIntervalSinceDate(dest) < 0.0 {
        start
    } else {
        start.dateByAddingTimeInterval(interval - 1.0)
    };
    let c = cal.components_fromDate_toDate_options(all_units(), &from, dest, NSCalendarOptions(0));
    first_non_zero(&c, &UNITS)
}

/// `_roundedLargestComponentValue` (swift-foundation, Date+RelativeFormatStyle.swift).
fn rounded(
    cal: &NSCalendar,
    dest: &NSDate,
    reference: &NSDate,
    largest_allowed: NSCalendarUnit,
) -> Option<(NSCalendarUnit, isize)> {
    let at = UNITS.iter().position(|&u| u == largest_allowed)?;
    let units = &UNITS[at..];
    let flags = units.iter().fold(NSCalendarUnit::Nanosecond, |a, &u| a | u);
    let c = cal.components_fromDate_toDate_options(flags, reference, dest, NSCalendarOptions(0));
    let nanos = value(&c, NSCalendarUnit::Nanosecond);
    let mut values: Vec<isize> = units.iter().map(|&u| value(&c, u)).collect();
    if let Some(sec) = values.last_mut()
        && nanos.abs() >= 500_000_000
    {
        *sec += nanos.signum();
    }
    // The largest non-zero unit, else the smallest (seconds).
    let li = values
        .iter()
        .position(|&v| v != 0)
        .unwrap_or(units.len() - 1);
    let largest = units[li];
    let (second_unit, second_value) = match units.get(li + 1) {
        Some(&u) => (u, values[li + 1]),
        None => (NSCalendarUnit::Nanosecond, nanos),
    };
    let mut rounded_value = values[li];
    let range = cal.rangeOfUnit_inUnit_forDate(second_unit, largest, dest);
    if range.location != NSNotFound as usize && second_value.unsigned_abs() * 2 >= range.length {
        rounded_value += if second_value > 0 { 1 } else { -1 };
    }
    let shifted = cal.dateByAddingUnit_value_toDate_options(
        largest,
        -rounded_value,
        dest,
        NSCalendarOptions(0),
    )?;
    let again =
        cal.components_fromDate_toDate_options(all_units(), &shifted, dest, NSCalendarOptions(0));
    match first_non_zero(&again, &UNITS) {
        Some(new) if new.0 != largest => Some(new),
        _ => Some((largest, rounded_value)),
    }
}

/// `date.formatted(.relative(presentation: .named))`, relative to `now`: the unit and value
/// `Date.RelativeFormatStyle` computes with the current calendar (aligned to calendar
/// boundaries for days and up, rounded for hours and down), then worded by
/// `NSRelativeDateTimeFormatter` (named style, full units) from that single component, which
/// is ICU's `ureldatefmt_format` as the format style uses.
pub fn relative_named(d: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let cal = NSCalendar::currentCalendar();
    let dest = ns_date(d);
    let reference = ns_date(now);
    // Round the reference to whole seconds from the destination.
    let reference = dest.dateByAddingTimeInterval(reference.timeIntervalSinceDate(&dest).round());
    let c = cal.components_fromDate_toDate_options(
        all_units(),
        &reference,
        &dest,
        NSCalendarOptions(0),
    );
    let largest = first_non_zero(&c, &UNITS).unwrap_or((NSCalendarUnit::Second, 0));
    let hms = [
        NSCalendarUnit::Hour,
        NSCalendarUnit::Minute,
        NSCalendarUnit::Second,
    ];
    let (unit, amount) = if hms.contains(&largest.0) {
        rounded(&cal, &dest, &reference, largest.0).unwrap_or(largest)
    } else {
        aligned(&cal, largest.0, &dest, &reference).unwrap_or(largest)
    };
    let components = NSDateComponents::new();
    components.setValue_forComponent(amount, unit);
    let f = NSRelativeDateTimeFormatter::new();
    f.setLocale(Some(&NSLocale::currentLocale()));
    f.setDateTimeStyle(NSRelativeDateTimeFormatterStyle::Named);
    f.setUnitsStyle(NSRelativeDateTimeFormatterUnitsStyle::Full);
    f.localizedStringFromDateComponents(&components).to_string()
}

/// `ByteCountFormatter.string(fromByteCount:countStyle: .file)`.
pub fn byte_count(bytes: i64) -> String {
    NSByteCountFormatter::stringFromByteCount_countStyle(
        bytes,
        NSByteCountFormatterCountStyle::File,
    )
    .to_string()
}

/// `ListFormatter.localizedString(byJoining:)`.
pub fn list(items: &[String]) -> String {
    let strings: Vec<_> = items.iter().map(|s| NSString::from_str(s)).collect();
    let array = NSArray::from_retained_slice(&strings);
    NSListFormatter::localizedStringByJoiningStrings(&array).to_string()
}
