//! Port of `TimeTracking.swift`: `RunningTimer`, `TimerEntry`, `TimeFormat`, `TimerStore` and
//! `TimeTracking`.
//!
//! Time spent (PLAN.md Task 8b; SPEC.md Server facts: TimeSpent → Activity → ticket).
//! The app keeps its own timer and saves one TimeSpent per stop. A web stopwatch that's
//! running is a TimeSpent of yours with no end date (probe 2026-11-11), so the app shows it
//! instead of starting a second timer.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::client::ITopClient;
use crate::current_user::CurrentUser;
use crate::error::ITopError;
use crate::json::{JsonObject, JsonValue, swift_int};
use crate::rest_operation::RestOperation;
use crate::session::SettingsStore;
use crate::ticket::{Ticket, TicketClass};
use crate::ticket_writes::{COMMENT, TicketWrites};

/// The one timer that's running, saved so it survives quitting the app.
///
/// Its serde form (camelCase, `start` as an ISO date) is for the UI. What [`TimerStore`] keeps
/// is the Swift app's own encoding instead (see there).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct RunningTimer {
    pub ticket_class: String,
    pub ticket_key: String,
    pub r#ref: String,
    pub title: String,
    pub org_id: String,
    pub start: DateTime<Utc>,
}

impl RunningTimer {
    pub fn new(ticket: &Ticket, start: DateTime<Utc>) -> Self {
        RunningTimer {
            ticket_class: ticket.ticket_class.raw_value().to_string(),
            ticket_key: ticket.key.clone(),
            r#ref: ticket.r#ref.clone(),
            title: ticket.title.clone(),
            org_id: ticket.org_id.clone(),
            start,
        }
    }

    /// The ticket's id ("UserRequest::7"), as `Ticket::id()` gives it.
    pub fn ticket_id(&self) -> String {
        format!("{}::{}", self.ticket_class, self.ticket_key)
    }
}

/// What one stop saves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct TimerEntry {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    #[ts(type = "number")]
    pub duration: i64,
    /// The timer ran past the limit and was cut at it.
    pub capped: bool,
}

impl TimerEntry {
    /// iTop's stopwatch stops itself after a while (4 hours by default in the extension's
    /// docs); the app stops at the same point so the two agree. Seconds.
    pub const LIMIT: f64 = 4.0 * 3600.0;

    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>, duration: i64, capped: bool) -> Self {
        TimerEntry {
            start,
            end,
            duration,
            capped,
        }
    }

    /// `stop(_:at:)` with the default limit.
    pub fn stop(timer: &RunningTimer, now: DateTime<Utc>) -> TimerEntry {
        Self::stop_with_limit(timer, now, Self::LIMIT)
    }

    pub fn stop_with_limit(timer: &RunningTimer, now: DateTime<Utc>, limit: f64) -> TimerEntry {
        let elapsed = seconds_between(timer.start, now).max(0.0);
        let capped = elapsed > limit;
        // `Int(_:)` truncates toward zero.
        let seconds = elapsed.min(limit) as i64;
        TimerEntry {
            start: timer.start,
            end: timer.start + Duration::seconds(seconds),
            duration: seconds,
            capped,
        }
    }

    /// A stop under a minute is most likely a misclick, so it isn't saved.
    pub fn is_too_short(&self) -> bool {
        self.duration < 60
    }
}

/// `later.timeIntervalSince(earlier)` as fractional seconds.
fn seconds_between(earlier: DateTime<Utc>, later: DateTime<Utc>) -> f64 {
    let d = later - earlier;
    d.num_seconds() as f64 + f64::from(d.subsec_nanos()) / 1e9
}

pub struct TimeFormat;

impl TimeFormat {
    /// "0:42", "1:05", "10:05": hours and minutes.
    pub fn clock(seconds: i64) -> String {
        let minutes = seconds.max(0) / 60;
        format!("{}:{:02}", minutes / 60, minutes % 60)
    }

    /// Totals on rows and in the detail (DESIGN.md: Time spent): "1h 20m", "45m".
    pub fn total(seconds: i64) -> String {
        let minutes = seconds.max(0) / 60;
        let (h, m) = (minutes / 60, minutes % 60);
        if h == 0 {
            return format!("{m}m");
        }
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m}m")
        }
    }

    /// The live counter while a timer runs: "00:12:34".
    pub fn counter(seconds: i64) -> String {
        let s = seconds.max(0);
        format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    }

    /// A date as iTop writes it, in the Mac's time zone ("2026-11-11 09:30:00"), the inverse of
    /// `Ticket::parse_date`.
    pub fn itop(date: DateTime<Utc>) -> String {
        date.with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }
}

