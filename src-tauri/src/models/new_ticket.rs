//! `newTicket:sheet` and `newTicket:window`: New ticket. Port of `NewTicketSheet`
//! (W/NewTicketView.swift:37-284) with `Workspace.newTicketStart` / `newTicketDraft`
//! (W/Workspace.swift:19-23, 51-58) and `NewTicketWindow` (W/App.swift:56-70). contract.md §5.9.
//!
//! The two instances are independent forms (separate `@State` in Swift) that share the kept draft
//! and the remembered "Assign to me". SwiftUI's `onChange(of: org)` / `onChange(of: service)`
//! cascades are the `Form::set_org` / `Form::set_service` helpers: they run whenever the value
//! actually changes, whoever changed it, and queue the loads they start.

use std::sync::{Arc, Mutex, MutexGuard, Weak};

use itop_kit::ticket_writes::Failure;
use itop_kit::{
    ITopError, NewTicket, PickLists, ServiceCatalog, ServiceRef, TicketClass, TicketWrites,
    WriteError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::cmdb::text::{self, Loadable, describe, describe_write, status_label, trim_whitespaces};
use super::seams::{Model, ModelContext, WorkspaceLink};
use crate::error::AppError;

#[cfg(test)]
#[path = "new_ticket_tests.rs"]
mod tests;

pub const SHEET_ID: &str = "newTicket:sheet";
pub const WINDOW_ID: &str = "newTicket:window";

/// Which New ticket: the main window's sheet, or the menu bar's separate window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NewTicketInstance {
    Sheet,
    Window,
}

impl NewTicketInstance {
    pub fn model_id(self) -> &'static str {
        match self {
            NewTicketInstance::Sheet => SHEET_ID,
            NewTicketInstance::Window => WINDOW_ID,
        }
    }

    pub fn from_model_id(id: &str) -> Option<Self> {
        match id {
            SHEET_ID => Some(NewTicketInstance::Sheet),
            WINDOW_ID => Some(NewTicketInstance::Window),
            _ => None,
        }
    }
}

/// `NewTicketStart`: plain (all `None`), from an organization (id and name), or from a config
/// item (its org name, key and name).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewTicketStart {
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub ci_id: Option<String>,
    pub ci_name: Option<String>,
}

impl NewTicketStart {
    /// "New ticket" on an organization (SHELL-12).
    pub fn for_org(org_id: &str, org_name: &str) -> Self {
        Self {
            org_id: Some(org_id.to_owned()),
            org_name: Some(org_name.to_owned()),
            ..Self::default()
        }
    }

    /// "New ticket" on a config item: org matched by name, CI linked (SHELL-13).
    pub fn for_ci(key: &str, name: &str, org_name: &str) -> Self {
        Self {
            org_id: None,
            org_name: Some(org_name.to_owned()),
            ci_id: Some(key.to_owned()),
            ci_name: Some(name.to_owned()),
        }
    }
}

