//! Port of `Workspace.swift`: everything one signed-in session works with, shared by the main
//! window and the menu bar. It lives as long as the session, not the window.
//!
//! All model state sits behind one mutex, the Rust stand-in for Swift's `@MainActor`: every
//! change is serialised, and the lock is never held across an `await`, so model methods
//! interleave at their awaits exactly like the Swift ones. After every observable change a model
//! calls [`Workspace::publish`], which rebuilds every snapshot and hands them to the hub (which
//! drops unchanged ones): what SwiftUI's observation re-rendered.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;

use chrono::{DateTime, Utc};
use itop_kit::{
    CurrentUser, ITopError, InlineImageError, InlineImageLoader, MenuBarStatus, ServiceCatalog,
    TeamDirectory, Ticket as KitTicket, TicketScope, TicketService, TicketWrites, TimeTracking,
    TimerStore, WriteError,
};
use tokio::task::AbortHandle;

use super::cmdb::CmdbModel;
use super::detail::DetailState;
use super::format::Fmt;
use super::list::ListState;
use super::menubar::MenuState;
use super::new_ticket::NewTickets;
use super::options::OptionsState;
use super::orgs::OrgsModel;
use super::people::PeopleModel;
use super::prefs::Prefs;
use super::records::{ListScope, SidebarItem};
use super::seams::{ModelContext, WorkspaceLink};
use super::sheets::{AssignState, PendingState, ResolveState};
use super::shell::ShellState;
use super::timer::TimerState;

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// New ticket, CMDB, organizations and people: models of their own that reach the rest of the
/// workspace through `WorkspaceLink`.
#[derive(Clone)]
pub struct Standalone {
    pub new_tickets: Arc<NewTickets>,
    pub cmdb: Arc<CmdbModel>,
    pub orgs: Arc<OrgsModel>,
    pub people: Arc<PeopleModel>,
}

impl Standalone {
    /// The sidebar section's views appeared (`appear`) or went away (`disappear`).
    pub(crate) fn section(&self, item: SidebarItem, shown: bool) {
        match (item, shown) {
            (SidebarItem::ConfigItems, true) => self.cmdb.appear(),
            (SidebarItem::ConfigItems, false) => self.cmdb.disappear(),
            (SidebarItem::Organizations, true) => self.orgs.appear(),
            (SidebarItem::Organizations, false) => self.orgs.disappear(),
            (SidebarItem::People, true) => self.people.appear(),
            (SidebarItem::People, false) => self.people.disappear(),
            _ => {}
        }
    }
}

/// Everything the models read and change.
pub(crate) struct State {
    pub options: OptionsState,
    pub shell: ShellState,
    pub lists: [ListState; 2],
    pub details: [DetailState; 2],
    pub assign: [AssignState; 2],
    pub pending: [PendingState; 2],
    pub resolve: [ResolveState; 2],
    pub timer: TimerState,
    pub menubar: MenuState,
    /// Swift `Workspace.clock`: ticks every second while a timer runs, else every 15 s.
    pub clock: DateTime<Utc>,
    /// What the tray shows now, to only call `set_tray` on a change.
    pub tray: Option<(String, String)>,
    /// Model instances: a detail or a sheet made for one ticket ignores answers meant for another.
    pub next_instance: u64,
}

#[derive(Default)]
pub(crate) struct Tasks {
    pub refresh_loop: Option<AbortHandle>,
    pub clock_loop: Option<AbortHandle>,
    pub emit_loop: Option<AbortHandle>,
    pub team_poll: Option<AbortHandle>,
    pub lookup: [Option<AbortHandle>; 2],
    pub detail_load: [Option<AbortHandle>; 2],
}

pub(crate) struct Inner {
    pub ctx: ModelContext,
    pub fmt: Fmt,
    pub prefs: Arc<dyn Prefs>,
    pub trace: bool,
    /// Swift `ErrorText.host`: the server's host, or "the server".
    pub host: String,
    pub tickets: TicketService,
    pub writes: TicketWrites,
    pub tracking: TimeTracking,
    pub images: InlineImageLoader,
    pub directory: TeamDirectory,
    pub catalog: ServiceCatalog,
    pub timer_store: TimerStore,
    pub state: Mutex<State>,
    pub tasks: Mutex<Tasks>,
    pub standalone: OnceLock<Standalone>,
}

/// One signed-in session's models. Clones share them.
#[derive(Clone)]
pub struct Workspace(pub(crate) Arc<Inner>);