/// Where the running timer is kept between launches, under the same key as the Swift app
/// (`timer.running`).
///
/// The value is the JSON the Swift app wrote with `JSONEncoder()`:
/// `{"ticketClass","ticketKey","ref","title","orgID","start"}` with `start` as Foundation's
/// default date encoding, seconds since 2001-01-01 00:00:00 UTC (parity checklist PERS-7). It is
/// written in that same form, so a timer started in either app survives a switch to the other.
/// Reading also accepts `orgId` and an RFC 3339 `start`, in case one was saved in the UI's form.
///
/// The Swift app stored the JSON as `Data`; the [`SettingsStore`] behind this hands it over as
/// UTF-8 text (the app crate's defaults-backed store must read a `Data` value for this key).
#[derive(Clone)]
pub struct TimerStore {
    store: Arc<dyn SettingsStore>,
}

/// Seconds from the Unix epoch to Foundation's reference date, 2001-01-01 00:00:00 UTC.
const REFERENCE_DATE_UNIX: i64 = 978_307_200;

#[derive(Serialize, Deserialize)]
struct StoredTimer {
    #[serde(rename = "ticketClass")]
    ticket_class: String,
    #[serde(rename = "ticketKey")]
    ticket_key: String,
    #[serde(rename = "ref")]
    r#ref: String,
    title: String,
    #[serde(rename = "orgID", alias = "orgId")]
    org_id: String,
    #[serde(
        serialize_with = "serialize_reference_date",
        deserialize_with = "deserialize_reference_date"
    )]
    start: DateTime<Utc>,
}

fn serialize_reference_date<S: Serializer>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
    let seconds = (date.timestamp() - REFERENCE_DATE_UNIX) as f64
        + f64::from(date.timestamp_subsec_nanos()) / 1e9;
    // Whole seconds as an integer, as Swift's JSONEncoder writes them.
    if seconds.fract() == 0.0 && seconds.abs() < 1e15 {
        s.serialize_i64(seconds as i64)
    } else {
        s.serialize_f64(seconds)
    }
}

fn deserialize_reference_date<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    use serde::de::Error;
    match JsonValue::deserialize(d)? {
        JsonValue::Number(n) if n.is_finite() => {
            let whole = n.floor();
            let nanos = ((n - whole) * 1e9).round() as i64;
            let unix = whole as i64 + REFERENCE_DATE_UNIX;
            Utc.timestamp_opt(unix, 0)
                .single()
                .map(|t| t + Duration::nanoseconds(nanos))
                .ok_or_else(|| D::Error::custom("date out of range"))
        }
        JsonValue::String(s) => DateTime::parse_from_rfc3339(&s)
            .map(|t| t.with_timezone(&Utc))
            .map_err(D::Error::custom),
        _ => Err(D::Error::custom("expected a date")),
    }
}

impl TimerStore {
    pub const KEY: &'static str = "timer.running";

    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        TimerStore { store }
    }

    /// The saved timer; `None` when there is none or it can't be read (Swift's `try?`).
    pub fn load(&self) -> Option<RunningTimer> {
        let text = self.store.string(Self::KEY)?;
        let s: StoredTimer = serde_json::from_str(&text).ok()?;
        Some(RunningTimer {
            ticket_class: s.ticket_class,
            ticket_key: s.ticket_key,
            r#ref: s.r#ref,
            title: s.title,
            org_id: s.org_id,
            start: s.start,
        })
    }

    /// Saves the timer, or removes it with `None` (or if it couldn't be encoded).
    pub fn save(&self, timer: Option<&RunningTimer>) {
        let encoded = timer.and_then(|t| {
            serde_json::to_string(&StoredTimer {
                ticket_class: t.ticket_class.clone(),
                ticket_key: t.ticket_key.clone(),
                r#ref: t.r#ref.clone(),
                title: t.title.clone(),
                org_id: t.org_id.clone(),
                start: t.start,
            })
            .ok()
        });
        self.store.set_string(Self::KEY, encoded.as_deref());
    }
}

