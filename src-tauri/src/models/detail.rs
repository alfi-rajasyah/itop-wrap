//! Port of `TicketDetailModel.swift` + what `TicketDetailView`, `LogComposer`, `AttachmentsView`
//! and `TimerButton` derive (DET-*, LOG-*, TMR-19).
//!
//! Swift makes one `TicketDetailModel` per shown ticket (`TicketDetailView(...).id(t.id)` in the
//! detail column of the shown list). Here each list has one detail state, reset whenever the
//! ticket it shows changes or its list's section comes back on screen.

use std::time::Duration;

use base64::Engine;
use itop_kit::{
    AttachmentRef, LogImage, LogImage as Img, TicketDetail, TicketLifecycle, TicketAction, TicketChange, TimeFormat,
    Ticket as KitTicket, TicketList, TicketWrites, WriteError, ITopError,
    log_image::Failure as ImageFailure, ticket_writes::Failure,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::format::byte_count;
use super::list::ListState;
use super::records::{
    AgentChip, ConfirmText, DraftImage, ImageIntakeItem, LinkedCI, ListScope, LogEntryView, LogTab, Notice, RichContent,
    Ticket, rich_content, ticket_view, with_period,
};
use super::workspace::{State, Workspace};
use crate::error::AppError;
use crate::native::ConvertTo;

/// Files bigger than this aren't read at all (`ImageIntake.maxFileBytes`, LOG-16).
pub const MAX_INTAKE_FILE_BYTES: i64 = 40 * 1_048_576;

#[derive(Clone, Debug, Default)]
pub enum DetailPhase {
    #[default]
    Loading,
    Loaded(Box<TicketDetail>),
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub struct AttachmentsState {
    pub started: bool,
    pub items: Option<Vec<AttachmentRef>>,
    pub error: Option<String>,
    pub saving: Option<String>,
    pub saved: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DetailState {
    pub instance: u64,
    /// `model.ticket`: the copy the detail opened with, updated before each load. `None`: no
    /// ticket shown.
    pub ticket: Option<KitTicket>,
    /// The `.task(id:)` key of the last load: status, agent, team and last update (DET-2).
    pub loaded_key: Option<String>,
    pub phase: DetailPhase,
    pub tab: LogTab,
    pub copied: bool,
    pub sending: bool,
    pub send_error: Option<String>,
    pub attachments: AttachmentsState,
}

impl Default for DetailState {
    fn default() -> Self {
        DetailState {
            instance: 0,
            ticket: None,
            loaded_key: None,
            phase: DetailPhase::Loading,
            tab: LogTab::Public,
            copied: false,
            sending: false,
            send_error: None,
            attachments: AttachmentsState::default(),
        }
    }
}

impl DetailState {
    pub fn loaded(&self) -> Option<&TicketDetail> {
        match &self.phase {
            DetailPhase::Loaded(d) => Some(d),
            _ => None,
        }
    }

    fn key(&self) -> Option<String> {
        self.ticket.as_ref().map(|t| ListState::draft_key(&t.id(), self.tab == LogTab::Private))
    }
}

fn reload_key(t: &KitTicket) -> String {
    format!(
        "{}|{}|{}|{}",
        t.status,
        t.agent_id,
        t.team_id,
        t.last_update.map_or(0, |d| d.timestamp())
    )
}

/// Initials of an agent: first letters of the first two words (W/TicketDetailView.swift:280).
pub fn name_initials(name: &str) -> String {
    name.split(' ').filter(|w| !w.is_empty()).take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

// MARK: Snapshot types

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TimerButton {
    None,
    Running,
    WebStopwatch { label: String },
    Start { enabled: bool, help: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PrimaryKind {
    AssignToMe,
    OpenAssign,
    OpenResolve,
    ConfirmClose,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PrimaryAction {
    None,
    Action { kind: PrimaryKind, label: String, help: String, teal: bool, enabled: bool, busy: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct MoreAssign {
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct MoreMenu {
    pub assign: Option<MoreAssign>,
    pub pending: bool,
    pub reopen: bool,
    pub enabled: bool,
    pub divider: bool,
    pub web_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct DetailStrip {
    pub caller: String,
    pub organization: String,
    pub team: String,
    pub agent: AgentChip,
    pub spent: Option<String>,
    #[serde(rename = "linkedCIs")]
    #[ts(rename = "linkedCIs")]
    pub linked_cis: Vec<LinkedCI>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum DetailBody {
    Loading,
    Failed {
        text: String,
    },
    Loaded {
        #[serde(rename = "serviceLine")]
        #[ts(rename = "serviceLine")]
        service_line: Option<String>,
        description: Option<RichContent>,
        tab: LogTab,
        entries: Vec<LogEntryView>,
        #[serde(rename = "emptyLogText")]
        #[ts(rename = "emptyLogText")]
        empty_log_text: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct AttachmentItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum Attachments {
    Loading,
    Failed {
        text: String,
    },
    Loaded {
        items: Vec<AttachmentItem>,
        #[serde(rename = "downloadingId")]
        #[ts(rename = "downloadingId")]
        downloading_id: Option<String>,
        saved: Option<String>,
        error: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct Composer {
    pub is_private: bool,
    pub draft: String,
    pub images: Vec<DraftImage>,
    pub images_accessibility_label: String,
    pub placeholder: String,
    pub accessibility_label: String,
    pub send_label: String,
    pub send_help: String,
    pub editable: bool,
    pub can_send: bool,
    pub sending: bool,
    pub can_add_images: bool,
    pub paperclip_help: String,
    pub status: Option<Notice>,
    pub max_intake_file_bytes: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TicketDetailView {
    pub ticket: Ticket,
    pub class_label: String,
    pub copied: bool,
    pub timer_button: TimerButton,
    pub primary: PrimaryAction,
    pub more: MoreMenu,
    pub close_confirm: ConfirmText,
    pub strip: DetailStrip,
    pub body: DetailBody,
    pub attachments: Attachments,
    pub composer: Option<Composer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum DetailSnapshot {
    None { placeholder: String },
    Ticket(Box<TicketDetailView>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum DetailIntent {
    Retry,
    SetTab {
        tab: LogTab,
    },
    CopyRef,
    AssignToMe,
    Reopen,
    Close,
    ToggleTimer,
    SetDraft {
        text: String,
    },
    AddImages {
        items: Vec<ImageIntakeItem>,
    },
    RemoveImage {
        #[serde(rename = "imageId")]
        #[ts(rename = "imageId")]
        image_id: String,
    },
    Send,
    DownloadAttachment {
        #[serde(rename = "attachmentId")]
        #[ts(rename = "attachmentId")]
        attachment_id: String,
    },
}

/// One pasted, dropped or chosen file (`ImageIntake.Item`).
pub struct IntakeItem {
    pub data: Vec<u8>,
    pub filename: String,
    pub oversize: bool,
}

/// `(name as NSString).deletingPathExtension` for a plain file name.
fn deleting_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[..i],
        _ => name,
    }
}

fn is_tiff(data: &[u8]) -> bool {
    data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
}

impl Workspace {
    /// What SwiftUI's view identity and `.task(id:)` modifiers did for the detail column: a new
    /// model when the shown ticket changes (or its list comes back on screen), a load when it
    /// opens and whenever the list's copy changes status, agent, team or last update (DET-2), and
    /// a web stopwatch check when it opens (TMR-15).
    pub(crate) fn sync_details(&self, st: &mut State) {
        for scope in ListScope::ALL {
            let i = scope.index();
            let active = st.shell.selection == scope.section();
            let selected = if active {
                let l = &st.lists[i];
                l.ticket(l.selection.as_deref()).cloned()
            } else {
                None
            };
            let Some(t) = selected else {
                if st.details[i].ticket.is_some() {
                    st.details[i] = DetailState::default();
                    self.close_sheets(st, scope);
                    if let Some(h) = self.tasks().detail_load[i].take() {
                        h.abort();
                    }
                }
                continue;
            };
            if st.details[i].ticket.as_ref().map(KitTicket::id) != Some(t.id()) {
                let instance = st.next_instance;
                st.next_instance += 1;
                st.details[i] = DetailState { instance, ticket: Some(t.clone()), ..DetailState::default() };
                self.close_sheets(st, scope);
                let ws = self.clone();
                let opened = t.clone();
                self.spawn(async move { ws.timer_check_web_stopwatch(&opened).await });
            }
            let key = reload_key(&t);
            let d = &mut st.details[i];
            if d.loaded_key.as_deref() != Some(key.as_str()) {
                d.loaded_key = Some(key);
                d.ticket = Some(t);
                let instance = d.instance;
                let ws = self.clone();
                let handle = self.spawn(async move { ws.detail_load(scope, instance).await });
                if let Some(old) = self.tasks().detail_load[i].replace(handle) {
                    old.abort();
                }
            }
        }
    }

    /// `load()`: a failed reload keeps what was loaded (DET-5).
    pub(crate) async fn detail_load(&self, scope: ListScope, instance: u64) {
        let i = scope.index();
        let t = {
            let mut st = self.lock();
            let d = &mut st.details[i];
            if d.instance != instance {
                return;
            }
            if d.loaded().is_none() {
                d.phase = DetailPhase::Loading;
            }
            let Some(t) = d.ticket.clone() else { return };
            t
        };
        self.trace(|| format!("detail: loading {}", t.id()));
        let result = self.0.tickets.detail(&t).await;
        let start_attachments = {
            let mut st = self.lock();
            let d = &mut st.details[i];
            if d.instance != instance {
                return;
            }
            match result {
                Ok(detail) => d.phase = DetailPhase::Loaded(Box::new(detail)),
                Err(e) => {
                    if d.loaded().is_none() {
                        d.phase = DetailPhase::Failed(self.describe(&e));
                    }
                }
            }
            // `AttachmentsView` appears with the description, once loaded.
            let start = d.loaded().is_some() && !d.attachments.started;
            if start {
                d.attachments.started = true;
            }
            start
        };
        self.publish();
        if start_attachments {
            let ws = self.clone();
            let ticket = t.clone();
            self.spawn(async move { ws.detail_attachments(scope, instance, ticket).await });
        }
    }

    async fn detail_attachments(&self, scope: ListScope, instance: u64, t: KitTicket) {
        let result = self.0.images.attachments(&t).await;
        {
            let mut st = self.lock();
            let d = &mut st.details[scope.index()];
            if d.instance != instance {
                return;
            }
            match result {
                Ok(items) => d.attachments.items = Some(items),
                Err(e) => d.attachments.error = Some(self.describe(&e)),
            }
        }
        self.publish();
    }

    pub(crate) fn detail_snapshot(&self, st: &State, scope: ListScope) -> DetailSnapshot {
        let i = scope.index();
        let d = &st.details[i];
        let l = &st.lists[i];
        let Some(model_ticket) = &d.ticket else {
            return DetailSnapshot::None { placeholder: "Select a ticket to see its details.".to_owned() };
        };
        let me = self.me();
        // The list's latest copy (DET-1).
        let t = l.ticket(Some(&model_ticket.id())).unwrap_or(model_ticket);
        let id = t.id();
        let can_write = self.can_write(model_ticket);
        let needs_you = TicketList::needs_you(t, me);
        let web_url = self.web_url(t);

        let timer = &st.timer;
        let timer_button = if timer.running.as_ref().is_some_and(|r| r.ticket_id() == id) {
            TimerButton::Running
        } else if let Some(since) = timer.web_stopwatches.get(&id) {
            TimerButton::WebStopwatch { label: format!("Web stopwatch since {}", self.0.fmt.since(*since, self.now())) }
        } else if timer.can_start(t, me) {
            TimerButton::Start {
                enabled: can_write && !timer.saving,
                help: if can_write {
                    timer.running.as_ref().map_or_else(
                        || "Start timer (⌘T)".to_owned(),
                        |r| format!("Start timer: stops the one on {} and saves its time (⌘T)", r.r#ref),
                    )
                } else {
                    "Changes to iTop are switched off in this build".to_owned()
                },
            }
        } else {
            TimerButton::None
        };

        let disabled = l.working.is_some() || !can_write;
        let help_off = (!can_write).then(|| "Changes to iTop are switched off in this build".to_owned());
        let busy = l.working.as_deref() == Some(id.as_str());
        let action = |kind, label: &str, teal, help: String| PrimaryAction::Action {
            kind,
            label: label.to_owned(),
            help: help_off.clone().unwrap_or(help),
            teal,
            enabled: !disabled,
            busy,
        };
        let primary_action = TicketLifecycle::primary(&t.status);
        let primary = match primary_action {
            Some(TicketAction::Assign) if t.is_unassigned() => {
                action(PrimaryKind::AssignToMe, "Assign to me", needs_you, format!("Assign {} to you", t.r#ref))
            }
            Some(TicketAction::Assign) => {
                action(PrimaryKind::OpenAssign, "Assign ticket", needs_you, "Choose a team and agent".to_owned())
            }
            Some(TicketAction::Resolve) => {
                action(PrimaryKind::OpenResolve, "Resolve ticket", needs_you, format!("Resolve {}", t.r#ref))
            }
            Some(TicketAction::Close) => {
                action(PrimaryKind::ConfirmClose, "Close ticket", false, format!("Close {}", t.r#ref))
            }
            _ => PrimaryAction::None,
        };

        let actions = TicketLifecycle::actions(&t.status);
        let can_assign = TicketWrites::assign_stimulus(&t.status).is_some();
        let more = MoreMenu {
            assign: (can_assign && (primary_action != Some(TicketAction::Assign) || t.is_unassigned())).then(|| {
                MoreAssign {
                    label: if t.is_unassigned() { "Assign to someone else…" } else { "Reassign…" }.to_owned(),
                }
            }),
            pending: actions.contains(&TicketAction::Pending),
            reopen: actions.contains(&TicketAction::Reopen),
            enabled: can_write && l.working.is_none(),
            divider: !actions.is_empty(),
            web_url: web_url.clone(),
        };

        let mine = t.agent_id == me.contact_id;
        let strip = DetailStrip {
            caller: if t.caller_name.is_empty() { "None".to_owned() } else { t.caller_name.clone() },
            organization: if t.org_name.is_empty() { "None".to_owned() } else { t.org_name.clone() },
            team: if t.team_name.is_empty() { "No team".to_owned() } else { t.team_name.clone() },
            agent: if t.is_unassigned() {
                AgentChip::Unassigned
            } else {
                AgentChip::Person {
                    initials: if mine { me.initials() } else { name_initials(&t.agent_name) },
                    name: if mine { "You".to_owned() } else { t.agent_name.clone() },
                }
            },
            spent: match timer.totals.get(&id) {
                Some(&s) if s > 0 => Some(format!("{} spent", TimeFormat::total(s))),
                _ => None,
            },
            linked_cis: d
                .loaded()
                .map(|det| {
                    det.linked_cis
                        .iter()
                        .map(|ci| LinkedCI { key: ci.id.clone(), class_name: ci.class_name.clone(), name: ci.name.clone() })
                        .collect()
                })
                .unwrap_or_default(),
        };

        let base = self.base_url();
        let body = match &d.phase {
            DetailPhase::Loading => DetailBody::Loading,
            DetailPhase::Failed(m) => DetailBody::Failed { text: format!("Couldn't load this ticket. {m}") },
            DetailPhase::Loaded(det) => {
                let is_public = d.tab == LogTab::Public;
                let entries = if is_public { &det.public_log } else { &det.private_log };
                let now = self.now();
                DetailBody::Loaded {
                    service_line: (!det.service_name.is_empty()).then(|| {
                        [det.service_name.as_str(), det.subcategory_name.as_str()]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" › ")
                    }),
                    description: if det.description.is_empty() && !det.description_html.contains("<img") {
                        None
                    } else {
                        Some(rich_content(Some(&det.description_html), &det.description, base, Some(&web_url)))
                    },
                    tab: d.tab,
                    entries: entries
                        .iter()
                        .map(|e| LogEntryView {
                            id: e.id,
                            header: [Some(e.author.clone()), e.date.map(|dt| self.0.fmt.log_date(dt, now))]
                                .into_iter()
                                .flatten()
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join(" · "),
                            is_private: !is_public,
                            content: rich_content(e.html.as_deref(), &e.text, base, Some(&web_url)),
                        })
                        .collect(),
                    empty_log_text: entries.is_empty().then(|| {
                        if is_public {
                            "No public entries yet.".to_owned()
                        } else {
                            "No private entries yet. Only agents can see the private log.".to_owned()
                        }
                    }),
                }
            }
        };

        let a = &d.attachments;
        let attachments = match &a.items {
            Some(items) => Attachments::Loaded {
                items: items.iter().map(|x| AttachmentItem { id: x.id.clone(), name: x.name.clone() }).collect(),
                downloading_id: a.saving.clone(),
                saved: a.saved.clone(),
                error: a.error.clone(),
            },
            None => match &a.error {
                Some(e) => Attachments::Failed { text: format!("Couldn't list attachments. {e}") },
                None => Attachments::Loading,
            },
        };

        let composer = d.loaded().map(|_| self.composer(d, l, can_write));

        DetailSnapshot::Ticket(Box::new(TicketDetailView {
            ticket: ticket_view(t, me),
            class_label: match t.ticket_class {
                itop_kit::TicketClass::UserRequest => "User request",
                itop_kit::TicketClass::Incident => "Incident",
            }
            .to_owned(),
            copied: d.copied,
            timer_button,
            primary,
            more,
            close_confirm: ConfirmText {
                title: format!("Close {}?", t.r#ref),
                message: "A closed ticket can't be reopened from the app.".to_owned(),
            },
            strip,
            body,
            attachments,
            composer,
        }))
    }

    fn composer(&self, d: &DetailState, l: &ListState, can_write: bool) -> Composer {
        let is_private = d.tab == LogTab::Private;
        let key = d.key().unwrap_or_default();
        let draft = l.drafts.get(&key).cloned().unwrap_or_default();
        let images = l.draft_images.get(&key).cloned().unwrap_or_default();
        let can_send = can_write && !d.sending && (!draft.trim().is_empty() || !images.is_empty());
        let can_add = can_write && !d.sending && images.len() < Img::MAX_PER_ENTRY;
        let status = if let Some(e) = &d.send_error {
            Some(Notice::error(e.clone()))
        } else if d.sending && !images.is_empty() {
            Some(Notice::ok(if images.len() == 1 {
                "Uploading the image to iTop".to_owned()
            } else {
                format!("Uploading {} images to iTop", images.len())
            }))
        } else if !can_write {
            Some(Notice::ok("Adding entries is switched off in this build, so nothing is sent to iTop."))
        } else {
            None
        };
        Composer {
            is_private,
            images_accessibility_label: if images.len() == 1 {
                "1 image to add".to_owned()
            } else {
                format!("{} images to add", images.len())
            },
            images: images
                .iter()
                .map(|im| DraftImage {
                    id: im.id.clone(),
                    filename: im.filename.clone(),
                    size_text: byte_count(im.data.len() as u64),
                    thumbnail_src: format!("itop://localhost/draft-image/{}", im.id),
                })
                .collect(),
            draft,
            placeholder: if is_private {
                "Write a private entry, only agents see it"
            } else {
                "Write a public entry, the caller sees it"
            }
            .to_owned(),
            accessibility_label: if is_private { "Private entry" } else { "Public entry" }.to_owned(),
            send_label: if is_private { "Add to private log" } else { "Add to public log" }.to_owned(),
            send_help: if can_write { "Add the entry (⌘↩)" } else { "Changes to iTop are switched off in this build" }
                .to_owned(),
            editable: can_write,
            can_send,
            sending: d.sending,
            can_add_images: can_add,
            paperclip_help: if can_add {
                format!(
                    "Add images. You can also paste or drop them in the entry. PNG, JPEG or GIF, up to {} MB each.",
                    Img::MAX_BYTES / 1_048_576
                )
            } else if images.len() >= Img::MAX_PER_ENTRY {
                format!("An entry can have up to {} images", Img::MAX_PER_ENTRY)
            } else {
                "Adding images is switched off".to_owned()
            },
            status,
            max_intake_file_bytes: MAX_INTAKE_FILE_BYTES,
        }
    }

    // MARK: Intents

    /// The detail's ticket as the views read it: the list's latest copy (DET-1).
    pub(crate) fn detail_ticket(&self, st: &State, scope: ListScope) -> Option<KitTicket> {
        let i = scope.index();
        let model = st.details[i].ticket.as_ref()?;
        Some(st.lists[i].ticket(Some(&model.id())).unwrap_or(model).clone())
    }

    pub(crate) async fn dispatch_detail(&self, scope: ListScope, intent: DetailIntent) -> Result<serde_json::Value, AppError> {
        let i = scope.index();
        let (ticket, instance) = {
            let st = self.lock();
            (self.detail_ticket(&st, scope), st.details[i].instance)
        };
        let Some(ticket) = ticket else {
            return Err(AppError::invalid_intent("No ticket is shown in this detail."));
        };
        match intent {
            DetailIntent::Retry => self.detail_load(scope, instance).await,
            DetailIntent::SetTab { tab } => {
                self.lock().details[i].tab = tab;
                self.publish();
            }
            DetailIntent::CopyRef => self.detail_copy_ref(scope, instance, &ticket),
            DetailIntent::AssignToMe => self.list_assign_to_me(scope, &ticket).await,
            DetailIntent::Reopen => self.list_apply_from_header(scope, &TicketChange::Reopen, &ticket).await,
            DetailIntent::Close => self.list_apply_from_header(scope, &TicketChange::Close, &ticket).await,
            DetailIntent::ToggleTimer => {
                let snapshot = {
                    let st = self.lock();
                    self.detail_snapshot(&st, scope)
                };
                if let DetailSnapshot::Ticket(view) = snapshot {
                    match view.timer_button {
                        TimerButton::Running => self.timer_stop(None).await,
                        TimerButton::Start { enabled: true, .. } => self.timer_start(&ticket).await,
                        _ => {}
                    }
                }
            }
            DetailIntent::SetDraft { text } => {
                {
                    let mut st = self.lock();
                    if let Some(key) = st.details[i].key() {
                        st.lists[i].drafts.insert(key, text);
                    }
                }
                self.publish();
            }
            DetailIntent::AddImages { items } => {
                let items = items.into_iter().map(intake_item).collect();
                self.detail_add_images(scope, items);
            }
            DetailIntent::RemoveImage { image_id } => {
                let found = {
                    let mut st = self.lock();
                    let key = st.details[i].key().unwrap_or_default();
                    let images = st.lists[i].draft_images.get(&key).cloned().unwrap_or_default();
                    if images.iter().any(|im| im.id == image_id) {
                        let rest: Vec<LogImage> = images.into_iter().filter(|im| im.id != image_id).collect();
                        if rest.is_empty() {
                            st.lists[i].draft_images.remove(&key);
                        } else {
                            st.lists[i].draft_images.insert(key, rest);
                        }
                        st.details[i].send_error = None;
                        true
                    } else {
                        false
                    }
                };
                if !found {
                    return Err(AppError::invalid_intent(format!("No image {image_id} in this entry.")));
                }
                self.publish();
            }
            DetailIntent::Send => return Ok(serde_json::Value::Bool(self.detail_send(scope).await)),
            DetailIntent::DownloadAttachment { attachment_id } => {
                let found = {
                    let st = self.lock();
                    st.details[i]
                        .attachments
                        .items
                        .as_ref()
                        .and_then(|items| items.iter().find(|a| a.id == attachment_id).cloned())
                };
                let Some(a) = found else {
                    return Err(AppError::invalid_intent(format!("No attachment {attachment_id} on this ticket.")));
                };
                self.detail_download(scope, instance, a).await;
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// `copyRef`: the ref on the clipboard and "Copied" for 2 s (DET-10).
    fn detail_copy_ref(&self, scope: ListScope, instance: u64, t: &KitTicket) {
        self.0.ctx.native.copy_text(&t.r#ref);
        self.lock().details[scope.index()].copied = true;
        self.publish();
        let ws = self.clone();
        self.spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            {
                let mut st = ws.lock();
                let d = &mut st.details[scope.index()];
                if d.instance == instance {
                    d.copied = false;
                }
            }
            ws.publish();
        });
    }

    /// `LogComposer.add` + `intake(_:existing:)` (LOG-16…19).
    pub(crate) fn detail_add_images(&self, scope: ListScope, items: Vec<IntakeItem>) {
        let i = scope.index();
        let existing = {
            let st = self.lock();
            let key = st.details[i].key().unwrap_or_default();
            st.lists[i].draft_images.get(&key).map_or(0, Vec::len)
        };
        let mut accepted: Vec<LogImage> = Vec::new();
        let mut problems: Vec<String> = Vec::new();
        for item in items {
            match self.log_image(&item) {
                Ok(image) => accepted.push(image),
                Err(ImageFailure::TooLarge { filename, max_mb }) => problems.push(format!("{filename} is larger than {max_mb} MB.")),
                Err(ImageFailure::UnsupportedType { .. }) => problems.push(format!(
                    "{} isn't an image iTop Wrap can add. Use PNG, JPEG or GIF.",
                    if item.filename.is_empty() { "That file" } else { item.filename.as_str() }
                )),
            }
        }
        let room = Img::MAX_PER_ENTRY.saturating_sub(existing);
        if accepted.len() > room {
            let left = accepted.len() - room;
            problems.push(format!(
                "An entry can have up to {} images, so {left} {} added.",
                Img::MAX_PER_ENTRY,
                if left == 1 { "wasn't" } else { "weren't" }
            ));
            accepted.truncate(room);
        }
        {
            let mut st = self.lock();
            st.details[i].send_error = if problems.is_empty() { None } else { Some(problems.join(" ")) };
            if !accepted.is_empty() {
                let key = st.details[i].key().unwrap_or_default();
                st.lists[i].draft_images.entry(key).or_default().extend(accepted);
            }
        }
        self.publish();
    }

    /// `ImageIntake.logImage(_:)`: PNG, JPEG and GIF as they are; other images macOS can read are
    /// converted, TIFF to PNG and the rest to JPEG (LOG-17, LOG-18).
    fn log_image(&self, item: &IntakeItem) -> Result<LogImage, ImageFailure> {
        if item.oversize {
            return Err(ImageFailure::TooLarge {
                filename: item.filename.clone(),
                max_mb: (Img::MAX_BYTES / 1_048_576) as i64,
            });
        }
        match LogImage::new(item.data.clone(), &item.filename) {
            Err(ImageFailure::UnsupportedType { .. }) => {
                let tiff = is_tiff(&item.data);
                let converted = self
                    .0
                    .ctx
                    .native
                    .convert_image(&item.data, if tiff { ConvertTo::Png } else { ConvertTo::Jpeg })
                    .ok_or_else(|| ImageFailure::UnsupportedType { filename: item.filename.clone() })?;
                let base = deleting_extension(&item.filename);
                let name = if base.is_empty() { "Pasted image" } else { base };
                LogImage::new(converted.data, &format!("{name}.{}", converted.ext))
            }
            other => other,
        }
    }

    /// `send(_:images:)` + clearing the sent key (LOG-5…9, LOG-22).
    async fn detail_send(&self, scope: ListScope) -> bool {
        let i = scope.index();
        let prepared = {
            let mut st = self.lock();
            let (key, private, t, instance) = {
                let d = &st.details[i];
                let (Some(key), Some(t)) = (d.key(), d.ticket.clone()) else { return false };
                (key, d.tab == LogTab::Private, t, d.instance)
            };
            let text = st.lists[i].drafts.get(&key).cloned().unwrap_or_default();
            let images = st.lists[i].draft_images.get(&key).cloned().unwrap_or_default();
            let can_write = self.can_write(&t);
            let d = &mut st.details[i];
            let can_send = can_write && !d.sending && (!text.trim().is_empty() || !images.is_empty());
            if !can_send {
                return false;
            }
            d.sending = true;
            d.send_error = None;
            (key, private, t, instance, text, images)
        };
        let (key, private, t, instance, text, images) = prepared;
        self.trace(|| format!("detail: adding an entry with {} images to {}", images.len(), t.id()));
        self.publish();
        let result = self.0.writes.add_log_entry_with_images(&t, &text, &images, private).await;
        let ok = match result {
            Ok(()) => {
                self.detail_load(scope, instance).await;
                true
            }
            Err(e) => {
                let message = match &e {
                    WriteError::Failure(Failure::EmptyEntry) => "Write something or add an image first.".to_owned(),
                    WriteError::Failure(Failure::TooManyImages { max }) => {
                        format!("An entry can have up to {max} images. Remove some, then add the entry. Your draft is kept.")
                    }
                    WriteError::ITop(ITopError::WritesDisabled) => {
                        "Changes to iTop are switched off in this build, so nothing was sent. Your draft is kept.".to_owned()
                    }
                    other => format!("iTop didn't save the entry. {} Your draft is kept.", with_period(&self.describe_write(other))),
                };
                let mut st = self.lock();
                if st.details[i].instance == instance {
                    st.details[i].send_error = Some(message);
                }
                false
            }
        };
        {
            let mut st = self.lock();
            if st.details[i].instance == instance {
                st.details[i].sending = false;
            }
            if ok {
                st.lists[i].drafts.remove(&key);
                st.lists[i].draft_images.remove(&key);
            }
        }
        self.publish();
        ok
    }

    /// The Save panel is modal and blocks until answered, so it runs off the async workers.
    async fn save_panel(&self, name: &str) -> Option<std::path::PathBuf> {
        let native = self.0.ctx.native.clone();
        let name = name.to_owned();
        tokio::task::spawn_blocking(move || native.save_panel(&name)).await.ok().flatten()
    }

    /// `AttachmentsView.save` (DET-33, DET-34).
    async fn detail_download(&self, scope: ListScope, instance: u64, a: AttachmentRef) {
        let i = scope.index();
        {
            let mut st = self.lock();
            let att = &mut st.details[i].attachments;
            att.saving = Some(a.id.clone());
            att.error = None;
            att.saved = None;
        }
        self.publish();
        let outcome: Result<Option<String>, String> = match self.0.images.download(&a).await {
            Ok(file) => match self.save_panel(&file.filename).await {
                None => Ok(None),
                Some(path) => match std::fs::write(&path, &file.data) {
                    Ok(()) => Ok(Some(format!("Saved {} ({}).", file.filename, byte_count(file.data.len() as u64)))),
                    Err(e) => Err(e.to_string()),
                },
            },
            Err(e) => Err(self.describe_image(&e)),
        };
        {
            let mut st = self.lock();
            let d = &mut st.details[i];
            if d.instance == instance {
                match outcome {
                    Ok(saved) => d.attachments.saved = saved,
                    Err(why) => d.attachments.error = Some(format!("Couldn't download {}. {why}", a.name)),
                }
                d.attachments.saving = None;
            }
        }
        self.publish();
    }
}

/// An `ImageIntakeItem` from the UI as `ImageIntake.Item`: raw clipboard data ("") is named
/// "Pasted image.png" or ".tiff" (LOG-14), files that weren't read are oversize (LOG-16).
pub fn intake_item(item: ImageIntakeItem) -> IntakeItem {
    let data = item
        .data_base64
        .as_deref()
        .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
        .unwrap_or_default();
    let oversize = item.data_base64.is_none() && item.size > MAX_INTAKE_FILE_BYTES;
    let filename = if item.filename.is_empty() {
        if is_tiff(&data) { "Pasted image.tiff" } else { "Pasted image.png" }.to_owned()
    } else {
        item.filename
    };
    IntakeItem { data, filename, oversize }
}
