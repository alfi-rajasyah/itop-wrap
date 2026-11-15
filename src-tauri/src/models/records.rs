//! The shared records of contract.ts ("Primitives" and "Shared records"), and the small rules
//! that build them from itop-kit values.

use chrono::{DateTime, SecondsFormat, Utc};
use itop_kit::{CurrentUser, RichBlock as KitBlock, RichImage, RichText, TicketList};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub fn rfc3339(d: DateTime<Utc>) -> String {
    d.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The two ticket lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum ListScope {
    Mine,
    Team,
}

impl ListScope {
    pub const ALL: [ListScope; 2] = [ListScope::Mine, ListScope::Team];

    pub fn index(self) -> usize {
        match self {
            ListScope::Mine => 0,
            ListScope::Team => 1,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            ListScope::Mine => "mine",
            ListScope::Team => "team",
        }
    }

    pub fn from_raw(raw: &str) -> Option<ListScope> {
        match raw {
            "mine" => Some(ListScope::Mine),
            "team" => Some(ListScope::Team),
            _ => None,
        }
    }

    pub fn view(self) -> itop_kit::TicketListView {
        match self {
            ListScope::Mine => itop_kit::TicketListView::Mine,
            ListScope::Team => itop_kit::TicketListView::Team,
        }
    }

    /// The sidebar section that shows this list.
    pub fn section(self) -> SidebarItem {
        match self {
            ListScope::Mine => SidebarItem::MyTickets,
            ListScope::Team => SidebarItem::TeamTickets,
        }
    }
}

/// Sidebar sections, in sidebar order (Swift `SidebarItem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum SidebarItem {
    MyTickets,
    TeamTickets,
    ConfigItems,
    Organizations,
    People,
}

impl SidebarItem {
    pub const ALL: [SidebarItem; 5] = [
        SidebarItem::MyTickets,
        SidebarItem::TeamTickets,
        SidebarItem::ConfigItems,
        SidebarItem::Organizations,
        SidebarItem::People,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SidebarItem::MyTickets => "My tickets",
            SidebarItem::TeamTickets => "Team tickets",
            SidebarItem::ConfigItems => "Config items",
            SidebarItem::Organizations => "Organizations",
            SidebarItem::People => "People",
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            SidebarItem::MyTickets => "myTickets",
            SidebarItem::TeamTickets => "teamTickets",
            SidebarItem::ConfigItems => "configItems",
            SidebarItem::Organizations => "organizations",
            SidebarItem::People => "people",
        }
    }

    pub fn from_raw(raw: &str) -> Option<SidebarItem> {
        Self::ALL.into_iter().find(|s| s.raw() == raw)
    }

    /// The ticket list this section shows, if any.
    pub fn list(self) -> Option<ListScope> {
        match self {
            SidebarItem::MyTickets => Some(ListScope::Mine),
            SidebarItem::TeamTickets => Some(ListScope::Team),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TicketClass {
    UserRequest,
    Incident,
}

impl From<itop_kit::TicketClass> for TicketClass {
    fn from(c: itop_kit::TicketClass) -> Self {
        match c {
            itop_kit::TicketClass::UserRequest => TicketClass::UserRequest,
            itop_kit::TicketClass::Incident => TicketClass::Incident,
        }
    }
}

/// Status groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TicketGroup {
    #[serde(rename = "escalatedTTR")]
    EscalatedTtr,
    #[serde(rename = "escalatedTTO")]
    EscalatedTto,
    #[serde(rename = "new")]
    New,
    #[serde(rename = "assigned")]
    Assigned,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "other")]
    Other,
    #[serde(rename = "resolved")]
    Resolved,
    #[serde(rename = "closed")]
    Closed,
}