// MARK: - Contract shapes
pub use super::records::{Dropdown, DropdownSearch, PickItem, RequiredMark};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct NewTicketRequired {
    pub organization: RequiredMark,
    pub caller: RequiredMark,
    pub service: RequiredMark,
    pub subcategory: RequiredMark,
    pub title: RequiredMark,
    pub description: RequiredMark,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct NewTicketOpen {
    /// "On <CI name>" when started from a CI.
    pub subtitle: Option<String>,
    pub ticket_class: TicketClass,
    pub organization: Dropdown,
    pub caller: Dropdown,
    pub service: Dropdown,
    pub subcategory: Dropdown,
    pub impact: Dropdown,
    pub urgency: Dropdown,
    pub title: String,
    pub description: String,
    pub assign_to_me: bool,
    pub required: NewTicketRequired,
    /// "Fill in the caller first."
    pub missing_text: Option<String>,
    pub error: Option<String>,
    /// "Creating tickets is switched off in this build, so nothing is sent to iTop."
    pub disabled_text: Option<String>,
    pub creating: bool,
    pub confirm_enabled: bool,
    /// A kept draft exists: show "Clear form".
    pub show_clear_form: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum NewTicketSnapshot {
    Closed,
    Open(Box<NewTicketOpen>),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum NewTicketIntent {
    /// Plain start (restores a kept draft). Org/CI starts go through the shell.
    Open,
    #[serde(rename_all = "camelCase")]
    SetClass {
        ticket_class: TicketClass,
    },
    SetOrganization {
        id: Option<String>,
    },
    SetCaller {
        id: Option<String>,
    },
    SetService {
        id: Option<String>,
    },
    SetSubcategory {
        id: Option<String>,
    },
    SetImpact {
        id: Option<String>,
    },
    SetUrgency {
        id: Option<String>,
    },
    SetTitle {
        title: String,
    },
    SetDescription {
        description: String,
    },
    #[serde(rename_all = "camelCase")]
    SetAssignToMe {
        assign_to_me: bool,
    },
    ClearForm,
    /// Cancel / Esc: keeps the draft unless org, title and description are empty.
    Cancel,
    /// The window's close button: no draft kept.
    Dismiss,
    Create,
}

// MARK: - State

/// A load an `onChange` started (`Task { await loadForOrg(new) }` and friends).
enum Spawn {
    ForOrg(ServiceRef),
    Subcategories(ServiceRef),
}

/// One open form: the sheet's `@State`.
struct Form {
    /// Which opening this is; answers for an earlier one are ignored.
    generation: u64,
    start: NewTicketStart,
    draft: NewTicket,
    orgs: Loadable<Vec<ServiceRef>>,
    callers: Loadable<Vec<ServiceRef>>,
    services: Loadable<Vec<ServiceRef>>,
    subcategories: Loadable<Vec<ServiceRef>>,
    org: Option<ServiceRef>,
    caller: Option<ServiceRef>,
    service: Option<ServiceRef>,
    subcategory: Option<ServiceRef>,
    impact: Option<ServiceRef>,
    urgency: Option<ServiceRef>,
    tried: bool,
    creating: bool,
    error: Option<String>,
    /// Set while restoring a draft or a prefill, so the cascade doesn't clear the children.
    restoring: bool,
    pending: Vec<Spawn>,
}

impl Form {
    fn new(generation: u64, start: NewTicketStart) -> Self {
        Form {
            generation,
            start,
            draft: NewTicket::new(TicketClass::UserRequest),
            orgs: Loadable::Loading,
            callers: Loadable::Idle,
            services: Loadable::Idle,
            subcategories: Loadable::Idle,
            org: None,
            caller: None,
            service: None,
            subcategory: None,
            impact: PickLists::impacts().first().cloned(),
            urgency: PickLists::urgencies().last().cloned(),
            tried: false,
            creating: false,
            error: None,
            restoring: true,
            pending: Vec::new(),
        }
    }

    /// `.whitespacesAndNewlines` (Unicode White_Space, what `trim` uses).
    fn title_empty(&self) -> bool {
        self.draft.title.trim().is_empty()
    }

    fn description_empty(&self) -> bool {
        self.draft.description.trim().is_empty()
    }

    /// `firstMissing` (W/NewTicketView.swift:155-163).
    fn first_missing(&self) -> Option<&'static str> {
        if self.org.is_none() {
            Some("organization")
        } else if self.caller.is_none() {
            Some("caller")
        } else if self.service.is_none() {
            Some("service")
        } else if self.subcategory.is_none() {
            Some("subcategory")
        } else if self.title_empty() {
            Some("title")
        } else if self.description_empty() {
            Some("description")
        } else {
            None
        }
    }

    /// `org` and its `onChange` (W/NewTicketView.swift:143-147).
    fn set_org(&mut self, new: Option<ServiceRef>) {
        if self.org == new {
            return;
        }
        self.org = new.clone();
        if !self.restoring {
            self.caller = None;
            self.set_service(None);
            self.subcategory = None;
        }
        self.callers = Loadable::Idle;
        self.services = Loadable::Idle;
        if let Some(org) = new {
            self.pending.push(Spawn::ForOrg(org));
        }
    }

    /// `service` and its `onChange` (W/NewTicketView.swift:148-152).
    fn set_service(&mut self, new: Option<ServiceRef>) {
        if self.service == new {
            return;
        }
        self.service = new.clone();
        if !self.restoring {
            self.subcategory = None;
        }
        self.subcategories = Loadable::Idle;
        if let Some(service) = new {
            self.pending.push(Spawn::Subcategories(service));
        }
    }

    /// `filled()` (W/NewTicketView.swift:238-247).
    fn filled(&self) -> NewTicket {
        let mut d = self.draft.clone();
        d.org_id = self.org.as_ref().map_or_else(String::new, |r| r.id.clone());
        d.caller_id = self
            .caller
            .as_ref()
            .map_or_else(String::new, |r| r.id.clone());
        d.service_id = self
            .service
            .as_ref()
            .map_or_else(String::new, |r| r.id.clone());
        d.subcategory_id = self
            .subcategory
            .as_ref()
            .map_or_else(String::new, |r| r.id.clone());
        d.impact = self
            .impact
            .as_ref()
            .map_or_else(|| "1".to_owned(), |r| r.id.clone());
        d.urgency = self
            .urgency
            .as_ref()
            .map_or_else(|| "4".to_owned(), |r| r.id.clone());
        d
    }
}

struct Shared {
    /// `Workspace.newTicketDraft`: what was typed before cancelling, for this session.
    kept: Option<NewTicket>,
    sheet: Option<Form>,
    window: Option<Form>,
    next_generation: u64,
}

impl Shared {
    fn slot(&mut self, instance: NewTicketInstance) -> &mut Option<Form> {
        match instance {
            NewTicketInstance::Sheet => &mut self.sheet,
            NewTicketInstance::Window => &mut self.window,
        }
    }

    /// The instance's form, if it is still the opening `generation`.
    fn form(&mut self, instance: NewTicketInstance, generation: u64) -> Option<&mut Form> {
        self.slot(instance)
            .as_mut()
            .filter(|f| f.generation == generation)
    }
}

/// Both New ticket instances and the draft they share.
pub struct NewTickets {
    ctx: ModelContext,
    link: Arc<dyn WorkspaceLink>,
    me: Weak<NewTickets>,
    state: Mutex<Shared>,
}

impl NewTickets {
    pub fn new(ctx: ModelContext, link: Arc<dyn WorkspaceLink>) -> Arc<Self> {
        let model = Arc::new_cyclic(|me| NewTickets {
            ctx,
            link,
            me: me.clone(),
            state: Mutex::new(Shared {
                kept: None,
                sheet: None,
                window: None,
                next_generation: 0,
            }),
        });
        model.publish();
        model
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Publishes both instances (the kept draft and "Assign to me" are shared).
    fn publish(&self) {
        let st = self.lock();
        for instance in [NewTicketInstance::Sheet, NewTicketInstance::Window] {
            self.ctx
                .hub
                .publish(instance.model_id(), &self.build(&st, instance));
        }
    }

    pub fn snapshot(&self, instance: NewTicketInstance) -> NewTicketSnapshot {
        self.build(&self.lock(), instance)
    }

    /// The models stopped existing (sign-out): the window closes itself (NEW-2).
    pub fn clear(&self) {
        let had_window = {
            let mut st = self.lock();
            st.sheet = None;
            st.window.take().is_some()
        };
        if had_window {
            self.ctx.native.close_new_ticket_window();
        }
        self.ctx.hub.clear(SHEET_ID);
        self.ctx.hub.clear(WINDOW_ID);
    }

    // MARK: Opening

    /// Opens New ticket: `workspace.requestNewTicket(start)` for the sheet (⌘N, an org, a CI:
    /// NEW-1, NEW-3; a request while it shows starts a fresh form, like SwiftUI's
    /// `.sheet(item:)` with a new item), or the menu bar's window (NEW-2: brought forward with
    /// its form kept when already open). Returns once the organizations have loaded (`load()`).
    pub async fn request(&self, instance: NewTicketInstance, start: NewTicketStart) {
        let generation = {
            let mut st = self.lock();
            if instance == NewTicketInstance::Window && st.window.is_some() {
                drop(st);
                self.ctx.native.show_new_ticket_window();
                return;
            }
            st.next_generation += 1;
            let generation = st.next_generation;
            *st.slot(instance) = Some(Form::new(generation, start));
            generation
        };
        if instance == NewTicketInstance::Window {
            self.ctx.native.show_new_ticket_window();
        }
        self.publish();
        self.load(instance, generation).await;
    }

    /// `load()` (W/NewTicketView.swift:175-194).
    async fn load(&self, instance: NewTicketInstance, generation: u64) {
        {
            let mut st = self.lock();
            let kept = st.kept.clone();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            // A kept draft wins over a plain start; a start from an org or CI begins fresh.
            match kept {
                Some(kept) if form.start.org_id.is_none() && form.start.ci_id.is_none() => {
                    form.draft = kept
                }
                _ => form.draft.ci_id = form.start.ci_id.clone(),
            }
        }
        self.publish();
        let host = text::host(&self.ctx.session);
        let answer = PickLists::new(self.ctx.session.client.clone())
            .organizations()
            .await;
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            match answer {
                Ok(all) => {
                    let wanted = form.start.org_id.clone().or_else(|| {
                        (!form.draft.org_id.is_empty()).then(|| form.draft.org_id.clone())
                    });
                    let org = all
                        .iter()
                        .find(|o| Some(&o.id) == wanted.as_ref())
                        .or_else(|| {
                            form.start
                                .org_name
                                .as_ref()
                                .and_then(|name| all.iter().find(|o| &o.name == name))
                        })
                        .cloned();
                    form.orgs = Loadable::Loaded(all);
                    form.impact = PickLists::impacts()
                        .into_iter()
                        .find(|r| r.id == form.draft.impact)
                        .or(form.impact.take());
                    form.urgency = PickLists::urgencies()
                        .into_iter()
                        .find(|r| r.id == form.draft.urgency)
                        .or(form.urgency.take());
                    if org.is_none() {
                        form.restoring = false;
                    }
                    form.set_org(org);
                }
                Err(e) => {
                    form.orgs = Loadable::Failed(describe(&e, &host));
                    form.restoring = false;
                }
            }
        }
        self.run_pending(instance, generation);
        self.publish();
    }

    /// Starts the loads the last change queued.
    fn run_pending(&self, instance: NewTicketInstance, generation: u64) {
        let pending = {
            let mut st = self.lock();
            match st.form(instance, generation) {
                Some(form) => std::mem::take(&mut form.pending),
                None => return,
            }
        };
        for spawn in pending {
            let me = self.me.clone();
            tokio::spawn(async move {
                let Some(model) = me.upgrade() else { return };
                match spawn {
                    Spawn::ForOrg(org) => model.load_for_org(instance, generation, org).await,
                    Spawn::Subcategories(service) => {
                        model
                            .load_subcategories(instance, generation, service)
                            .await
                    }
                }
            });
        }
    }

    /// `loadForOrg(_:)` (W/NewTicketView.swift:196-221): callers and services together, callers
    /// applied first; late answers for another org are ignored.
    async fn load_for_org(&self, instance: NewTicketInstance, generation: u64, chosen: ServiceRef) {
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            form.callers = Loadable::Loading;
            form.services = Loadable::Loading;
        }
        self.publish();
        let host = text::host(&self.ctx.session);
        let client = self.ctx.session.client.clone();
        let catalog = ServiceCatalog::new(client.clone());
        let org_id = chosen.id.clone();
        let offered = tokio::spawn(async move { catalog.services_for_org(&org_id).await });
        let people = PickLists::new(client).callers(&chosen.id).await;
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                offered.abort();
                return;
            };
            match people {
                Ok(p) => {
                    if form.org.as_ref() != Some(&chosen) {
                        offered.abort();
                        return;
                    }
                    if form.restoring {
                        form.caller = p.iter().find(|c| c.id == form.draft.caller_id).cloned();
                    }
                    form.callers = Loadable::Loaded(p);
                }
                Err(e) => form.callers = Loadable::Failed(describe(&e, &host)),
            }
        }
        self.publish();
        let Ok(offered) = offered.await else { return };
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            match offered {
                Ok(s) => {
                    if form.org.as_ref() != Some(&chosen) {
                        return;
                    }
                    let restore = form
                        .restoring
                        .then(|| s.iter().find(|x| x.id == form.draft.service_id).cloned());
                    form.services = Loadable::Loaded(s);
                    if let Some(service) = restore {
                        let none = service.is_none();
                        form.set_service(service);
                        if none {
                            form.restoring = false;
                        }
                    }
                }
                Err(e) => {
                    form.services = Loadable::Failed(describe(&e, &host));
                    form.restoring = false;
                }
            }
        }
        self.run_pending(instance, generation);
        self.publish();
    }

    /// `loadSubcategories(_:)` (W/NewTicketView.swift:223-234).
    async fn load_subcategories(
        &self,
        instance: NewTicketInstance,
        generation: u64,
        chosen: ServiceRef,
    ) {
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            form.subcategories = Loadable::Loading;
        }
        self.publish();
        let host = text::host(&self.ctx.session);
        let found = ServiceCatalog::new(self.ctx.session.client.clone())
            .subcategories(&chosen.id)
            .await;
        {
            let mut st = self.lock();
            let Some(form) = st.form(instance, generation) else {
                return;
            };
            match found {
                Ok(found) => {
                    if form.service.as_ref() != Some(&chosen) {
                        return;
                    }
                    if form.restoring {
                        form.subcategory = found
                            .iter()
                            .find(|x| x.id == form.draft.subcategory_id)
                            .cloned();
                    }
                    form.subcategories = Loadable::Loaded(found);
                }
                Err(e) => form.subcategories = Loadable::Failed(describe(&e, &host)),
            }
            form.restoring = false;
        }
        self.publish();
    }

    // MARK: Intents

    /// Applies a change to the open form; a closed one ignores it.
    fn edit(
        &self,
        instance: NewTicketInstance,
        change: impl FnOnce(&mut Form) -> Result<(), AppError>,
    ) -> Result<Value, AppError> {
        let generation = {
            let mut st = self.lock();
            let Some(form) = st.slot(instance).as_mut() else {
                return Ok(Value::Null);
            };
            change(form)?;
            form.generation
        };
        self.run_pending(instance, generation);
        self.publish();
        Ok(Value::Null)
    }

    /// Closes an instance; the window's native window goes with it (NEW-4).
    fn close(&self, st: &mut Shared, instance: NewTicketInstance) -> bool {
        st.slot(instance).take().is_some() && instance == NewTicketInstance::Window
    }

    /// Cancel / Esc: `keepDraft()` then dismiss (NEW-21).
    fn cancel(&self, instance: NewTicketInstance) {
        let close_window = {
            let mut st = self.lock();
            let Some(form) = st.slot(instance).as_ref() else {
                return;
            };
            let d = form.filled();
            let empty = d.org_id.is_empty()
                && trim_whitespaces(&d.title).is_empty()
                && trim_whitespaces(&d.description).is_empty();
            st.kept = if empty { None } else { Some(d) };
            self.close(&mut st, instance)
        };
        if close_window {
            self.ctx.native.close_new_ticket_window();
        }
        self.publish();
    }

    /// The window's close button: no draft (NEW-21).
    fn dismiss(&self, instance: NewTicketInstance) {
        let close_window = {
            let mut st = self.lock();
            self.close(&mut st, instance)
        };
        if close_window {
            self.ctx.native.close_new_ticket_window();
        }
        self.publish();
    }

    /// "Clear form": `clear()` (W/NewTicketView.swift:256-263, NEW-23).
    fn clear_form(&self, instance: NewTicketInstance) {
        let generation = {
            let mut st = self.lock();
            st.kept = None;
            let Some(form) = st.slot(instance).as_mut() else {
                drop(st);
                self.publish();
                return;
            };
            form.draft = NewTicket::new(form.draft.ticket_class);
            form.restoring = false;
            form.set_org(None);
            form.caller = None;
            form.set_service(None);
            form.subcategory = None;
            form.impact = PickLists::impacts().first().cloned();
            form.urgency = PickLists::urgencies().last().cloned();
            form.tried = false;
            form.error = None;
            form.generation
        };
        self.run_pending(instance, generation);
        self.publish();
    }

    /// "Create ticket" (⌘↩): `create()` (W/NewTicketView.swift:265-283). Returns the error shown
    /// under the form, or null (created, a field missing, or creation off).
    async fn create(&self, instance: NewTicketInstance) -> Value {
        let client = self.ctx.session.client.clone();
        let (generation, filled) = {
            let mut st = self.lock();
            let Some(form) = st.slot(instance).as_mut() else {
                return Value::Null;
            };
            // The button is disabled while creating and when creation is off.
            if form.creating || !client.can_create_tickets() {
                return Value::Null;
            }
            form.tried = true;
            form.error = None;
            if form.first_missing().is_some() {
                drop(st);
                self.publish();
                return Value::Null;
            }
            form.creating = true;
            (form.generation, form.filled())
        };
        self.publish();
        let host = text::host(&self.ctx.session);
        match TicketWrites::new(client).create(&filled).await {
            Ok(ticket) => {
                let close_window = {
                    let mut st = self.lock();
                    st.kept = None;
                    if st.form(instance, generation).is_some() {
                        self.close(&mut st, instance)
                    } else {
                        false
                    }
                };
                if close_window {
                    self.ctx.native.close_new_ticket_window();
                }
                self.publish();
                // `opened(ticket, assignToMe)`: My tickets, then `workspace.created`.
                let assign = self.ctx.native.new_ticket_assign_to_me();
                self.link.select_section("myTickets").await;
                self.link.ticket_created(ticket, assign).await;
                Value::Null
            }
            Err(e) => {
                let text = action_error(&e, &host);
                {
                    let mut st = self.lock();
                    if let Some(form) = st.form(instance, generation) {
                        form.creating = false;
                        form.error = Some(text.clone());
                    }
                }
                self.publish();
                Value::String(text)
            }
        }
    }

    async fn handle(
        &self,
        instance: NewTicketInstance,
        intent: NewTicketIntent,
    ) -> Result<Value, AppError> {
        use NewTicketIntent as I;
        match intent {
            I::Open => {
                self.request(instance, NewTicketStart::default()).await;
                Ok(Value::Null)
            }
            I::SetClass { ticket_class } => self.edit(instance, |f| {
                f.draft.ticket_class = ticket_class;
                Ok(())
            }),
            I::SetOrganization { id } => self.edit(instance, |f| {
                let org = pick(f.orgs.value().map_or(&[][..], Vec::as_slice), id)?;
                f.set_org(org);
                Ok(())
            }),
            I::SetCaller { id } => self.edit(instance, |f| {
                let items = if f.org.is_some() {
                    f.callers.value().map_or(&[][..], Vec::as_slice)
                } else {
                    &[][..]
                };
                f.caller = pick(items, id)?;
                Ok(())
            }),
            I::SetService { id } => self.edit(instance, |f| {
                let items = if f.org.is_some() {
                    f.services.value().map_or(&[][..], Vec::as_slice)
                } else {
                    &[][..]
                };
                let service = pick(items, id)?;
                f.set_service(service);
                Ok(())
            }),
            I::SetSubcategory { id } => self.edit(instance, |f| {
                let items = if f.service.is_some() {
                    f.subcategories.value().map_or(&[][..], Vec::as_slice)
                } else {
                    &[][..]
                };
                f.subcategory = pick(items, id)?;
                Ok(())
            }),
            I::SetImpact { id } => self.edit(instance, |f| {
                f.impact = pick(&PickLists::impacts(), id)?;
                Ok(())
            }),
            I::SetUrgency { id } => self.edit(instance, |f| {
                f.urgency = pick(&PickLists::urgencies(), id)?;
                Ok(())
            }),
            I::SetTitle { title } => self.edit(instance, |f| {
                f.draft.title = title;
                Ok(())
            }),
            I::SetDescription { description } => self.edit(instance, |f| {
                f.draft.description = description;
                Ok(())
            }),
            I::SetAssignToMe { assign_to_me } => {
                self.ctx.native.set_new_ticket_assign_to_me(assign_to_me);
                self.publish();
                Ok(Value::Null)
            }
            I::ClearForm => {
                self.clear_form(instance);
                Ok(Value::Null)
            }
            I::Cancel => {
                self.cancel(instance);
                Ok(Value::Null)
            }
            I::Dismiss => {
                self.dismiss(instance);
                Ok(Value::Null)
            }
            I::Create => Ok(self.create(instance).await),
        }
    }

    // MARK: Snapshot

    fn build(&self, st: &Shared, instance: NewTicketInstance) -> NewTicketSnapshot {
        let form = match instance {
            NewTicketInstance::Sheet => st.sheet.as_ref(),
            NewTicketInstance::Window => st.window.as_ref(),
        };
        let Some(f) = form else {
            return NewTicketSnapshot::Closed;
        };
        let can_create = self.ctx.session.client.can_create_tickets();
        let org_name = f.org.as_ref().map(|o| o.name.as_str());
        let mark = |empty: bool| RequiredMark {
            missing: f.tried && empty,
        };
        let items = |list: Option<&Vec<ServiceRef>>| -> Vec<PickItem> {
            list.map_or_else(Vec::new, |l| {
                l.iter()
                    .map(|r| PickItem {
                        id: r.id.clone(),
                        label: r.name.clone(),
                    })
                    .collect()
            })
        };
        let id = |r: &Option<ServiceRef>| r.as_ref().map(|r| r.id.clone());
        let fixed = |list: Vec<ServiceRef>, selected: &Option<ServiceRef>| Dropdown {
            items: items(Some(&list)),
            selected_id: id(selected),
            placeholder: "Choose…".to_owned(),
            enabled: true,
            search: None,
        };
        let has_org = f.org.is_some();
        let has_service = f.service.is_some();
        NewTicketSnapshot::Open(Box::new(NewTicketOpen {
            subtitle: f.start.ci_name.as_ref().map(|n| format!("On {n}")),
            ticket_class: f.draft.ticket_class,
            organization: Dropdown {
                items: items(f.orgs.value()),
                selected_id: id(&f.org),
                placeholder: placeholder(
                    &f.orgs,
                    "Loading organizations…",
                    "No organizations found",
                ),
                enabled: f.orgs.value().is_some(),
                search: Some(DropdownSearch {
                    prompt: "Search organizations".to_owned(),
                    no_match: "No organization matches".to_owned(),
                }),
            },
            caller: Dropdown {
                items: if has_org {
                    items(f.callers.value())
                } else {
                    Vec::new()
                },
                selected_id: id(&f.caller),
                placeholder: if has_org {
                    placeholder(
                        &f.callers,
                        "Loading people…",
                        &format!("Nobody in {}", org_name.unwrap_or("this organization")),
                    )
                } else {
                    "Choose an organization first".to_owned()
                },
                enabled: has_org && f.callers.value().is_some(),
                search: Some(DropdownSearch {
                    prompt: format!(
                        "Search people in {}",
                        org_name.unwrap_or("the organization")
                    ),
                    no_match: format!(
                        "No caller in {} matches",
                        org_name.unwrap_or("this organization")
                    ),
                }),
            },
            service: Dropdown {
                items: if has_org {
                    items(f.services.value())
                } else {
                    Vec::new()
                },
                selected_id: id(&f.service),
                placeholder: if has_org {
                    placeholder(&f.services, "Loading…", "No services")
                } else {
                    "Choose an organization first".to_owned()
                },
                enabled: has_org && f.services.value().is_some(),
                search: None,
            },
            subcategory: Dropdown {
                items: if has_service {
                    items(f.subcategories.value())
                } else {
                    Vec::new()
                },
                selected_id: id(&f.subcategory),
                placeholder: if has_service {
                    placeholder(&f.subcategories, "Loading…", "None for this service")
                } else {
                    "Choose a service first".to_owned()
                },
                enabled: has_service && f.subcategories.value().is_some(),
                search: None,
            },
            impact: fixed(PickLists::impacts(), &f.impact),
            urgency: fixed(PickLists::urgencies(), &f.urgency),
            title: f.draft.title.clone(),
            description: f.draft.description.clone(),
            assign_to_me: self.ctx.native.new_ticket_assign_to_me(),
            required: NewTicketRequired {
                organization: mark(f.org.is_none()),
                caller: mark(f.caller.is_none()),
                service: mark(f.service.is_none()),
                subcategory: mark(f.subcategory.is_none()),
                title: mark(f.title_empty()),
                description: mark(f.description_empty()),
            },
            missing_text: if f.tried {
                f.first_missing().map(|m| format!("Fill in the {m} first."))
            } else {
                None
            },
            error: f.error.clone(),
            disabled_text: (!can_create).then(|| {
                "Creating tickets is switched off in this build, so nothing is sent to iTop."
                    .to_owned()
            }),
            creating: f.creating,
            // `.disabled(creating || !source.canCreate)`.
            confirm_enabled: !f.creating && can_create,
            show_clear_form: st.kept.is_some(),
        }))
    }
}