#[derive(Clone, Debug)]
pub struct TimeTracking {
    client: ITopClient,
}

impl TimeTracking {
    pub fn new(client: ITopClient) -> Self {
        TimeTracking { client }
    }

    fn label(ticket: &Ticket) -> String {
        format!("{} {}", ticket.r#ref, ticket.title)
    }

    /// The ticket's Activity id, or `None` when it has none yet.
    async fn activity_id(&self, ticket: &Ticket) -> Result<Option<String>, ITopError> {
        if !TicketWrites::is_id(&ticket.key) {
            return Ok(None);
        }
        let response = self
            .client
            .send(RestOperation::get(
                "Activity",
                format!(
                    "SELECT Activity WHERE obj_class = '{}' AND obj_id = {}",
                    ticket.ticket_class.raw_value(),
                    ticket.key
                ),
                &["obj_id"],
            ))
            .await?;
        Ok(response.objects().first().map(|o| o.key.clone()))
    }

    /// Saves one stop. Creates the ticket's Activity first when it has none (your choice,
    /// 2026-11-11).
    pub async fn save(
        &self,
        entry: &TimerEntry,
        ticket: &Ticket,
        me: &CurrentUser,
    ) -> Result<(), ITopError> {
        let link = TicketWrites::allowlist_key(ticket);
        let mut activity = self.activity_id(ticket).await?;
        if activity.is_none() {
            let mut fields = JsonObject::new();
            fields.insert("obj_class".into(), ticket.ticket_class.raw_value().into());
            fields.insert("obj_id".into(), ticket.key.as_str().into());
            fields.insert("label".into(), Self::label(ticket).into());
            let response = self
                .client
                .send_on_ticket(
                    RestOperation::create("Activity", fields, COMMENT),
                    Some(&link),
                )
                .await?;
            TicketWrites::check(&response)?;
            activity = response.objects().first().map(|o| o.key.clone());
        }
        let Some(activity) = activity.filter(|a| TicketWrites::is_id(a)) else {
            return Err(ITopError::Server {
                code: 0,
                message: "iTop didn't return the ticket's time-tracking activity.".into(),
            });
        };
        let mut fields = JsonObject::new();
        fields.insert("activity_id".into(), activity.into());
        fields.insert("user_id".into(), me.user_id.as_str().into());
        fields.insert("contact_id".into(), me.contact_id.as_str().into());
        fields.insert("org_id".into(), ticket.org_id.as_str().into());
        fields.insert("start_date".into(), TimeFormat::itop(entry.start).into());
        fields.insert("end_date".into(), TimeFormat::itop(entry.end).into());
        fields.insert("duration".into(), entry.duration.to_string().into());
        // Like iTop's web stopwatch: origin "stopwatch" and a ref-only title, so the calendar
        // and reports treat app time the same (your choice, 2026-11-11; probe showed "manual"
        // otherwise).
        fields.insert("title".into(), ticket.r#ref.as_str().into());
        fields.insert("origin".into(), "stopwatch".into());
        let response = self
            .client
            .send_on_ticket(
                RestOperation::create("TimeSpent", fields, COMMENT),
                Some(&link),
            )
            .await?;
        TicketWrites::check(&response)
    }

    /// When a web stopwatch of yours is running on the ticket: when it started. `None` otherwise.
    pub async fn running_stopwatch(
        &self,
        ticket: &Ticket,
        me: &CurrentUser,
    ) -> Result<Option<DateTime<Utc>>, ITopError> {
        let Some(activity) = self.activity_id(ticket).await? else {
            return Ok(None);
        };
        if !TicketWrites::is_id(&me.user_id) {
            return Ok(None);
        }
        let response = self
            .client
            .send(RestOperation::get(
                "TimeSpent",
                format!(
                    "SELECT TimeSpent WHERE activity_id = {activity} AND user_id = {}",
                    me.user_id
                ),
                &["start_date", "end_date", "duration"],
            ))
            .await?;
        let objects = response.objects();
        let open = objects.iter().rev().find(|o| {
            o.fields
                .get("end_date")
                .and_then(JsonValue::string_value)
                .unwrap_or("")
                .is_empty()
        });
        Ok(open.and_then(|o| {
            Ticket::parse_date(
                o.fields
                    .get("start_date")
                    .and_then(JsonValue::string_value)
                    .unwrap_or(""),
            )
        }))
    }

    /// Total seconds spent per ticket id, everyone's time included. Tickets with no Activity are
    /// left out.
    pub async fn totals(&self, tickets: &[Ticket]) -> Result<HashMap<String, i64>, ITopError> {
        let mut result: HashMap<String, i64> = HashMap::new();
        for cls in TicketClass::ALL {
            let keys: Vec<&str> = tickets
                .iter()
                .filter(|t| t.ticket_class == cls && TicketWrites::is_id(&t.key))
                .map(|t| t.key.as_str())
                .collect();
            if keys.is_empty() {
                continue;
            }
            let raw = cls.raw_value();
            let activities = self
                .client
                .send(RestOperation::get(
                    "Activity",
                    format!(
                        "SELECT Activity WHERE obj_class = '{raw}' AND obj_id IN ({})",
                        keys.join(",")
                    ),
                    &["obj_id"],
                ))
                .await?
                .objects();
            let mut ticket_for: HashMap<String, String> = HashMap::new();
            for a in &activities {
                let obj = a.fields.get("obj_id").and_then(|v| match v {
                    JsonValue::String(s) => Some(s.clone()),
                    other => other.int_value().map(|n| n.to_string()),
                });
                let Some(obj) = obj else { continue };
                let id = format!("{raw}::{obj}");
                ticket_for.insert(a.key.clone(), id.clone());
                result.entry(id).or_insert(0);
            }
            let ids: Vec<&str> = activities
                .iter()
                .map(|a| a.key.as_str())
                .filter(|k| TicketWrites::is_id(k))
                .collect();
            if ids.is_empty() {
                continue;
            }
            let spent = self
                .client
                .send(RestOperation::get(
                    "TimeSpent",
                    format!("SELECT TimeSpent WHERE activity_id IN ({})", ids.join(",")),
                    &["activity_id", "duration"],
                ))
                .await?
                .objects();
            for s in &spent {
                let Some(id) = s
                    .fields
                    .get("activity_id")
                    .and_then(JsonValue::string_value)
                    .and_then(|a| ticket_for.get(a))
                else {
                    continue;
                };
                // `Int(stringValue ?? "") ?? intValue ?? 0`.
                let duration = s.fields.get("duration");
                let seconds = swift_int(duration.and_then(JsonValue::string_value).unwrap_or(""))
                    .or_else(|| duration.and_then(JsonValue::int_value))
                    .unwrap_or(0);
                *result.entry(id.clone()).or_insert(0) += seconds;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemorySettingsStore;

    /// A value the Swift app wrote (`start` = 2026-11-11 08:30:00 UTC, with a fraction).
    #[test]
    fn decodes_what_swift_saved() {
        let settings = Arc::new(MemorySettingsStore::new());
        settings.set_string(
            TimerStore::KEY,
            Some(r#"{"ticketClass":"UserRequest","ticketKey":"7","ref":"R-000007","title":"VPN drops","orgID":"4","start":816078600.25}"#),
        );
        let timer = TimerStore::new(settings.clone()).load().unwrap();
        assert_eq!(timer.org_id, "4");
        assert_eq!(
            timer.start,
            Utc.with_ymd_and_hms(2026, 11, 11, 8, 30, 0).unwrap() + Duration::milliseconds(250)
        );
        assert_eq!(timer.ticket_id(), "UserRequest::7");

        // Written back in the same form.
        TimerStore::new(settings.clone()).save(Some(&timer));
        let text = settings.string(TimerStore::KEY).unwrap();
        assert!(text.contains(r#""orgID":"4""#), "{text}");
        assert!(text.contains(r#""start":816078600.25"#), "{text}");

        let whole = RunningTimer {
            start: Utc.with_ymd_and_hms(2026, 11, 11, 8, 30, 0).unwrap(),
            ..timer
        };
        TimerStore::new(settings.clone()).save(Some(&whole));
        assert!(
            settings
                .string(TimerStore::KEY)
                .unwrap()
                .contains(r#""start":816078600}"#)
        );
    }

    #[test]
    fn garbage_reads_as_none() {
        let settings = Arc::new(MemorySettingsStore::new());
        settings.set_string(TimerStore::KEY, Some("not json"));
        assert_eq!(TimerStore::new(settings).load(), None);
    }
}