impl From<itop_kit::TicketGroup> for TicketGroup {
    fn from(g: itop_kit::TicketGroup) -> Self {
        use itop_kit::TicketGroup as K;
        match g {
            K::EscalatedTtr => TicketGroup::EscalatedTtr,
            K::EscalatedTto => TicketGroup::EscalatedTto,
            K::New => TicketGroup::New,
            K::Assigned => TicketGroup::Assigned,
            K::Pending => TicketGroup::Pending,
            K::Other => TicketGroup::Other,
            K::Resolved => TicketGroup::Resolved,
            K::Closed => TicketGroup::Closed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum TicketSort {
    Priority,
    LastUpdate,
    Newest,
    Oldest,
}

impl From<itop_kit::TicketSort> for TicketSort {
    fn from(s: itop_kit::TicketSort) -> Self {
        match s {
            itop_kit::TicketSort::Priority => TicketSort::Priority,
            itop_kit::TicketSort::LastUpdate => TicketSort::LastUpdate,
            itop_kit::TicketSort::Newest => TicketSort::Newest,
            itop_kit::TicketSort::Oldest => TicketSort::Oldest,
        }
    }
}

impl From<TicketSort> for itop_kit::TicketSort {
    fn from(s: TicketSort) -> Self {
        match s {
            TicketSort::Priority => itop_kit::TicketSort::Priority,
            TicketSort::LastUpdate => itop_kit::TicketSort::LastUpdate,
            TicketSort::Newest => itop_kit::TicketSort::Newest,
            TicketSort::Oldest => itop_kit::TicketSort::Oldest,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum LogTab {
    Public,
    Private,
}

/// One line of feedback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct Notice {
    pub text: String,
    pub is_error: bool,
}

impl Notice {
    pub fn ok(text: impl Into<String>) -> Self {
        Notice { text: text.into(), is_error: false }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Notice { text: text.into(), is_error: true }
    }
}

/// One ticket plus the per-user facts every view needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct Ticket {
    pub id: String,
    pub key: String,
    pub ticket_class: TicketClass,
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub r#ref: String,
    pub title: String,
    pub status: String,
    pub status_label: String,
    pub priority: i64,
    pub caller_name: String,
    pub org_name: String,
    pub org_id: String,
    pub team_id: String,
    pub team_name: String,
    pub agent_id: String,
    pub agent_name: String,
    pub start_date: Option<String>,
    pub last_update: Option<String>,
    pub is_unassigned: bool,
    pub is_mine: bool,
    pub needs_you: bool,
}

/// `TicketGroup.statusLabel` (W/TicketListView.swift:386, DET-8).
pub fn status_label(status: &str) -> String {
    match status {
        "escalated_ttr" => "Escalated TTR".to_owned(),
        "escalated_tto" => "Escalated TTO".to_owned(),
        "waiting_for_approval" => "Waiting for approval".to_owned(),
        _ => {
            let mut chars = status.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().replace('_', " "),
                None => String::new(),
            }
        }
    }
}

pub fn ticket_view(t: &itop_kit::Ticket, me: &CurrentUser) -> Ticket {
    Ticket {
        id: t.id(),
        key: t.key.clone(),
        ticket_class: t.ticket_class.into(),
        r#ref: t.r#ref.clone(),
        title: t.title.clone(),
        status: t.status.clone(),
        status_label: status_label(&t.status),
        priority: t.priority,
        caller_name: t.caller_name.clone(),
        org_name: t.org_name.clone(),
        org_id: t.org_id.clone(),
        team_id: t.team_id.clone(),
        team_name: t.team_name.clone(),
        agent_id: t.agent_id.clone(),
        agent_name: t.agent_name.clone(),
        start_date: t.start_date.map(rfc3339),
        last_update: t.last_update.map(rfc3339),
        is_unassigned: t.is_unassigned(),
        is_mine: t.agent_id == me.contact_id,
        needs_you: TicketList::needs_you(t, me),
    }
}

/// Who has a ticket, as the detail strip draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum AgentChip {
    Unassigned,
    Person { initials: String, name: String },
}

/// The agent column of a list row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum RowAgent {
    Hidden,
    AssignToMe,
    Unassigned,
    You,
    Name { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum RowTime {
    None,
    Running,
    Spent { text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum PriorityMark {
    P1,
    P2,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TicketRow {
    pub ticket: Ticket,
    pub age: Option<String>,
    pub opened_tooltip: Option<String>,
    pub priority_mark: Option<PriorityMark>,
    pub agent: RowAgent,
    pub time: RowTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TicketSection {
    pub group: TicketGroup,
    pub title: String,
    pub total: i64,
    pub rows: Vec<TicketRow>,
}

/// Under a paged list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum Pager {
    None,
    Count {
        text: String,
    },
    Pages {
        page: i64,
        #[serde(rename = "pageCount")]
        #[ts(rename = "pageCount")]
        page_count: i64,
        label: String,
        #[serde(rename = "canPrevious")]
        #[ts(rename = "canPrevious")]
        can_previous: bool,
        #[serde(rename = "canNext")]
        #[ts(rename = "canNext")]
        can_next: bool,
    },
}

/// The `Pager` view (W/ListOptions.swift:156-178).
pub fn pager(page: &itop_kit::TicketPage) -> Pager {
    if page.page_count > 1 {
        Pager::Pages {
            page: page.page,
            page_count: page.page_count,
            label: page.label(),
            can_previous: page.page > 1,
            can_next: page.page < page.page_count,
        }
    } else if page.total > 0 {
        Pager::Count { text: if page.total == 1 { "1 ticket".to_owned() } else { format!("{} tickets", page.total) } }
    } else {
        Pager::None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum RichRun {
    Text { text: String, link: Option<String> },
    Icon,
    OutsideImage { host: String, url: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum RichBlock {
    Text {
        runs: Vec<RichRun>,
    },
    Image {
        src: String,
        #[serde(rename = "ticketWebUrl")]
        #[ts(rename = "ticketWebUrl")]
        ticket_web_url: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct RichContent {
    pub blocks: Vec<RichBlock>,
}

/// The `itop://` URL of a pasted iTop image.
pub fn inline_image_src(id: &str) -> String {
    format!("itop://localhost/inline-image/{id}")
}

/// `RichTextView` (W/RichTextView.swift:15-66): parsed blocks, or the plain text when the parse
/// yields nothing (DET-50).
pub fn rich_content(html: Option<&str>, plain: &str, base: &str, ticket_web_url: Option<&str>) -> RichContent {
    let blocks = html.map(|h| RichText::parse(h, base)).unwrap_or_default();
    if blocks.is_empty() {
        return RichContent {
            blocks: vec![RichBlock::Text { runs: vec![RichRun::Text { text: plain.to_owned(), link: None }] }],
        };
    }
    RichContent {
        blocks: blocks
            .into_iter()
            .map(|b| match b {
                KitBlock::Text(runs) => RichBlock::Text {
                    runs: runs
                        .into_iter()
                        .map(|r| {
                            if r.icon.is_some() {
                                RichRun::Icon
                            } else if r.is_outside_image
                                && let Some(url) = r.link.clone()
                            {
                                RichRun::OutsideImage { host: r.text, url }
                            } else {
                                RichRun::Text { text: r.text, link: r.link }
                            }
                        })
                        .collect(),
                },
                KitBlock::Image(image) => RichBlock::Image {
                    src: match image {
                        RichImage::Inline { id } => inline_image_src(&id),
                        RichImage::Data(bytes) => data_url(&bytes),
                    },
                    ticket_web_url: ticket_web_url.map(str::to_owned),
                },
            })
            .collect(),
    }
}

/// A `data:` URL for an image embedded in the HTML. The parser keeps only the bytes, so the type
/// is read from them (PNG, JPEG, GIF; anything else is labelled PNG and left to the web view).
fn data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    let mime = itop_kit::LogImage::sniff(bytes).map_or("image/png", |(m, _)| m);
    format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct LogEntryView {
    pub id: i64,
    pub header: String,
    pub is_private: bool,
    pub content: RichContent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct LinkedCI {
    pub key: String,
    pub class_name: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct PickItem {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct DropdownSearch {
    pub prompt: String,
    pub no_match: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct Dropdown {
    pub items: Vec<PickItem>,
    pub selected_id: Option<String>,
    pub placeholder: String,
    pub enabled: bool,
    pub search: Option<DropdownSearch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct RequiredMark {
    pub missing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct TicketLink {
    pub ticket_id: String,
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub r#ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ImageIntakeItem {
    pub filename: String,
    pub data_base64: Option<String>,
    pub size: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct DraftImage {
    pub id: String,
    pub filename: String,
    pub size_text: String,
    pub thumbnail_src: String,
}

/// A confirmation dialog's texts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct ConfirmText {
    pub title: String,
    pub message: String,
}

/// A loading state for sheet pickers (Swift's `Loadable`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Loadable<T> {
    #[default]
    Idle,
    Loading,
    Loaded(T),
    Failed(String),
}

impl<T> Loadable<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Loadable::Loaded(v) => Some(v),
            _ => None,
        }
    }
}

/// The sheets' `placeholder(_:empty:loading:)`.
pub fn placeholder<T>(state: &Loadable<Vec<T>>, loading: &str, empty: &str) -> String {
    match state {
        Loadable::Idle | Loadable::Loading => loading.to_owned(),
        Loadable::Failed(message) => format!("Couldn't load: {message}"),
        Loadable::Loaded(items) => {
            if items.is_empty() {
                empty.to_owned()
            } else {
                "Choose…".to_owned()
            }
        }
    }
}

/// Adds a final "." when the reason doesn't end in . ! or ? (ERR-13); trims spaces first.
pub fn with_period(reason: &str) -> String {
    let mut reason = reason.trim_matches(|c: char| c == ' ' || c == '\t').to_owned();
    if let Some(last) = reason.chars().last()
        && !".!?".contains(last)
    {
        reason.push('.');
    }
    reason
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn det_8_status_labels() {
        assert_eq!(status_label("escalated_ttr"), "Escalated TTR");
        assert_eq!(status_label("escalated_tto"), "Escalated TTO");
        assert_eq!(status_label("waiting_for_approval"), "Waiting for approval");
        assert_eq!(status_label("assigned"), "Assigned");
        assert_eq!(status_label("pending_customer_reply"), "Pending customer reply");
        assert_eq!(status_label(""), "");
    }

    #[test]
    fn det_50_empty_parse_shows_plain_text() {
        let c = rich_content(None, "Plain", "https://x", None);
        assert_eq!(
            serde_json::to_value(&c).ok(),
            Some(serde_json::json!({"blocks":[{"type":"text","runs":[{"type":"text","text":"Plain","link":null}]}]}))
        );
    }

    #[test]
    fn err_13_period_added_only_without_final_punctuation() {
        assert_eq!(with_period("Nope"), "Nope.");
        assert_eq!(with_period("Nope!"), "Nope!");
        assert_eq!(with_period(" Why? "), "Why?");
        assert_eq!(with_period(""), "");
    }

    #[test]
    fn pager_shapes() {
        assert_eq!(serde_json::to_value(Pager::None).ok(), Some(serde_json::json!({"type":"none"})));
        let p = Pager::Pages { page: 2, page_count: 3, label: "21–40 of 56".into(), can_previous: true, can_next: true };
        assert_eq!(
            serde_json::to_value(p).ok(),
            Some(serde_json::json!({"type":"pages","page":2,"pageCount":3,"label":"21–40 of 56","canPrevious":true,"canNext":true}))
        );
    }
}