/// The workspace ids the contract lists (all but `session`).
pub const WORKSPACE_MODEL_IDS: [&str; 19] = [
    "shell",
    "options",
    "list:mine",
    "list:team",
    "detail:mine",
    "detail:team",
    "assign:mine",
    "assign:team",
    "pending:mine",
    "pending:team",
    "resolve:mine",
    "resolve:team",
    "newTicket:sheet",
    "newTicket:window",
    "timer",
    "cmdb",
    "orgs",
    "people",
    "menubar",
];

impl Workspace {
    /// Swift `Workspace.init(session:)`. Call [`Workspace::start`] to begin the loops.
    pub fn new(ctx: ModelContext, fmt: Fmt, prefs: Arc<dyn Prefs>, trace: bool) -> Workspace {
        let client = ctx.session.client.clone();
        let host = client.server.host();
        let host = if host.is_empty() {
            "the server".to_owned()
        } else {
            host.to_owned()
        };
        let settings: Arc<dyn itop_kit::SettingsStore> = prefs.clone();
        let timer_store = TimerStore::new(settings);
        let now = ctx.native.now();
        let state = State {
            options: OptionsState::load(prefs.as_ref()),
            shell: ShellState::default(),
            lists: [
                ListState::new(ListScope::Mine),
                ListState::new(ListScope::Team),
            ],
            details: Default::default(),
            assign: Default::default(),
            pending: Default::default(),
            resolve: Default::default(),
            timer: TimerState::new(timer_store.load()),
            menubar: MenuState::default(),
            clock: now,
            tray: None,
            next_instance: 1,
        };
        let ws = Workspace(Arc::new(Inner {
            fmt,
            prefs,
            trace,
            host,
            tickets: TicketService::new(client.clone()),
            writes: TicketWrites::new(client.clone()),
            tracking: TimeTracking::new(client.clone()),
            images: InlineImageLoader::new(client.clone()),
            directory: TeamDirectory::new(client.clone()),
            catalog: ServiceCatalog::new(client),
            timer_store,
            state: Mutex::new(state),
            tasks: Mutex::default(),
            standalone: OnceLock::new(),
            ctx,
        }));
        let link: Arc<dyn WorkspaceLink> = Arc::new(WeakLink(Arc::downgrade(&ws.0)));
        let ctx = ws.0.ctx.clone();
        let _ = ws.0.standalone.set(Standalone {
            new_tickets: NewTickets::new(ctx.clone(), link.clone()),
            cmdb: CmdbModel::new(ctx.clone(), link.clone()),
            orgs: OrgsModel::new(ctx.clone(), link.clone()),
            people: PeopleModel::new(ctx, link),
        });
        ws
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn tasks(&self) -> MutexGuard<'_, Tasks> {
        self.0.tasks.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn session(&self) -> &itop_kit::Session {
        &self.0.ctx.session
    }

    pub(crate) fn me(&self) -> &CurrentUser {
        &self.0.ctx.session.user
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.0.ctx.native.now()
    }

    pub(crate) fn base_url(&self) -> &str {
        self.0.ctx.session.client.server.base_url()
    }

    pub(crate) fn standalone(&self) -> Option<&Standalone> {
        self.0.standalone.get()
    }

    /// `client.canWrite(className:key:)` for a ticket.
    pub(crate) fn can_write(&self, t: &KitTicket) -> bool {
        self.0
            .ctx
            .session
            .client
            .can_write(t.ticket_class.raw_value(), &t.key)
    }

    /// Debug builds with `ITOPWRAP_TRACE=1`: one line per UI event (Swift `UITrace.log`).
    pub(crate) fn trace(&self, event: impl FnOnce() -> String) {
        if cfg!(debug_assertions) && self.0.trace {
            eprintln!(
                "{} [UI] {}",
                self.now().format("%Y-%m-%d %H:%M:%S%.3f"),
                event()
            );
        }
    }

    pub(crate) fn spawn<F>(&self, f: F) -> AbortHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(f).abort_handle()
    }

    // MARK: Error text (Swift `ErrorText.describe`)

    /// "Can't reach <host>. Check your connection, then retry." for network failures, iTop's own
    /// message otherwise (ERR-1).
    pub(crate) fn describe(&self, e: &ITopError) -> String {
        match e {
            ITopError::Transport(_) => format!(
                "Can't reach {}. Check your connection, then retry.",
                self.0.host
            ),
            other => other.to_string(),
        }
    }

