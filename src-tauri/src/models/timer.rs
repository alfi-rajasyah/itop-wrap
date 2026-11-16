//! Port of `TimerModel.swift`: the one timer (TMR-*). Starting another stops and saves the first;
//! a ticket that leaves Assigned stops it; it survives quitting (`timer.running`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use itop_kit::{
    CurrentUser, RunningTimer, Ticket as KitTicket, TicketClass, TimeFormat, TimerEntry,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::records::{rfc3339, with_period};
use super::workspace::{State, Workspace};
use crate::error::AppError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimerNoticeState {
    pub text: String,
    pub is_error: bool,
    /// A save that failed, kept so it can be retried.
    pub can_retry: bool,
}

#[derive(Clone, Debug)]
pub struct TimerState {
    pub running: Option<RunningTimer>,
    pub totals: HashMap<String, i64>,
    /// Web stopwatches of yours seen running, by ticket id, with their start.
    pub web_stopwatches: HashMap<String, DateTime<Utc>>,
    pub notice: Option<TimerNoticeState>,
    pub saving: bool,
    pub failed: Option<(TimerEntry, KitTicket)>,
}

impl TimerState {
    pub fn new(running: Option<RunningTimer>) -> Self {
        TimerState {
            running,
            totals: HashMap::new(),
            web_stopwatches: HashMap::new(),
            notice: None,
            saving: false,
            failed: None,
        }
    }

    /// Only tickets assigned to you and in the Assigned state get a start button (TMR-2).
    pub fn can_start(&self, t: &KitTicket, me: &CurrentUser) -> bool {
        t.status == "assigned" && t.agent_id == me.contact_id
    }

    pub fn is_running(&self, t: &KitTicket) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| r.ticket_id() == t.id())
    }
}

