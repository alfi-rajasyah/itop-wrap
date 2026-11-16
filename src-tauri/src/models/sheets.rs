//! Ports of `AssignSheet` and `PendingSheet` / `ResolveSheet` (W/AssignSheet.swift,
//! W/LifecycleSheets.swift): their `@State` and `.task` loads, per list (ASG-6…13, LC-7…22).

use chrono::{DateTime, Utc};
use itop_kit::{
    Agent, Resolution, ResolutionCode, ServiceRef, Team, Ticket as KitTicket, TicketChange,
    TicketWrites, TimeFormat,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::records::{Dropdown, ListScope, Loadable, PickItem, RequiredMark, placeholder};
use super::workspace::{State, Workspace};
use crate::error::AppError;

#[derive(Clone, Debug, Default)]
pub struct AssignState {
    pub open: bool,
    pub instance: u64,
    pub ticket: Option<KitTicket>,
    pub teams: Loadable<Vec<Team>>,
    pub agents: Loadable<Vec<Agent>>,
    pub team: Option<Team>,
    pub agent: Option<Agent>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PendingState {
    pub open: bool,
    pub instance: u64,
    pub ticket: Option<KitTicket>,
    pub reason: String,
    pub error: Option<String>,
    pub tried: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ResolveState {
    pub open: bool,
    pub instance: u64,
    pub ticket: Option<KitTicket>,
    pub services: Loadable<Vec<ServiceRef>>,
    pub subcategories: Loadable<Vec<ServiceRef>>,
    pub service: Option<ServiceRef>,
    pub subcategory: Option<ServiceRef>,
    pub code: Option<ResolutionCode>,
    pub solution: String,
    pub error: Option<String>,
    pub tried: bool,
    pub prefilling: bool,
}

// MARK: Snapshots and intents

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct AssignToMeButton {
    pub enabled: bool,
    pub help: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct AssignSheetOpen {
    pub ticket_id: String,
    pub title: String,
    pub subtitle: String,
    pub assign_to_me: AssignToMeButton,
    pub team: Dropdown,
    pub agent: Dropdown,
    pub confirm_label: String,
    pub confirm_enabled: bool,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum AssignSheetSnapshot {
    Closed,
    Open(Box<AssignSheetOpen>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum AssignSheetIntent {
    Open {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    Close,
    AssignToMe,
    SetTeam {
        #[serde(rename = "teamId")]
        #[ts(rename = "teamId")]
        team_id: Option<String>,
    },
    SetAgent {
        #[serde(rename = "agentId")]
        #[ts(rename = "agentId")]
        agent_id: Option<String>,
    },
    Confirm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct PendingSheetOpen {
    pub ticket_id: String,
    pub title: String,
    pub subtitle: String,
    pub reason: String,
    pub reason_required: RequiredMark,
    pub reason_error: Option<String>,
    pub timer_line: Option<String>,
    pub error: Option<String>,
    pub busy: bool,
    pub confirm_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PendingSheetSnapshot {
    Closed,
    Open(Box<PendingSheetOpen>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PendingSheetIntent {
    Open {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    Close,
    SetReason {
        reason: String,
    },
    Confirm,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ResolveSheetOpen {
    pub ticket_id: String,
    pub title: String,
    pub subtitle: String,
    pub service: Dropdown,
    pub service_required: RequiredMark,
    pub subcategory: Dropdown,
    pub subcategory_required: RequiredMark,
    pub code: Dropdown,
    pub solution: String,
    pub solution_required: RequiredMark,
    pub solution_error: Option<String>,
    pub timer_line: Option<String>,
    pub error: Option<String>,
    pub busy: bool,
    pub confirm_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ResolveSheetSnapshot {
    Closed,
    Open(Box<ResolveSheetOpen>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ResolveSheetIntent {
    Open {
        #[serde(rename = "ticketId")]
        #[ts(rename = "ticketId")]
        ticket_id: String,
    },
    Close,
    SetService {
        #[serde(rename = "serviceId")]
        #[ts(rename = "serviceId")]
        service_id: Option<String>,
    },
    SetSubcategory {
        #[serde(rename = "subcategoryId")]
        #[ts(rename = "subcategoryId")]
        subcategory_id: Option<String>,
    },
    SetCode {
        #[serde(rename = "codeId")]
        #[ts(rename = "codeId")]
        code_id: Option<String>,
    },
    SetSolution {
        solution: String,
    },
    Confirm,
    /// Resolve, then Close the resolved ticket (iTop Wrap addition, not in the Swift app).
    ConfirmAndClose,
}

fn dropdown(
    items: Vec<PickItem>,
    selected: Option<String>,
    placeholder: String,
    enabled: bool,
) -> Dropdown {
    Dropdown {
        items,
        selected_id: selected,
        placeholder,
        enabled,
        search: None,
    }
}

fn services_items(list: &[ServiceRef]) -> Vec<PickItem> {
    list.iter()
        .map(|s| PickItem {
            id: s.id.clone(),
            label: s.name.clone(),
        })
        .collect()
}

const TIMER_LINE: fn(&str) -> String =
    |clock| format!("Your timer stops and saves {clock} to this ticket.");

impl Workspace {
    /// The sheet's `ticket`: the list's latest copy by id (contract §3).
    fn sheet_ticket(st: &State, scope: ListScope, stored: Option<&KitTicket>) -> Option<KitTicket> {
        let stored = stored?;
        Some(
            st.lists[scope.index()]
                .ticket(Some(&stored.id()))
                .unwrap_or(stored)
                .clone(),
        )
    }

    /// The detail went to another ticket (or none): its sheets go with it.
    pub(crate) fn close_sheets(&self, st: &mut State, scope: ListScope) {
        let i = scope.index();
        st.assign[i].open = false;
        st.pending[i].open = false;
        st.resolve[i].open = false;
    }

    /// LC-11, LC-20: "Your timer stops and saves 0:42 to this ticket." when it runs on the ticket.
    fn timer_line(st: &State, t: &KitTicket, now: DateTime<Utc>) -> Option<String> {
        let running = st.timer.running.as_ref()?;
        (running.ticket_id() == t.id())
            .then(|| TIMER_LINE(&TimeFormat::clock((now - running.start).num_seconds())))
    }

    fn open_ticket_for_sheet(
        &self,
        scope: ListScope,
        ticket_id: &str,
    ) -> Result<(KitTicket, u64), AppError> {
        let mut st = self.lock();
        let t = st.lists[scope.index()]
            .ticket(Some(ticket_id))
            .cloned()
            .ok_or_else(|| {
                AppError::invalid_intent(format!("No ticket {ticket_id} in this list."))
            })?;
        let instance = st.next_instance;
        st.next_instance += 1;
        Ok((t, instance))
    }

    // MARK: Assign

    pub(crate) fn assign_snapshot(&self, st: &State, scope: ListScope) -> AssignSheetSnapshot {
        let a = &st.assign[scope.index()];
        let Some(t) = Self::sheet_ticket(st, scope, a.ticket.as_ref()).filter(|_| a.open) else {
            return AssignSheetSnapshot::Closed;
        };
        let me = self.me();
        let busy = st.lists[scope.index()].working.is_some();
        let yours = t.agent_id == me.contact_id;
        let agents = if a.team.is_none() {
            Vec::new()
        } else {
            a.agents.value().cloned().unwrap_or_default()
        };
        AssignSheetSnapshot::Open(Box::new(AssignSheetOpen {
            ticket_id: t.id(),
            title: if t.is_unassigned() {
                format!("Assign {}", t.r#ref)
            } else {
                format!("Reassign {}", t.r#ref)
            },
            subtitle: t.title.clone(),
            assign_to_me: AssignToMeButton {
                enabled: !(busy || yours),
                help: if yours {
                    "It's already yours".to_owned()
                } else {
                    format!("Assign {} to you", t.r#ref)
                },
            },
            team: dropdown(
                a.teams
                    .value()
                    .map(|ts| {
                        ts.iter()
                            .map(|x| PickItem {
                                id: x.id.clone(),
                                label: x.name.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                a.team.as_ref().map(|x| x.id.clone()),
                placeholder(&a.teams, "Loading teams…", "No teams found"),
                a.teams.value().is_some(),
            ),
            agent: dropdown(
                agents
                    .iter()
                    .map(|x| PickItem {
                        id: x.id.clone(),
                        label: if x.id == me.contact_id {
                            format!("{} (you)", x.name)
                        } else {
                            x.name.clone()
                        },
                    })
                    .collect(),
                a.agent.as_ref().map(|x| x.id.clone()),
                if a.team.is_none() {
                    "Choose a team first".to_owned()
                } else {
                    placeholder(&a.agents, "Loading agents…", "Nobody in this team")
                },
                !(a.team.is_none() || a.agents.value().is_none()),
            ),
            confirm_label: if t.is_unassigned() {
                "Assign ticket"
            } else {
                "Reassign ticket"
            }
            .to_owned(),
            confirm_enabled: !(busy || a.team.is_none() || a.agent.is_none()),
            busy,
            error: a.error.clone(),
        }))
    }

    pub(crate) async fn dispatch_assign(
        &self,
        scope: ListScope,
        intent: AssignSheetIntent,
    ) -> Result<serde_json::Value, AppError> {
        let i = scope.index();
        let null = serde_json::Value::Null;
        let text = |s: Option<String>| s.map_or(serde_json::Value::Null, serde_json::Value::String);
        if let AssignSheetIntent::Open { ticket_id } = &intent {
            let (t, instance) = self.open_ticket_for_sheet(scope, ticket_id)?;
            self.lock().assign[i] = AssignState {
                open: true,
                instance,
                ticket: Some(t.clone()),
                teams: Loadable::Loading,
                agents: Loadable::Loading,
                ..Default::default()
            };
            self.publish();
            let ws = self.clone();
            self.spawn(async move { ws.assign_load_teams(scope, instance, t).await });
            return Ok(null);
        }
        let (t, instance) = {
            let st = self.lock();
            let a = &st.assign[i];
            match Self::sheet_ticket(&st, scope, a.ticket.as_ref()).filter(|_| a.open) {
                Some(t) => (t, a.instance),
                None => return Err(AppError::invalid_intent("The Assign sheet isn't open.")),
            }
        };
        match intent {
            AssignSheetIntent::Open { .. } => Ok(null),
            AssignSheetIntent::Close => {
                self.lock().assign[i].open = false;
                self.publish();
                Ok(null)
            }
            AssignSheetIntent::AssignToMe => {
                self.list_assign_to_me(scope, &t).await;
                let error = {
                    let mut st = self.lock();
                    let ok = matches!(&st.lists[i].notice, Some(n) if !n.is_error);
                    let notice = st.lists[i].notice.as_ref().map(|n| n.text.clone());
                    let a = &mut st.assign[i];
                    if a.instance != instance {
                        None
                    } else if ok {
                        a.open = false;
                        None
                    } else {
                        a.error = notice.clone();
                        notice
                    }
                };
                self.publish();
                Ok(text(error))
            }
            AssignSheetIntent::SetTeam { team_id } => {
                let new =
                    match &team_id {
                        None => None,
                        Some(id) => {
                            let found = self.lock().assign[i]
                                .teams
                                .value()
                                .and_then(|ts| ts.iter().find(|x| &x.id == id).cloned());
                            Some(found.ok_or_else(|| {
                                AppError::invalid_intent(format!("No team {id}."))
                            })?)
                        }
                    };
                self.assign_set_team(scope, instance, new);
                Ok(null)
            }
            AssignSheetIntent::SetAgent { agent_id } => {
                {
                    let mut st = self.lock();
                    let a = &mut st.assign[i];
                    a.agent = match &agent_id {
                        None => None,
                        Some(id) => {
                            let found = if a.team.is_none() {
                                None
                            } else {
                                a.agents
                                    .value()
                                    .and_then(|xs| xs.iter().find(|x| &x.id == id).cloned())
                            };
                            Some(found.ok_or_else(|| {
                                AppError::invalid_intent(format!("No agent {id}."))
                            })?)
                        }
                    };
                }
                self.publish();
                Ok(null)
            }
            AssignSheetIntent::Confirm => {
                let (team, agent) = {
                    let st = self.lock();
                    (st.assign[i].team.clone(), st.assign[i].agent.clone())
                };
                let (Some(team), Some(agent)) = (team, agent) else {
                    return Ok(null);
                };
                let message = self.list_assign(scope, &t, &team, &agent).await;
                {
                    let mut st = self.lock();
                    let a = &mut st.assign[i];
                    if a.instance == instance {
                        match &message {
                            Some(m) => a.error = Some(m.clone()),
                            None => a.open = false,
                        }
                    }
                }
                self.publish();
                Ok(text(message))
            }
        }
    }

    /// `loadTeams()`: every active team, the ticket's own preselected (ASG-9).
    async fn assign_load_teams(&self, scope: ListScope, instance: u64, t: KitTicket) {
        let result = self.0.directory.teams().await;
        let preselect = {
            let mut st = self.lock();
            let a = &mut st.assign[scope.index()];
            if a.instance != instance {
                return;
            }
            match result {
                Ok(all) => {
                    let own = all.iter().find(|x| x.id == t.team_id).cloned();
                    a.teams = Loadable::Loaded(all);
                    own
                }
                Err(e) => {
                    a.teams = Loadable::Failed(self.describe(&e));
                    None
                }
            }
        };
        self.publish();
        if preselect.is_some() {
            self.assign_set_team(scope, instance, preselect);
        }
    }

    /// The `team` binding and its `onChange`: the agent clears and the team's agents load (ASG-11).
    fn assign_set_team(&self, scope: ListScope, instance: u64, team: Option<Team>) {
        let load = {
            let mut st = self.lock();
            let a = &mut st.assign[scope.index()];
            if a.team == team {
                return;
            }
            a.team = team.clone();
            a.agent = None;
            team
        };
        self.publish();
        if let Some(chosen) = load {
            let ws = self.clone();
            self.spawn(async move { ws.assign_load_agents(scope, instance, chosen).await });
        }
    }

    /// `loadAgents(_:)`: a late answer for a previous team is ignored.
    async fn assign_load_agents(&self, scope: ListScope, instance: u64, chosen: Team) {
        {
            let mut st = self.lock();
            let a = &mut st.assign[scope.index()];
            if a.instance != instance {
                return;
            }
            a.agents = Loadable::Loading;
        }
        self.publish();
        let result = self.0.directory.agents(&chosen.id).await;
        {
            let mut st = self.lock();
            let a = &mut st.assign[scope.index()];
            if a.instance != instance {
                return;
            }
            match result {
                Ok(found) => {
                    if a.team.as_ref() != Some(&chosen) {
                        return;
                    }
                    a.agents = Loadable::Loaded(found);
                }
                Err(e) => a.agents = Loadable::Failed(self.describe(&e)),
            }
        }
        self.publish();
    }

    // MARK: Pending

    pub(crate) fn pending_snapshot(
        &self,
        st: &State,
        scope: ListScope,
        now: DateTime<Utc>,
    ) -> PendingSheetSnapshot {
        let p = &st.pending[scope.index()];
        let Some(t) = Self::sheet_ticket(st, scope, p.ticket.as_ref()).filter(|_| p.open) else {
            return PendingSheetSnapshot::Closed;
        };
        let working = &st.lists[scope.index()].working;
        let empty = p.reason.trim().is_empty();
        PendingSheetSnapshot::Open(Box::new(PendingSheetOpen {
            ticket_id: t.id(),
            title: format!("Set {} to pending", t.r#ref),
            subtitle: t.title.clone(),
            reason: p.reason.clone(),
            reason_required: RequiredMark {
                missing: p.tried && empty,
            },
            reason_error: (p.tried && empty).then(|| "Enter why the ticket is waiting.".to_owned()),
            timer_line: Self::timer_line(st, &t, now),
            error: p.error.clone(),
            busy: working.as_deref() == Some(t.id().as_str()),
            confirm_enabled: working.is_none(),
        }))
    }

    pub(crate) async fn dispatch_pending(
        &self,
        scope: ListScope,
        intent: PendingSheetIntent,
    ) -> Result<serde_json::Value, AppError> {
        let i = scope.index();
        let null = serde_json::Value::Null;
        if let PendingSheetIntent::Open { ticket_id } = &intent {
            let (t, instance) = self.open_ticket_for_sheet(scope, ticket_id)?;
            self.lock().pending[i] = PendingState {
                open: true,
                instance,
                ticket: Some(t),
                ..Default::default()
            };
            self.publish();
            return Ok(null);
        }
        let (t, instance) = {
            let st = self.lock();
            let p = &st.pending[i];
            match Self::sheet_ticket(&st, scope, p.ticket.as_ref()).filter(|_| p.open) {
                Some(t) => (t, p.instance),
                None => return Err(AppError::invalid_intent("The Pending sheet isn't open.")),
            }
        };
        match intent {
            PendingSheetIntent::Open { .. } => Ok(null),
            PendingSheetIntent::Close => {
                self.lock().pending[i].open = false;
                self.publish();
                Ok(null)
            }
            PendingSheetIntent::SetReason { reason } => {
                self.lock().pending[i].reason = reason;
                self.publish();
                Ok(null)
            }
            PendingSheetIntent::Confirm => {
                let reason = {
                    let mut st = self.lock();
                    st.pending[i].tried = true;
                    st.pending[i].reason.clone()
                };
                self.publish();
                if reason.trim().is_empty() {
                    return Ok(null);
                }
                let message = self
                    .list_apply(scope, &TicketChange::Pending { reason }, &t)
                    .await;
                {
                    let mut st = self.lock();
                    let p = &mut st.pending[i];
                    if p.instance == instance {
                        match &message {
                            Some(m) => p.error = Some(m.clone()),
                            None => p.open = false,
                        }
                    }
                }
                self.publish();
                Ok(message.map_or(serde_json::Value::Null, serde_json::Value::String))
            }
        }
    }

    // MARK: Resolve

    pub(crate) fn resolve_snapshot(
        &self,
        st: &State,
        scope: ListScope,
        now: DateTime<Utc>,
    ) -> ResolveSheetSnapshot {
        let r = &st.resolve[scope.index()];
        let Some(t) = Self::sheet_ticket(st, scope, r.ticket.as_ref()).filter(|_| r.open) else {
            return ResolveSheetSnapshot::Closed;
        };
        let working = &st.lists[scope.index()].working;
        let solution_empty = r.solution.trim().is_empty();
        let subcats = if r.service.is_none() {
            Vec::new()
        } else {
            r.subcategories.value().cloned().unwrap_or_default()
        };
        ResolveSheetSnapshot::Open(Box::new(ResolveSheetOpen {
            ticket_id: t.id(),
            title: format!("Resolve {}", t.r#ref),
            subtitle: t.title.clone(),
            service: dropdown(
                r.services
                    .value()
                    .map(|s| services_items(s))
                    .unwrap_or_default(),
                r.service.as_ref().map(|s| s.id.clone()),
                placeholder(&r.services, "Loading services…", "No services found"),
                r.services.value().is_some(),
            ),
            service_required: RequiredMark {
                missing: r.tried && r.service.is_none(),
            },
            subcategory: dropdown(
                services_items(&subcats),
                r.subcategory.as_ref().map(|s| s.id.clone()),
                if r.service.is_none() {
                    "Choose a service first".to_owned()
                } else {
                    placeholder(
                        &r.subcategories,
                        "Loading subcategories…",
                        "This service has no subcategories",
                    )
                },
                !(r.service.is_none() || r.subcategories.value().is_none()),
            ),
            subcategory_required: RequiredMark {
                missing: r.tried && r.subcategory.is_none(),
            },
            code: dropdown(
                ResolutionCode::standard()
                    .iter()
                    .map(|c| PickItem {
                        id: c.id.clone(),
                        label: c.name(),
                    })
                    .collect(),
                r.code.as_ref().map(|c| c.id.clone()),
                "Choose…".to_owned(),
                true,
            ),
            solution: r.solution.clone(),
            solution_required: RequiredMark {
                missing: r.tried && solution_empty,
            },
            solution_error: (r.tried && solution_empty)
                .then(|| "Describe what fixed it. The caller sees this.".to_owned()),
            timer_line: Self::timer_line(st, &t, now),
            error: r.error.clone(),
            busy: working.as_deref() == Some(t.id().as_str()),
            confirm_enabled: working.is_none(),
        }))
    }

    pub(crate) async fn dispatch_resolve(
        &self,
        scope: ListScope,
        intent: ResolveSheetIntent,
    ) -> Result<serde_json::Value, AppError> {
        let i = scope.index();
        let null = serde_json::Value::Null;
        if let ResolveSheetIntent::Open { ticket_id } = &intent {
            let (t, instance) = self.open_ticket_for_sheet(scope, ticket_id)?;
            self.lock().resolve[i] = ResolveState {
                open: true,
                instance,
                ticket: Some(t.clone()),
                services: Loadable::Loading,
                subcategories: Loadable::Idle,
                code: ResolutionCode::standard().into_iter().next(),
                prefilling: true,
                ..Default::default()
            };
            self.publish();
            let ws = self.clone();
            self.spawn(async move { ws.resolve_load_services(scope, instance, t).await });
            return Ok(null);
        }
        let (t, instance) = {
            let st = self.lock();
            let r = &st.resolve[i];
            match Self::sheet_ticket(&st, scope, r.ticket.as_ref()).filter(|_| r.open) {
                Some(t) => (t, r.instance),
                None => return Err(AppError::invalid_intent("The Resolve sheet isn't open.")),
            }
        };
        let and_close = matches!(intent, ResolveSheetIntent::ConfirmAndClose);
        match intent {
            ResolveSheetIntent::Open { .. } => Ok(null),
            ResolveSheetIntent::Close => {
                self.lock().resolve[i].open = false;
                self.publish();
                Ok(null)
            }
            ResolveSheetIntent::SetService { service_id } => {
                let new =
                    match &service_id {
                        None => None,
                        Some(id) => {
                            let found = self.lock().resolve[i]
                                .services
                                .value()
                                .and_then(|xs| xs.iter().find(|x| &x.id == id).cloned());
                            Some(found.ok_or_else(|| {
                                AppError::invalid_intent(format!("No service {id}."))
                            })?)
                        }
                    };
                self.resolve_set_service(scope, instance, new);
                Ok(null)
            }
            ResolveSheetIntent::SetSubcategory { subcategory_id } => {
                {
                    let mut st = self.lock();
                    let r = &mut st.resolve[i];
                    r.subcategory = match &subcategory_id {
                        None => None,
                        Some(id) => {
                            let found = if r.service.is_none() {
                                None
                            } else {
                                r.subcategories
                                    .value()
                                    .and_then(|xs| xs.iter().find(|x| &x.id == id).cloned())
                            };
                            Some(found.ok_or_else(|| {
                                AppError::invalid_intent(format!("No subcategory {id}."))
                            })?)
                        }
                    };
                }
                self.publish();
                Ok(null)
            }
            ResolveSheetIntent::SetCode { code_id } => {
                let code = match &code_id {
                    None => None,
                    Some(id) => Some(
                        ResolutionCode::standard()
                            .into_iter()
                            .find(|c| &c.id == id)
                            .ok_or_else(|| {
                                AppError::invalid_intent(format!("No resolution code {id}."))
                            })?,
                    ),
                };
                self.lock().resolve[i].code = code;
                self.publish();
                Ok(null)
            }
            ResolveSheetIntent::SetSolution { solution } => {
                self.lock().resolve[i].solution = solution;
                self.publish();
                Ok(null)
            }
            ResolveSheetIntent::Confirm | ResolveSheetIntent::ConfirmAndClose => {
                let fields = {
                    let mut st = self.lock();
                    let r = &mut st.resolve[i];
                    r.tried = true;
                    match (&r.service, &r.subcategory, &r.code) {
                        (Some(s), Some(sub), Some(code)) if !r.solution.trim().is_empty() => {
                            Some(Resolution::new(
                                s.id.clone(),
                                sub.id.clone(),
                                code.id.clone(),
                                r.solution.clone(),
                            ))
                        }
                        _ => None,
                    }
                };
                self.publish();
                let Some(resolution) = fields else {
                    return Ok(null);
                };
                let message = self
                    .list_apply(scope, &TicketChange::Resolve(resolution), &t)
                    .await;
                if and_close && message.is_none() {
                    // The ticket is resolved now; Close needs that state. A refused Close
                    // leaves it resolved and reports to the list, since the sheet is done.
                    let resolved = KitTicket {
                        status: "resolved".to_owned(),
                        ..t.clone()
                    };
                    self.list_apply_from_header(scope, &TicketChange::Close, &resolved)
                        .await;
                }
                {
                    let mut st = self.lock();
                    let r = &mut st.resolve[i];
                    if r.instance == instance {
                        match &message {
                            Some(m) => r.error = Some(m.clone()),
                            None => r.open = false,
                        }
                    }
                }
                self.publish();
                Ok(message.map_or(serde_json::Value::Null, serde_json::Value::String))
            }
        }
    }

    /// `loadServices()`: the org's services, the ticket's own service and subcategory prefilled
    /// even when not offered (LC-14, LC-15).
    async fn resolve_load_services(&self, scope: ListScope, instance: u64, t: KitTicket) {
        let i = scope.index();
        let result = self.0.catalog.services(&t).await;
        let mut all = match result {
            Ok(all) => all,
            Err(e) => {
                {
                    let mut st = self.lock();
                    if st.resolve[i].instance == instance {
                        st.resolve[i].services = Loadable::Failed(self.describe(&e));
                    }
                }
                self.publish();
                return;
            }
        };
        let loaded = {
            let st = self.lock();
            let d = &st.details[i];
            d.loaded()
                .filter(|_| d.ticket.as_ref().map(KitTicket::id) == Some(t.id()))
                .cloned()
        };
        let loaded = match loaded {
            Some(d) => Some(d),
            None => self.0.tickets.detail(&t).await.ok(),
        };
        let select = {
            let mut st = self.lock();
            let r = &mut st.resolve[i];
            if r.instance != instance {
                return;
            }
            match loaded.filter(|d| TicketWrites::is_id(&d.service_id) && d.service_id != "0") {
                Some(d) => {
                    let name = if d.service_name.is_empty() {
                        format!("Service {}", d.service_id)
                    } else {
                        d.service_name.clone()
                    };
                    let current = ServiceRef::new(d.service_id.clone(), name);
                    if !all.iter().any(|s| s.id == current.id) {
                        all.insert(0, current.clone());
                    }
                    if TicketWrites::is_id(&d.subcategory_id) && d.subcategory_id != "0" {
                        let name = if d.subcategory_name.is_empty() {
                            format!("Subcategory {}", d.subcategory_id)
                        } else {
                            d.subcategory_name.clone()
                        };
                        r.subcategory = Some(ServiceRef::new(d.subcategory_id.clone(), name));
                    }
                    let chosen = all.iter().find(|s| s.id == current.id).cloned();
                    r.services = Loadable::Loaded(all);
                    chosen
                }
                None => {
                    r.prefilling = false;
                    r.services = Loadable::Loaded(all);
                    None
                }
            }
        };
        self.publish();
        if select.is_some() {
            self.resolve_set_service(scope, instance, select);
        }
    }

    /// The `service` binding and its `onChange`: after the prefill, a new service clears the
    /// subcategory; its subcategories load (LC-16).
    fn resolve_set_service(&self, scope: ListScope, instance: u64, service: Option<ServiceRef>) {
        let load = {
            let mut st = self.lock();
            let r = &mut st.resolve[scope.index()];
            if r.service == service {
                return;
            }
            r.service = service.clone();
            if r.prefilling {
                r.prefilling = false;
            } else {
                r.subcategory = None;
            }
            r.subcategories = Loadable::Idle;
            service
        };
        self.publish();
        if let Some(chosen) = load {
            let ws = self.clone();
            self.spawn(async move { ws.resolve_load_subcategories(scope, instance, chosen).await });
        }
    }

    async fn resolve_load_subcategories(
        &self,
        scope: ListScope,
        instance: u64,
        chosen: ServiceRef,
    ) {
        let i = scope.index();
        {
            let mut st = self.lock();
            if st.resolve[i].instance != instance {
                return;
            }
            st.resolve[i].subcategories = Loadable::Loading;
        }
        self.publish();
        let result = self.0.catalog.subcategories(&chosen.id).await;
        {
            let mut st = self.lock();
            let r = &mut st.resolve[i];
            if r.instance != instance {
                return;
            }
            match result {
                Ok(mut found) => {
                    if r.service.as_ref() != Some(&chosen) {
                        return;
                    }
                    if let Some(s) = &r.subcategory
                        && !found.contains(s)
                    {
                        found.insert(0, s.clone());
                    }
                    r.subcategories = Loadable::Loaded(found);
                }
                Err(e) => r.subcategories = Loadable::Failed(self.describe(&e)),
            }
        }
        self.publish();
    }
}