    pub(crate) fn describe_write(&self, e: &WriteError) -> String {
        match e {
            WriteError::ITop(e) => self.describe(e),
            WriteError::Failure(f) => f.to_string(),
        }
    }

    pub(crate) fn describe_image(&self, e: &InlineImageError) -> String {
        match e {
            InlineImageError::ITop(e) => self.describe(e),
            InlineImageError::Failure(f) => f.to_string(),
        }
    }

    // MARK: Lifecycle

    /// Swift `Workspace.start()`: the clock and the My tickets refresh loop, plus what the shown
    /// section's views do on appear (My tickets at sign-in), plus the 30 s re-emission of
    /// time-dependent text.
    pub fn start(&self) {
        if self.tasks().refresh_loop.is_some() {
            return;
        }
        let ws = self.clone();
        let clock = self.spawn(async move { ws.clock_loop().await });
        let ws = self.clone();
        let refresh = self.spawn(async move { ws.refresh_loop().await });
        let ws = self.clone();
        let emit = self.spawn(async move { ws.emit_loop().await });
        {
            let mut tasks = self.tasks();
            tasks.clock_loop = Some(clock);
            tasks.refresh_loop = Some(refresh);
            tasks.emit_loop = Some(emit);
        }
        if let Some(s) = self.standalone() {
            let (sort, page_size) = {
                let st = self.lock();
                (st.options.sort, st.options.page_size)
            };
            s.orgs.set_table_options(sort, page_size);
            s.people.set_table_options(sort, page_size);
        }
        let shown = self.lock().shell.selection;
        self.section_appeared(shown);
        self.publish();
    }

    /// Swift `Workspace.stop()`, plus every task the views owned.
    pub fn stop(&self) {
        let mut tasks = self.tasks();
        let handles = [
            tasks.refresh_loop.take(),
            tasks.clock_loop.take(),
            tasks.emit_loop.take(),
            tasks.team_poll.take(),
            tasks.lookup[0].take(),
            tasks.lookup[1].take(),
            tasks.detail_load[0].take(),
            tasks.detail_load[1].take(),
        ];
        for h in handles.into_iter().flatten() {
            h.abort();
        }
        drop(tasks);
        if let Some(s) = self.standalone() {
            s.new_tickets.clear();
            s.cmdb.clear();
            s.orgs.clear();
            s.people.clear();
        }
    }

    async fn refresh_loop(&self) {
        let loaded = matches!(self.lock().lists[0].phase, super::list::Phase::Loaded);
        if !loaded {
            self.list_refresh(ListScope::Mine).await;
        }
        loop {
            tokio::time::sleep(Duration::from_secs(120)).await;
            self.list_refresh(ListScope::Mine).await;
        }
    }

