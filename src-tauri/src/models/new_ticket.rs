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
    EditableTicket, ITopError, NewTicket, PickLists, ServiceCatalog, ServiceRef, Ticket,
    TicketClass, TicketWrites, WriteError,
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
/// Edit ticket (iTop Wrap addition): the same form, prefilled from a ticket, saving changes.
pub const EDIT_ID: &str = "editTicket";

/// Which form: New ticket's sheet, the menu bar's New ticket window, or Edit ticket's sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NewTicketInstance {
    Sheet,
    Window,
    Edit,
}

impl NewTicketInstance {
    pub fn model_id(self) -> &'static str {
        match self {
            NewTicketInstance::Sheet => SHEET_ID,
            NewTicketInstance::Window => WINDOW_ID,
            NewTicketInstance::Edit => EDIT_ID,
        }
    }

    pub fn from_model_id(id: &str) -> Option<Self> {
        match id {
            SHEET_ID => Some(NewTicketInstance::Sheet),
            WINDOW_ID => Some(NewTicketInstance::Window),
            EDIT_ID => Some(NewTicketInstance::Edit),
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
    /// Set for Edit ticket only.
    pub edit: Option<NewTicketEdit>,
}

/// What only Edit ticket shows (iTop Wrap addition).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct NewTicketEdit {
    /// "Edit R-000412".
    pub title: String,
    /// The ticket is still being read from iTop.
    pub loading: bool,
    /// The description has images or formatting: shown read-only, never sent.
    pub description_locked: bool,
    /// Said under a locked description.
    pub description_note: Option<String>,
    /// The ticket in iTop's web UI (to change a locked description there).
    pub web_url: String,
    /// Something differs from the ticket as read: Save changes is worth pressing.
    pub changed: bool,
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
    /// Edit ticket only.
    edit: Option<EditTarget>,
}

/// The ticket Edit ticket is changing, and its fields as read (to send only what changed).
struct EditTarget {
    ticket: Ticket,
    web_url: String,
    /// `None` until the ticket has been read.
    read: Option<EditableTicket>,
}

impl EditTarget {
    fn locked(&self) -> bool {
        self.read
            .as_ref()
            .is_some_and(EditableTicket::description_locked)
    }
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
            edit: None,
        }
    }

    /// Edit ticket's current value for a picker, as a choice, when the list doesn't offer it
    /// (an inactive caller, a service outside the org's contracts…): kept, inserted at the top.
    fn keep_current(
        &self,
        list: &mut Vec<ServiceRef>,
        id: &str,
        name: impl Fn(&EditableTicket) -> String,
    ) -> Option<ServiceRef> {
        if let Some(found) = list.iter().find(|r| r.id == id) {
            return Some(found.clone());
        }
        let read = self.edit.as_ref()?.read.as_ref()?;
        if !TicketWrites::is_id(id) {
            return None;
        }
        let current = ServiceRef::new(id.to_owned(), name(read));
        list.insert(0, current.clone());
        Some(current)
    }

    fn description_locked(&self) -> bool {
        self.edit.as_ref().is_some_and(EditTarget::locked)
    }

    /// `.whitespacesAndNewlines` (Unicode White_Space, what `trim` uses).
    fn title_empty(&self) -> bool {
        self.draft.title.trim().is_empty()
    }

    fn description_empty(&self) -> bool {
        !self.description_locked() && self.draft.description.trim().is_empty()
    }

    /// Edit ticket: whether anything differs from the ticket as read.
    fn changed(&self) -> bool {
        match self.edit.as_ref().and_then(|e| e.read.as_ref()) {
            Some(read) => !itop_kit::ticket_edit::edit_changes(
                &read.values,
                &self.filled(),
                read.description_locked(),
            )
            .is_empty(),
            None => false,
        }
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
    edit: Option<Form>,
    next_generation: u64,
}