/// `TimerModel.ticket(for:)`: what saving needs of the ticket the timer runs on.
pub fn ticket_for(timer: &RunningTimer) -> KitTicket {
    KitTicket {
        key: timer.ticket_key.clone(),
        ticket_class: TicketClass::from_raw(&timer.ticket_class)
            .unwrap_or(TicketClass::UserRequest),
        r#ref: timer.r#ref.clone(),
        title: timer.title.clone(),
        status: String::new(),
        priority: 0,
        caller_name: String::new(),
        org_name: String::new(),
        org_id: timer.org_id.clone(),
        team_id: "0".to_owned(),
        team_name: String::new(),
        agent_id: "0".to_owned(),
        agent_name: String::new(),
        start_date: None,
        last_update: None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct RunningTimerView {
    pub ticket_id: String,
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub r#ref: String,
    pub title: String,
    pub started_at: String,
    pub elapsed_seconds: i64,
    pub counter: String,
    pub clock: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TimerNotice {
    pub text: String,
    pub is_error: bool,
    pub can_retry: bool,
    pub dismiss_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TimerSnapshot {
    pub running: Option<RunningTimerView>,
    pub saving: bool,
    pub notice: Option<TimerNotice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TimerIntent {
    Start {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    Stop,
    Toggle {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    Retry,
    ClearNotice,
}

impl Workspace {
    pub(crate) fn timer_snapshot(&self, st: &State, now: DateTime<Utc>) -> TimerSnapshot {
        let t = &st.timer;
        TimerSnapshot {
            running: t.running.as_ref().map(|r| {
                // The views count `Int(now - start)`; a clock set back shows 0, not a negative time.
                let elapsed = (now - r.start).num_seconds().max(0);
                RunningTimerView {
                    ticket_id: r.ticket_id(),
                    r#ref: r.r#ref.clone(),
                    title: r.title.clone(),
                    started_at: rfc3339(r.start),
                    elapsed_seconds: elapsed,
                    counter: TimeFormat::counter(elapsed),
                    clock: TimeFormat::clock(elapsed),
                }
            }),
            saving: t.saving,
            notice: t.notice.as_ref().map(|n| TimerNotice {
                text: n.text.clone(),
                is_error: n.is_error,
                can_retry: n.can_retry,
                dismiss_label: if n.can_retry {
                    "Discard this time"
                } else {
                    "Dismiss"
                }
                .to_owned(),
            }),
        }
    }

    /// A ticket the timer can act on, from either list (or a list's found ticket).
    fn timer_ticket(&self, id: &str) -> Option<KitTicket> {
        let st = self.lock();
        st.lists.iter().find_map(|l| l.ticket(Some(id)).cloned())
    }

    pub(crate) async fn dispatch_timer(
        &self,
        intent: TimerIntent,
    ) -> Result<serde_json::Value, AppError> {
        match intent {
            TimerIntent::Start { ticket_id } => {
                let t = self
                    .timer_ticket(&ticket_id)
                    .ok_or_else(|| AppError::invalid_intent(format!("No ticket {ticket_id}.")))?;
                self.timer_start(&t).await;
            }
            TimerIntent::Stop => self.timer_stop(None).await,
            TimerIntent::Toggle { ticket_id } => {
                let t = self
                    .timer_ticket(&ticket_id)
                    .ok_or_else(|| AppError::invalid_intent(format!("No ticket {ticket_id}.")))?;
                if self.lock().timer.is_running(&t) {
                    self.timer_stop(None).await;
                } else {
                    self.timer_start(&t).await;
                }
            }
            TimerIntent::Retry => self.timer_retry().await,
            TimerIntent::ClearNotice => {
                {
                    let mut st = self.lock();
                    st.timer.notice = None;
                    st.timer.failed = None;
                }
                self.publish();
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// `start(_:)` (TMR-2…6).
    pub(crate) async fn timer_start(&self, t: &KitTicket) {
        {
            let mut st = self.lock();
            let timer = &mut st.timer;
            if !timer.can_start(t, self.me()) || timer.is_running(t) || timer.saving {
                return;
            }
            if !self.can_write(t) {
                timer.notice = Some(TimerNoticeState {
                    text: "Saving time to iTop is switched off in this build, so the timer can't start.".to_owned(),
                    is_error: true,
                    can_retry: false,
                });
                drop(st);
                self.publish();
                return;
            }
        }
        // Don't count twice: a web stopwatch of yours on this ticket wins.
        if let Ok(Some(since)) = self.0.tracking.running_stopwatch(t, self.me()).await {
            let now = self.now();
            {
                let mut st = self.lock();
                st.timer.web_stopwatches.insert(t.id(), since);
                st.timer.notice = Some(TimerNoticeState {
                    text: format!(
                        "Your web stopwatch is already running on {} since {}. Stop it in iTop, or keep using it there.",
                        t.r#ref,
                        self.0.fmt.since(self.0.ctx.native.as_ref(), since, now)
                    ),
                    is_error: false,
                    can_retry: false,
                });
            }
            self.publish();
            return;
        }
        let another = {
            let mut st = self.lock();
            st.timer.web_stopwatches.remove(&t.id());
            st.timer.running.is_some()
        };
        if another {
            self.timer_stop(None).await;
        }
        self.trace(|| format!("timer: started on {}", t.id()));
        let running = RunningTimer::new(t, self.now());
        self.0.timer_store.save(Some(&running));
        {
            let mut st = self.lock();
            st.timer.running = Some(running);
            st.timer.notice = None;
        }
        self.publish();
    }

    /// `stop(because:)`: stops and saves; `reason` explains a stop you didn't ask for (TMR-7, TMR-13).
    pub(crate) async fn timer_stop(&self, reason: Option<&str>) {
        let now = self.now();
        let (timer, entry) = {
            let mut st = self.lock();
            let Some(timer) = st.timer.running.take() else {
                return;
            };
            let entry = TimerEntry::stop(&timer, now);
            (timer, entry)
        };
        self.0.timer_store.save(None);
        self.trace(|| {
            format!(
                "timer: stopped on {}, {} s",
                timer.ticket_id(),
                entry.duration
            )
        });
        let prefix = reason.map(|r| format!("{r} ")).unwrap_or_default();
        if entry.is_too_short() {
            self.lock().timer.notice = Some(TimerNoticeState {
                text: format!(
                    "{prefix}The timer on {} ran under a minute, so nothing was saved.",
                    timer.r#ref
                ),
                is_error: false,
                can_retry: false,
            });
            self.publish();
            return;
        }
        self.publish();
        self.timer_save(entry, ticket_for(&timer), &prefix).await;
    }

    /// `retry()` (TMR-12).
    pub(crate) async fn timer_retry(&self) {
        let failed = self.lock().timer.failed.clone();
        if let Some((entry, ticket)) = failed {
            self.timer_save(entry, ticket, "").await;
        }
    }

    async fn timer_save(&self, entry: TimerEntry, ticket: KitTicket, prefix: &str) {
        self.lock().timer.saving = true;
        self.publish();
        let result = self.0.tracking.save(&entry, &ticket, self.me()).await;
        {
            let mut st = self.lock();
            let timer = &mut st.timer;
            match result {
                Ok(()) => {
                    timer.failed = None;
                    *timer.totals.entry(ticket.id()).or_insert(0) += entry.duration;
                    let cap = if entry.capped {
                        format!(
                            " It stopped at {}, like iTop's own stopwatch.",
                            TimeFormat::clock(entry.duration)
                        )
                    } else {
                        String::new()
                    };
                    timer.notice = Some(TimerNoticeState {
                        text: format!(
                            "{prefix}Saved {} to {}.{cap}",
                            TimeFormat::clock(entry.duration),
                            ticket.r#ref
                        ),
                        is_error: false,
                        can_retry: false,
                    });
                }
                Err(e) => {
                    let reason = with_period(&self.describe(&e));
                    timer.notice = Some(TimerNoticeState {
                        text: format!(
                            "{prefix}Couldn't save {} to {}. {reason}",
                            TimeFormat::clock(entry.duration),
                            ticket.r#ref
                        ),
                        is_error: true,
                        can_retry: true,
                    });
                    timer.failed = Some((entry, ticket));
                }
            }
            timer.saving = false;
        }
        self.publish();
    }

    /// `myTicketsChanged(_:)`: after My tickets reloads, a running ticket that's no longer
    /// Assigned to you stops and saves (TMR-14).
    pub(crate) async fn timer_my_tickets_changed(&self, tickets: &[KitTicket]) {
        let reason = {
            let st = self.lock();
            let Some(timer) = &st.timer.running else {
                return;
            };
            let id = timer.ticket_id();
            if tickets
                .iter()
                .any(|t| t.id() == id && st.timer.can_start(t, self.me()))
            {
                return;
            }
            format!("{} left Assigned, so its timer stopped.", timer.r#ref)
        };
        self.timer_stop(Some(&reason)).await;
    }

    /// `stopIfRunning(on:because:)`: before Set to pending or Resolve (LC-4).
    pub(crate) async fn timer_stop_if_running(&self, t: &KitTicket, reason: &str) {
        if self.lock().timer.is_running(t) {
            self.timer_stop(Some(reason)).await;
        }
    }

    /// `refreshTotals(_:)` (TMR-17): open tickets only; a failure keeps the old totals.
    pub(crate) async fn timer_refresh_totals(&self, tickets: &[KitTicket]) {
        let open: Vec<KitTicket> = tickets
            .iter()
            .filter(|t| t.status != "resolved" && t.status != "closed")
            .cloned()
            .collect();
        let Ok(found) = self.0.tracking.totals(&open).await else {
            return;
        };
        self.lock().timer.totals.extend(found);
        self.publish();
    }

    /// `checkWebStopwatch(_:)` (TMR-15).
    pub(crate) async fn timer_check_web_stopwatch(&self, t: &KitTicket) {
        {
            let st = self.lock();
            if !st.timer.can_start(t, self.me()) || st.timer.is_running(t) {
                return;
            }
        }
        let found = self
            .0
            .tracking
            .running_stopwatch(t, self.me())
            .await
            .ok()
            .flatten();
        {
            let mut st = self.lock();
            match found {
                Some(since) => st.timer.web_stopwatches.insert(t.id(), since),
                None => st.timer.web_stopwatches.remove(&t.id()),
            };
        }
        self.publish();
    }
}