    /// Every second while a timer runs, so the menu bar counter ticks; otherwise every 15 s.
    async fn clock_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let now = self.now();
            let tick = {
                let mut st = self.lock();
                let running = st.timer.running.is_some();
                if running || (now - st.clock).num_milliseconds() >= 15_000 {
                    st.clock = now;
                }
                running
            };
            if tick {
                // The live counters (timer snapshot) and the tray tick by the second.
                self.publish_live();
            } else {
                self.update_tray();
            }
        }
    }

    /// Lists' "Updated …" and ages, the sheets' timer line and the panel's "Updated …" are
    /// re-made every 30 s (POLL-8).
    async fn emit_loop(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            self.publish();
        }
    }

    // MARK: Publishing

    /// Rebuilds every snapshot and publishes it (the hub drops unchanged ones). Also starts what
    /// SwiftUI's `.task(id:)` modifiers on the detail would start for the current selection.
    pub(crate) fn publish(&self) {
        let now = self.now();
        let snaps = {
            let mut st = self.lock();
            self.sync_details(&mut st);
            Snaps {
                shell: self.shell_snapshot(&st),
                options: st.options.snapshot(),
                lists: ListScope::ALL.map(|s| self.list_snapshot(&st, s, now)),
                details: ListScope::ALL.map(|s| self.detail_snapshot(&st, s)),
                assign: ListScope::ALL.map(|s| self.assign_snapshot(&st, s)),
                pending: ListScope::ALL.map(|s| self.pending_snapshot(&st, s, now)),
                resolve: ListScope::ALL.map(|s| self.resolve_snapshot(&st, s, now)),
                timer: self.timer_snapshot(&st, now),
                menubar: self.menubar_snapshot(&st, now),
            }
        };
        let hub = &self.0.ctx.hub;
        hub.publish("shell", &snaps.shell);
        hub.publish("options", &snaps.options);
        for s in ListScope::ALL {
            let i = s.index();
            hub.publish(&format!("list:{}", s.raw()), &snaps.lists[i]);
            hub.publish(&format!("detail:{}", s.raw()), &snaps.details[i]);
            hub.publish(&format!("assign:{}", s.raw()), &snaps.assign[i]);
            hub.publish(&format!("pending:{}", s.raw()), &snaps.pending[i]);
            hub.publish(&format!("resolve:{}", s.raw()), &snaps.resolve[i]);
        }
        hub.publish("timer", &snaps.timer);
        hub.publish("menubar", &snaps.menubar);
        self.update_tray();
    }

    /// The once-a-second part: the timer's counters and the menu bar.
    pub(crate) fn publish_live(&self) {
        let now = self.now();
        let (timer, menubar) = {
            let st = self.lock();
            (
                self.timer_snapshot(&st, now),
                self.menubar_snapshot(&st, now),
            )
        };
        self.0.ctx.hub.publish("timer", &timer);
        self.0.ctx.hub.publish("menubar", &menubar);
        self.update_tray();
    }

    /// The tray title and label, from `Workspace.clock` like Swift's `MenuBarLabel`.
    pub(crate) fn tray_status(&self, st: &State) -> (String, String) {
        let status =
            MenuBarStatus::new(st.lists[0].need_count(self.me()), st.timer.running.clone());
        (status.title(st.clock), status.accessibility_label(st.clock))
    }

    fn update_tray(&self) {
        let changed = {
            let mut st = self.lock();
            let tray = self.tray_status(&st);
            if st.tray.as_ref() == Some(&tray) {
                None
            } else {
                st.tray = Some(tray.clone());
                Some(tray)
            }
        };
        if let Some((title, label)) = changed {
            self.0.ctx.native.set_tray(&title, &label);
            let now = self.now();
            let menubar = {
                let st = self.lock();
                self.menubar_snapshot(&st, now)
            };
            self.0.ctx.hub.publish("menubar", &menubar);
        }
    }

    /// Clears every workspace snapshot (sign-out).
    pub fn clear_snapshots(&self) {
        for id in WORKSPACE_MODEL_IDS {
            self.0.ctx.hub.clear(id);
        }
    }

    /// Swift `Workspace.created(_:assign:)`, after ShellView's `model.selection = .myTickets`.
    pub async fn created(&self, ticket: KitTicket, assign: bool) {
        self.shell_select(SidebarItem::MyTickets).await;
        let notice = format!("{} is created.", ticket.r#ref);
        self.list_show(ListScope::Mine, &ticket, Some(notice));
        if assign {
            let ws = self.clone();
            self.spawn(async move { ws.list_assign_new(ListScope::Mine, &ticket).await });
        }
    }
}

struct Snaps {
    shell: super::shell::ShellSnapshot,
    options: super::options::OptionsSnapshot,
    lists: [super::list::ListSnapshot; 2],
    details: [super::detail::DetailSnapshot; 2],
    assign: [super::sheets::AssignSheetSnapshot; 2],
    pending: [super::sheets::PendingSheetSnapshot; 2],
    resolve: [super::sheets::ResolveSheetSnapshot; 2],
    timer: super::timer::TimerSnapshot,
    menubar: super::menubar::MenuBarSnapshot,
}

/// The workspace as the standalone models see it; weak, so they don't keep it alive.
struct WeakLink(Weak<Inner>);

impl WeakLink {
    fn ws(&self) -> Option<Workspace> {
        self.0.upgrade().map(Workspace)
    }
}

#[async_trait::async_trait]
impl WorkspaceLink for WeakLink {
    fn scope(&self) -> TicketScope {
        self.ws()
            .map(|ws| ws.lock().options.scope())
            .unwrap_or_default()
    }

    async fn open_ticket(&self, ticket: KitTicket) {
        if let Some(ws) = self.ws() {
            ws.shell_open_ticket(&ticket.id(), &ticket.r#ref).await;
        }
    }

    async fn ticket_created(&self, ticket: KitTicket, assign: bool) {
        if let Some(ws) = self.ws() {
            ws.created(ticket, assign).await;
        }
    }

    async fn select_section(&self, section: &str) {
        if let (Some(ws), Some(item)) = (self.ws(), SidebarItem::from_raw(section)) {
            ws.shell_select(item).await;
        }
    }
}