impl Shared {
    fn slot(&mut self, instance: NewTicketInstance) -> &mut Option<Form> {
        match instance {
            NewTicketInstance::Sheet => &mut self.sheet,
            NewTicketInstance::Window => &mut self.window,
            NewTicketInstance::Edit => &mut self.edit,
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
                edit: None,
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
        for instance in [
            NewTicketInstance::Sheet,
            NewTicketInstance::Window,
            NewTicketInstance::Edit,
        ] {
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
            st.edit = None;
            st.window.take().is_some()
        };
        if had_window {
            self.ctx.native.close_new_ticket_window();
        }
        self.ctx.hub.clear(SHEET_ID);
        self.ctx.hub.clear(WINDOW_ID);
        self.ctx.hub.clear(EDIT_ID);
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

    /// Opens Edit ticket on `ticket`: reads its fields fresh from iTop, then loads the pickers
    /// with them selected. A request while it shows starts over on the new ticket.
    pub async fn request_edit(&self, ticket: Ticket, web_url: String) {
        let generation = {
            let mut st = self.lock();
            st.next_generation += 1;
            let generation = st.next_generation;
            let mut form = Form::new(generation, NewTicketStart::default());
            form.draft = NewTicket::new(ticket.ticket_class);
            form.edit = Some(EditTarget {
                ticket: ticket.clone(),
                web_url,
                read: None,
            });
            st.edit = Some(form);
            generation
        };
        self.publish();
        let host = text::host(&self.ctx.session);
        let answer = TicketWrites::new(self.ctx.session.client.clone())
            .editable(&ticket)
            .await;
        {
            let mut st = self.lock();
            let Some(form) = st.form(NewTicketInstance::Edit, generation) else {
                return;
            };
            match answer {
                Ok(read) => {
                    form.draft = read.values.clone();
                    if let Some(e) = form.edit.as_mut() {
                        e.read = Some(read);
                    }
                }
                Err(e) => {
                    form.orgs = Loadable::Failed(describe_write(&e, &host));
                    form.restoring = false;
                    form.error = Some(format!(
                        "Couldn't read {} from iTop. {}",
                        ticket.r#ref,
                        trim_whitespaces(&describe_write(&e, &host))
                    ));
                    drop(st);
                    self.publish();
                    return;
                }
            }
        }
        self.publish();
        self.load_orgs(NewTicketInstance::Edit, generation).await;
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
        self.load_orgs(instance, generation).await;
    }

    /// The organizations, then the chosen org's lists (the rest of `load()`).
    async fn load_orgs(&self, instance: NewTicketInstance, generation: u64) {
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
                Ok(mut all) => {
                    if form.edit.is_some() {
                        let id = form.draft.org_id.clone();
                        form.keep_current(&mut all, &id, |r| r.org_name.clone());
                    }
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
                Ok(mut p) => {
                    if form.org.as_ref() != Some(&chosen) {
                        offered.abort();
                        return;
                    }
                    if form.restoring {
                        let id = form.draft.caller_id.clone();
                        form.caller = if form.edit.is_some() {
                            form.keep_current(&mut p, &id, |r| r.caller_name.clone())
                        } else {
                            p.iter().find(|c| c.id == id).cloned()
                        };
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
                Ok(mut s) => {
                    if form.org.as_ref() != Some(&chosen) {
                        return;
                    }
                    let id = form.draft.service_id.clone();
                    let restore = if !form.restoring {
                        None
                    } else if form.edit.is_some() {
                        Some(form.keep_current(&mut s, &id, |r| r.service_name.clone()))
                    } else {
                        Some(s.iter().find(|x| x.id == id).cloned())
                    };
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
                Ok(mut found) => {
                    if form.service.as_ref() != Some(&chosen) {
                        return;
                    }
                    if form.restoring {
                        let id = form.draft.subcategory_id.clone();
                        form.subcategory = if form.edit.is_some() {
                            form.keep_current(&mut found, &id, |r| r.subcategory_name.clone())
                        } else {
                            found.iter().find(|x| x.id == id).cloned()
                        };
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
        if instance == NewTicketInstance::Edit {
            // Edit keeps no draft: cancelling drops the changes.
            self.dismiss(instance);
            return;
        }
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
        if instance == NewTicketInstance::Edit {
            return self.save_edit().await;
        }
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

    /// Edit ticket's "Save changes" (⌘↩): sends only what changed. Returns the error shown under
    /// the form, or null (saved, nothing changed, or a field missing).
    async fn save_edit(&self) -> Value {
        let instance = NewTicketInstance::Edit;
        let client = self.ctx.session.client.clone();
        let (generation, ticket, before, after, locked) = {
            let mut st = self.lock();
            let Some(form) = st.slot(instance).as_mut() else {
                return Value::Null;
            };
            let Some((ticket, read)) = form
                .edit
                .as_ref()
                .and_then(|e| e.read.clone().map(|r| (e.ticket.clone(), r)))
            else {
                return Value::Null;
            };
            if form.creating || !client.can_write(ticket.ticket_class.raw_value(), &ticket.key) {
                return Value::Null;
            }
            form.tried = true;
            form.error = None;
            if form.first_missing().is_some() || !form.changed() {
                drop(st);
                self.publish();
                return Value::Null;
            }
            form.creating = true;
            let locked = read.description_locked();
            (form.generation, ticket, read.values, form.filled(), locked)
        };
        self.publish();
        let host = text::host(&self.ctx.session);
        match TicketWrites::new(client)
            .save_edit(&ticket, &before, &after, locked)
            .await
        {
            Ok(_) => {
                {
                    let mut st = self.lock();
                    if st.form(instance, generation).is_some() {
                        st.edit = None;
                    }
                }
                self.publish();
                self.link.ticket_edited(ticket).await;
                Value::Null
            }
            Err(e) => {
                let text = edit_error(&e, &ticket, &host);
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
        if instance == NewTicketInstance::Edit
            && matches!(
                intent,
                I::Open | I::SetClass { .. } | I::SetAssignToMe { .. } | I::ClearForm
            )
        {
            return Err(AppError::invalid_intent(
                "Edit ticket can't change the type, assign, clear or open on its own.",
            ));
        }
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
            NewTicketInstance::Edit => st.edit.as_ref(),
        };
        let Some(f) = form else {
            return NewTicketSnapshot::Closed;
        };
        let can_create = match f.edit.as_ref() {
            Some(e) => self
                .ctx
                .session
                .client
                .can_write(e.ticket.ticket_class.raw_value(), &e.ticket.key),
            None => self.ctx.session.client.can_create_tickets(),
        };
        let edit = f.edit.as_ref().map(|e| NewTicketEdit {
            title: format!("Edit {}", e.ticket.r#ref),
            loading: e.read.is_none() && f.error.is_none(),
            description_locked: e.locked(),
            description_note: e.locked().then(|| {
                "This description has images or formatting, so it can only be changed in iTop on the web."
                    .to_owned()
            }),
            web_url: e.web_url.clone(),
            changed: f.changed(),
        });
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
                if f.edit.is_some() {
                    "Changes to this ticket are switched off in this build, so nothing is sent to iTop."
                } else {
                    "Creating tickets is switched off in this build, so nothing is sent to iTop."
                }
                .to_owned()
            }),
            creating: f.creating,
            // `.disabled(creating || !source.canCreate)`; Edit also needs something changed.
            confirm_enabled: !f.creating
                && can_create
                && edit.as_ref().is_none_or(|e| e.changed && !e.loading),
            show_clear_form: f.edit.is_none() && st.kept.is_some(),
            edit,
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

/// Edit ticket's errors, phrased like the other ticket changes (ERR-14).
fn edit_error(error: &WriteError, ticket: &Ticket, host: &str) -> String {
    match error {
        WriteError::Failure(Failure::Missing(field)) => format!("Fill in the {field} first."),
        WriteError::Failure(Failure::NotAllowed { status }) => {
            format!("{} tickets can't be edited.", status_label(status))
        }
        WriteError::ITop(ITopError::WritesDisabled) => {
            "Changes to iTop are switched off in this build, so nothing was sent.".to_owned()
        }
        other => {
            let mut reason = trim_whitespaces(&describe_write(other, host)).to_owned();
            if let Some(last) = reason.chars().last()
                && !".!?".contains(last)
            {
                reason.push('.');
            }
            format!("iTop didn't save the changes to {}. {reason}", ticket.r#ref)
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