/// `placeholder(_:loading:empty:)` (W/NewTicketView.swift:165-171).
fn placeholder(state: &Loadable<Vec<ServiceRef>>, loading: &str, empty: &str) -> String {
    match state {
        Loadable::Idle | Loadable::Loading => loading.to_owned(),
        Loadable::Failed(message) => format!("Couldn't load: {message}"),
        Loadable::Loaded(items) if items.is_empty() => empty.to_owned(),
        Loadable::Loaded(_) => "Choose…".to_owned(),
    }
}

/// A dropdown choice by id: `None` clears it; an id the dropdown doesn't list is refused.
fn pick(items: &[ServiceRef], id: Option<String>) -> Result<Option<ServiceRef>, AppError> {
    match id {
        None => Ok(None),
        Some(id) => items
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .map(Some)
            .ok_or_else(|| AppError::invalid_intent(format!("No choice {id} in this dropdown."))),
    }
}

/// `TicketListModel.actionError(_:ticket:verb:)` (W/TicketListModel.swift:316-337) as New ticket
/// calls it: the ref is "the ticket", the verb "create the ticket" (NEW-17, ERR-14).
fn action_error(error: &WriteError, host: &str) -> String {
    let r = "the ticket";
    match error {
        WriteError::Failure(Failure::AlreadyTaken { by }) => format!("{by} took {r} a moment ago, so it stays with them."),
        WriteError::Failure(Failure::NoTeam) => {
            "You aren't in any team in iTop, so Assign to me can't pick one. Choose a team and agent instead.".to_owned()
        }
        WriteError::Failure(Failure::CannotAssign { status }) => format!("{} tickets can't be assigned.", status_label(status)),
        WriteError::Failure(Failure::InvalidChoice) => "Pick a team and an agent first.".to_owned(),
        WriteError::Failure(Failure::Missing(field)) => format!("Fill in the {field} first."),
        WriteError::Failure(Failure::NotAllowed { status }) => {
            format!("{} tickets can't do that. Refresh to see the ticket as it is now.", status_label(status))
        }
        WriteError::ITop(ITopError::WritesDisabled) => "Changes to iTop are switched off in this build, so nothing was sent.".to_owned(),
        other => {
            let mut reason = trim_whitespaces(&describe_write(other, host)).to_owned();
            if let Some(last) = reason.chars().last()
                && !".!?".contains(last)
            {
                reason.push('.');
            }
            format!("iTop didn't create the ticket. {reason}")
        }
    }
}

#[async_trait::async_trait]
impl Model for NewTickets {
    async fn dispatch(&self, model_id: &str, intent: Value) -> Result<Value, AppError> {
        let instance = NewTicketInstance::from_model_id(model_id)
            .ok_or_else(|| AppError::unknown_model(model_id))?;
        let intent: NewTicketIntent =
            serde_json::from_value(intent).map_err(AppError::invalid_intent)?;
        self.handle(instance, intent).await
    }
}
