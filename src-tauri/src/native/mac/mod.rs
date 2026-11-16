//! `Native` for the real Mac: windows, menus, the tray and the menu bar panel (contract.md §6),
//! plus clipboard, URLs, the Save panel and image conversion.
//!
//! [`Shell`] holds the native state; [`MacNative`] is the `Native` the models get. All AppKit work
//! hops to the main thread. Windows are created by [`Shell::create_windows`] in Tauri's `setup`,
//! after the app state is managed, so the web views never call a command before it exists.

pub mod appkit;
pub mod files;
pub mod image;
pub mod locale;
pub mod tray_draw;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use objc2::MainThreadMarker;
use serde_json::Value;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::webview::NewWindowResponse;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Rect, TitleBarStyle, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry,
};

use crate::native::{ConvertTo, ConvertedImage, Native, PastedImage};
use crate::router::{NativeEvent, Router};
use appkit::{PanelKey, on_main, on_main_async};
use tray_draw::Slide;

/// Navigation guard for every app web view (see [`crate::commands::is_app_url`]).
fn app_page(url: &tauri::Url) -> bool {
    let ok = crate::commands::is_app_url(url);
    if !ok {
        eprintln!("navigation refused: {}", url.scheme());
    }
    ok
}

pub const MAIN: &str = "main";
pub const MENUBAR: &str = "menubar";
pub const NEW_TICKET: &str = "new-ticket";
pub const RESOLVE: &str = "resolve";
const TRAY_ID: &str = "itop-wrap";

/// The panel's width (MB-6) and its height until the UI sizes it to its content.
pub const PANEL_WIDTH: f64 = 360.0;
const PANEL_HEIGHT: f64 = 420.0;
/// The shell's minimum (SHELL-2: columns 180 + 300 + 380, height 520) and its first size when no
/// frame is saved: the ideal column widths 200 + 360 + 380 at the minimum height (SHELL-1).
const SHELL_MIN: (f64, f64) = (860.0, 520.0);
const SHELL_DEFAULT: (f64, f64) = (940.0, 520.0);
/// New Ticket is 480 pt wide (W/NewTicketView.swift:141); the UI may fit the height to its content.
const NEW_TICKET_SIZE: (f64, f64) = (480.0, 640.0);
/// The Resolve window's first size; the page then fits the height to the form.
const RESOLVE_SIZE: (f64, f64) = (500.0, 480.0);
/// The sidebar items in order, for View ▸ ⌘1…⌘5 (W/SessionModel.swift:5-31).
pub const SECTIONS: [&str; 5] = [
    "My tickets",
    "Team tickets",
    "Config items",
    "Organizations",
    "People",
];

/// What the main window looks like for a `session` snapshot type (contract.md §6, AUTH-1, AUTH-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Restoring and unreachable: 480 × 320, fixed.
    Small,
    /// Signed out: the sign-in form, 480 × 520, fixed.
    SignIn,
    /// Signed in: resizable, at least 860 × 520.
    Shell,
}

impl Layout {
    pub fn for_session(snapshot: &Value) -> Option<Self> {
        match snapshot.get("type")?.as_str()? {
            "restoring" | "unreachable" => Some(Self::Small),
            "signedOut" => Some(Self::SignIn),
            "signedIn" => Some(Self::Shell),
            _ => None,
        }
    }

    fn fixed_size(self) -> Option<(f64, f64)> {
        match self {
            Self::Small => Some((480.0, 320.0)),
            Self::SignIn => Some((480.0, 520.0)),
            Self::Shell => None,
        }
    }
}

/// Where the panel goes for a status item at `icon` (logical x, y, width, height) on a screen at
/// `screen` (logical x, y, width, height): centred under the icon, kept 8 pt inside the screen.
pub fn panel_origin(icon: (f64, f64, f64, f64), screen: (f64, f64, f64, f64)) -> (f64, f64) {
    let (ix, iy, iw, ih) = icon;
    let (sx, _, sw, _) = screen;
    let x = (ix + iw / 2.0 - PANEL_WIDTH / 2.0)
        .min(sx + sw - PANEL_WIDTH - 8.0)
        .max(sx + 8.0);
    (x, iy + ih + 4.0)
}

#[derive(Default)]
struct State {
    layout: Option<Layout>,
    signed_in: bool,
    tray: Option<TrayIcon<Wry>>,
    tray_title: String,
    tray_label: String,
    slide: Slide,
    /// When the panel last hid because it lost focus: a click on the status item that caused
    /// that must not reopen it.
    panel_hidden_at: Option<Instant>,
    /// The panel's top-left (logical), kept while the UI resizes it.
    panel_origin: Option<(f64, f64)>,
    /// The main window was closed (hidden) while signed in and hasn't been shown since.
    main_hidden: bool,
    /// The shell's sidebar is collapsed: View shows "Show Sidebar".
    sidebar_hidden: bool,
}

/// The native shell: windows, menus, tray, panel.
pub struct Shell {
    app: AppHandle<Wry>,
    router: OnceLock<Arc<dyn Router>>,
    state: Mutex<State>,
}

impl Shell {
    pub fn new(app: AppHandle<Wry>) -> Arc<Self> {
        Arc::new(Self {
            app,
            router: OnceLock::new(),
            state: Mutex::default(),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Connects the router; native events before this are dropped.
    pub fn set_router(&self, router: Arc<dyn Router>) {
        let _ = self.router.set(router);
    }

    /// Hands an event to the router on the async runtime.
    pub fn send(&self, event: NativeEvent) {
        if let Some(router) = self.router.get().cloned() {
            tauri::async_runtime::spawn(async move { router.native_event(event).await });
        }
    }

    fn window(&self, label: &str) -> Option<WebviewWindow<Wry>> {
        self.app.get_webview_window(label)
    }

    // MARK: windows

    /// Creates the main window (visible, restoring size) and the hidden panel, installs the menu
    /// and the panel's key monitor. Call on the main thread from `setup`.
    pub fn create_windows(self: &Arc<Self>, mtm: MainThreadMarker) -> tauri::Result<()> {
        let main = WebviewWindowBuilder::new(&self.app, MAIN, WebviewUrl::App("index.html".into()))
            .on_navigation(app_page)
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .title("iTop Wrap")
            .inner_size(480.0, 320.0)
            .resizable(false)
            .maximizable(false)
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .disable_drag_drop_handler()
            .center()
            .visible(true)
            .build()?;
        let shell = self.clone();
        main.on_window_event(move |event| shell.main_window_event(event));
        self.state().layout = Some(Layout::Small);

        let panel =
            WebviewWindowBuilder::new(&self.app, MENUBAR, WebviewUrl::App("menubar.html".into()))
                .on_navigation(app_page)
                .on_new_window(|_, _| NewWindowResponse::Deny)
                .title("iTop Wrap")
                .inner_size(PANEL_WIDTH, PANEL_HEIGHT)
                .decorations(false)
                .transparent(true)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible_on_all_workspaces(true)
                .accept_first_mouse(true)
                .disable_drag_drop_handler()
                .focused(false)
                .visible(false)
                .build()?;
        appkit::make_menubar_panel(mtm, &panel);
        let shell = self.clone();
        panel.on_window_event(move |event| shell.panel_window_event(event));

        let shell = self.clone();
        let shell2 = self.clone();
        appkit::install_panel_key_monitor(
            mtm,
            move || shell.panel_is_key(),
            move |key| {
                shell2.send(match key {
                    PanelKey::NewTicket => NativeEvent::PanelNewTicket,
                    PanelKey::OpenMain => NativeEvent::PanelOpenMain,
                    PanelKey::Refresh => NativeEvent::PanelRefresh,
                })
            },
        );

        self.rebuild_menu();
        appkit::activate(mtm);
        Ok(())
    }

    fn main_window_event(&self, event: &WindowEvent) {
        let WindowEvent::CloseRequested { api, .. } = event else {
            return;
        };
        if self.state().signed_in {
            // SHELL-16: signed in, the menu bar keeps working with the window closed.
            api.prevent_close();
            if let Some(w) = self.window(MAIN) {
                let _ = w.hide();
            }
            self.state().main_hidden = true;
            self.send(NativeEvent::MainWindowHidden);
        } else {
            // Signed out, restoring or unreachable: closing the window quits.
            self.app.exit(0);
        }
    }

    fn panel_window_event(&self, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(false) => {
                let visible = self
                    .window(MENUBAR)
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);
                if visible {
                    self.state().panel_hidden_at = Some(Instant::now());
                    self.hide_panel();
                }
            }
            WindowEvent::Resized(_) => {
                // Keep the top edge under the status item while the UI fits the height.
                let origin = self.state().panel_origin;
                if let (Some((x, y)), Some(w)) = (origin, self.window(MENUBAR)) {
                    let _ = w.set_position(LogicalPosition::new(x, y));
                }
            }
            _ => {}
        }
    }

    fn new_ticket_window_event(&self, event: &WindowEvent) {
        if let WindowEvent::CloseRequested { .. } = event {
            self.send(NativeEvent::NewTicketWindowClosed);
        }
    }

    fn resolve_window_event(&self, event: &WindowEvent) {
        if let WindowEvent::CloseRequested { .. } = event {
            self.send(NativeEvent::ResolveWindowClosed);
        }
    }

    /// The `session` snapshot changed: size the main window for it (call from any thread).
    pub fn session_snapshot(self: &Arc<Self>, snapshot: &Value) {
        let Some(layout) = Layout::for_session(snapshot) else {
            return;
        };
        let shell = self.clone();
        on_main_async(&self.app, move |mtm| shell.apply_layout(mtm, layout));
    }

    fn apply_layout(&self, mtm: MainThreadMarker, layout: Layout) {
        if self.state().layout == Some(layout) {
            return;
        }
        let Some(w) = self.window(MAIN) else { return };
        let scale = w.scale_factor().unwrap_or(1.0);
        let top_left = w.outer_position().ok().map(|p| p.to_logical::<f64>(scale));
        self.state().layout = Some(layout);
        // Only the shell's frame is remembered; stop saving before resizing to a fixed size.
        appkit::autosave_frame(mtm, &w, None);
        let _ = w.set_min_size(None::<LogicalSize<f64>>);
        let _ = w.set_max_size(None::<LogicalSize<f64>>);
        match layout.fixed_size() {
            Some((width, height)) => {
                let size = LogicalSize::new(width, height);
                let _ = w.set_resizable(false);
                let _ = w.set_maximizable(false);
                let _ = w.set_size(size);
                let _ = w.set_min_size(Some(size));
                let _ = w.set_max_size(Some(size));
                if let Some(p) = top_left {
                    let _ = w.set_position(p);
                }
            }
            None => {
                let _ = w.set_resizable(true);
                let _ = w.set_maximizable(true);
                let _ = w.set_min_size(Some(LogicalSize::new(SHELL_MIN.0, SHELL_MIN.1)));
                if !appkit::restore_frame(mtm, &w, appkit::MAIN_FRAME_AUTOSAVE) {
                    let _ = w.set_size(LogicalSize::new(SHELL_DEFAULT.0, SHELL_DEFAULT.1));
                    let _ = w.center();
                }
                appkit::autosave_frame(mtm, &w, Some(appkit::MAIN_FRAME_AUTOSAVE));
            }
        }
    }

    fn show_main(&self, mtm: MainThreadMarker) {
        if let Some(w) = self.window(MAIN) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        appkit::activate(mtm);
        if std::mem::take(&mut self.state().main_hidden) {
            self.send(NativeEvent::MainWindowShown);
        }
    }

    fn show_new_ticket(self: &Arc<Self>, mtm: MainThreadMarker) {
        if let Some(w) = self.window(NEW_TICKET) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            appkit::activate(mtm);
            return;
        }
        let built = WebviewWindowBuilder::new(
            &self.app,
            NEW_TICKET,
            WebviewUrl::App("index.html?window=new-ticket".into()),
        )
        .on_navigation(app_page)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .title("New Ticket")
        .inner_size(NEW_TICKET_SIZE.0, NEW_TICKET_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .disable_drag_drop_handler()
        .center()
        .visible(true)
        .build();
        match built {
            Ok(w) => {
                appkit::exclude_from_windows_menu(mtm, &w);
                let shell = self.clone();
                w.on_window_event(move |event| shell.new_ticket_window_event(event));
                let _ = w.set_focus();
                appkit::activate(mtm);
            }
            Err(e) => eprintln!("New Ticket window wasn't created: {e}"),
        }
    }

    fn close_new_ticket(&self) {
        // `destroy` skips CloseRequested, so this doesn't look like the close button.
        if let Some(w) = self.window(NEW_TICKET) {
            let _ = w.destroy();
        }
    }

    /// Like the New Ticket window: its own titled window, no main window (menu bar Resolve).
    fn show_resolve(self: &Arc<Self>, mtm: MainThreadMarker) {
        if let Some(w) = self.window(RESOLVE) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
            appkit::activate(mtm);
            return;
        }
        let built = WebviewWindowBuilder::new(
            &self.app,
            RESOLVE,
            WebviewUrl::App("index.html?window=resolve".into()),
        )
        .on_navigation(app_page)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .title("Resolve Ticket")
        .inner_size(RESOLVE_SIZE.0, RESOLVE_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .disable_drag_drop_handler()
        .center()
        .visible(true)
        .build();
        match built {
            Ok(w) => {
                appkit::exclude_from_windows_menu(mtm, &w);
                let shell = self.clone();
                w.on_window_event(move |event| shell.resolve_window_event(event));
                let _ = w.set_focus();
                appkit::activate(mtm);
            }
            Err(e) => eprintln!("Resolve window wasn't created: {e}"),
        }
    }

    fn close_resolve(&self) {
        // `destroy` skips CloseRequested, so this doesn't look like the close button.
        if let Some(w) = self.window(RESOLVE) {
            let _ = w.destroy();
        }
    }

    // MARK: panel

    fn panel_is_key(&self) -> bool {
        match (MainThreadMarker::new(), self.window(MENUBAR)) {
            (Some(mtm), Some(w)) => appkit::is_key(mtm, &w),
            _ => false,
        }
    }

    fn hide_panel(&self) {
        if let Some(w) = self.window(MENUBAR) {
            let _ = w.hide();
        }
        self.highlight_tray(false);
    }

    fn toggle_panel(&self, icon: Rect) {
        let Some(panel) = self.window(MENUBAR) else {
            return;
        };
        if panel.is_visible().unwrap_or(false) {
            self.hide_panel();
            return;
        }
        let just_hidden = self
            .state()
            .panel_hidden_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
        if just_hidden {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if let Some((x, y)) = self.panel_position(icon) {
            self.state().panel_origin = Some((x, y));
            let _ = panel.set_position(LogicalPosition::new(x, y));
        }
        appkit::show_panel(mtm, &panel);
        self.highlight_tray(true);
        self.send(NativeEvent::PanelShown);
    }

    fn panel_position(&self, icon: Rect) -> Option<(f64, f64)> {
        let probe = icon.position.to_physical::<f64>(1.0);
        let monitor = self
            .app
            .monitor_from_point(probe.x, probe.y)
            .ok()
            .flatten()
            .or_else(|| self.app.primary_monitor().ok().flatten())?;
        let scale = monitor.scale_factor();
        let p = icon.position.to_logical::<f64>(scale);
        let s = icon.size.to_logical::<f64>(scale);
        let mp = monitor.position().to_logical::<f64>(scale);
        let ms = monitor.size().to_logical::<f64>(scale);
        Some(panel_origin(
            (p.x, p.y, s.width, s.height),
            (mp.x, mp.y, ms.width, ms.height),
        ))
    }

    // MARK: menus (contract.md §6, KEY-1, KEY-2)

    fn rebuild_menu(&self) {
        let (signed_in, sidebar_hidden) = {
            let s = self.state();
            (s.signed_in, s.sidebar_hidden)
        };
        match build_menu(&self.app, signed_in, sidebar_hidden) {
            Ok(menu) => {
                if let Err(e) = self.app.set_menu(menu) {
                    eprintln!("menu wasn't installed: {e}");
                }
            }
            Err(e) => eprintln!("menu wasn't built: {e}"),
        }
    }

    /// The app's menu handler (register once with `on_menu_event`).
    pub fn menu_event(self: &Arc<Self>, id: &str) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let panel_key = self.panel_is_key();
        match id {
            "new-ticket" if panel_key => self.send(NativeEvent::PanelNewTicket),
            "open-main" if panel_key => self.send(NativeEvent::PanelOpenMain),
            "new-ticket" => {
                self.show_main(mtm);
                self.send(NativeEvent::MenuNewTicket);
            }
            "open-main" => {
                self.show_main(mtm);
                self.send(NativeEvent::MenuOpenMain);
            }
            "toggle-sidebar" => self.send(NativeEvent::ToggleSidebar),
            other => {
                if let Some(n) = other
                    .strip_prefix("view-")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    self.send(NativeEvent::MenuSelectSection(n));
                }
            }
        }
    }

    /// Dock icon clicked (`RunEvent::Reopen`): bring the main window back if nothing shows.
    pub fn reopen(&self, has_visible_windows: bool) {
        if has_visible_windows {
            return;
        }
        if let Some(mtm) = MainThreadMarker::new() {
            self.show_main(mtm);
        }
    }

    // MARK: tray (MB-1…6)

    fn set_signed_in(self: &Arc<Self>, signed_in: bool) {
        let was = {
            let mut s = self.state();
            if signed_in && !s.signed_in {
                // A new workspace starts with the sidebar showing.
                s.sidebar_hidden = false;
            }
            std::mem::replace(&mut s.signed_in, signed_in)
        };
        if was != signed_in {
            self.rebuild_menu();
        }
        if signed_in {
            self.ensure_tray();
        } else {
            let tray = self.state().tray.take();
            if tray.is_some() {
                let _ = self.app.remove_tray_by_id(TRAY_ID);
            }
            self.hide_panel();
            self.close_new_ticket();
            self.close_resolve();
        }
    }

    fn ensure_tray(self: &Arc<Self>) {
        if self.state().tray.is_some() {
            return;
        }
        let Some(icon) = self.tray_image() else {
            return;
        };
        let shell = self.clone();
        let built = TrayIconBuilder::with_id(TRAY_ID)
            .icon(icon)
            .icon_as_template(true)
            .show_menu_on_left_click(false)
            .on_tray_icon_event(move |_, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Down,
                    rect,
                    ..
                } = event
                {
                    shell.toggle_panel(rect);
                }
            })
            .build(&self.app);
        match built {
            Ok(tray) => {
                self.state().tray = Some(tray);
                self.apply_tray_label();
            }
            Err(e) => eprintln!("tray wasn't created: {e}"),
        }
    }

    /// The image for the current title and slide position.
    fn tray_image(&self) -> Option<tauri::image::Image<'static>> {
        let (title, visible) = {
            let s = self.state();
            let (t, v) = s.slide.shown(&s.tray_title);
            (t.to_owned(), v)
        };
        let r = tray_draw::label(&title, visible)?;
        Some(tauri::image::Image::new_owned(
            r.rgba,
            r.width_px,
            r.height_px,
        ))
    }

    fn render_tray(&self) {
        let tray = self.state().tray.clone();
        if let (Some(tray), Some(img)) = (tray, self.tray_image())
            // Not `set_icon`: tray-icon's macOS `set_icon` drops the template flag, leaving a
            // plain black image that ignores the menu bar's light/dark appearance.
            && let Err(e) = tray.set_icon_with_as_template(Some(img), true)
        {
            eprintln!("tray icon wasn't updated: {e}");
        }
    }

    fn apply_tray_label(&self) {
        let (tray, label) = {
            let s = self.state();
            (s.tray.clone(), s.tray_label.clone())
        };
        let Some(tray) = tray else { return };
        let _ = tray.with_inner_tray_icon(move |inner| {
            if let (Some(mtm), Some(item)) = (MainThreadMarker::new(), inner.ns_status_item()) {
                appkit::status_item_label(mtm, &item, &label);
            }
        });
    }

    fn highlight_tray(&self, on: bool) {
        let tray = self.state().tray.clone();
        if let Some(tray) = tray {
            let _ = tray.with_inner_tray_icon(move |inner| {
                if let (Some(mtm), Some(item)) = (MainThreadMarker::new(), inner.ns_status_item()) {
                    appkit::status_item_highlight(mtm, &item, on);
                }
            });
        }
    }

    /// New title and label (main thread): slide when the width changes (MB-4), then redraw.
    fn update_tray(self: &Arc<Self>, mtm: MainThreadMarker, title: String, label: String) {
        let plan = {
            let mut s = self.state();
            let old = std::mem::replace(&mut s.tray_title, title.clone());
            let label_changed = s.tray_label != label;
            s.tray_label = label;
            let plan = if old != title {
                s.slide.begin(
                    &old,
                    &title,
                    tray_draw::text_width,
                    appkit::reduce_motion(mtm),
                )
            } else {
                None
            };
            drop(s);
            if label_changed {
                self.apply_tray_label();
            }
            plan
        };
        self.render_tray();
        if let Some((generation, widths)) = plan {
            let shell = self.clone();
            std::thread::spawn(move || shell.run_slide(generation, widths));
        }
    }

    /// The slide: one width per 18 ms step on the main thread, then the full title.
    fn run_slide(self: Arc<Self>, generation: u64, widths: Vec<f64>) {
        for width in widths {
            let shell = self.clone();
            let current = on_main(&self.app, move |_| {
                let ok = shell.state().slide.step(generation, width);
                if ok {
                    shell.render_tray();
                }
                ok
            });
            if current != Some(true) {
                return;
            }
            std::thread::sleep(Duration::from_millis(tray_draw::SLIDE_STEP_MS));
        }
        let shell = self.clone();
        on_main_async(&self.app, move |_| {
            shell.state().slide.finish(generation);
            shell.render_tray();
        });
    }
}

fn build_menu(
    app: &AppHandle<Wry>,
    signed_in: bool,
    sidebar_hidden: bool,
) -> tauri::Result<Menu<Wry>> {
    let sep = || PredefinedMenuItem::separator(app);
    let app_menu = Submenu::with_items(
        app,
        "iTop Wrap",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About iTop Wrap"), None)?,
            &sep()?,
            &PredefinedMenuItem::services(app, None)?,
            &sep()?,
            &PredefinedMenuItem::hide(app, Some("Hide iTop Wrap"))?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &sep()?,
            &PredefinedMenuItem::quit(app, Some("Quit iTop Wrap"))?,
        ],
    )?;

    let file = Submenu::new(app, "File", true)?;
    if signed_in {
        file.append(&MenuItem::with_id(
            app,
            "new-ticket",
            "New Ticket…",
            true,
            Some("CmdOrCtrl+N"),
        )?)?;
    }
    file.append(&MenuItem::with_id(
        app,
        "open-main",
        "Open iTop Wrap",
        true,
        Some("CmdOrCtrl+O"),
    )?)?;
    file.append(&sep()?)?;
    file.append(&PredefinedMenuItem::close_window(app, None)?)?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &sep()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    // SwiftUI's View menu for the NavigationSplitView: the sidebar toggle first, then the
    // `CommandGroup(after: .sidebar)` items (a divider and ⌘1…⌘5), then Enter Full Screen (KEY-2).
    let view = Submenu::new(app, "View", true)?;
    if signed_in {
        view.append(&MenuItem::with_id(
            app,
            "toggle-sidebar",
            if sidebar_hidden {
                "Show Sidebar"
            } else {
                "Hide Sidebar"
            },
            true,
            Some("Ctrl+Cmd+S"),
        )?)?;
        view.append(&sep()?)?;
        for (i, title) in SECTIONS.iter().enumerate() {
            view.append(&MenuItem::with_id(
                app,
                format!("view-{i}"),
                *title,
                true,
                Some(format!("CmdOrCtrl+{}", i + 1).as_str()),
            )?)?;
        }
        view.append(&sep()?)?;
    }
    view.append(&PredefinedMenuItem::fullscreen(app, None)?)?;

    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &sep()?,
            &PredefinedMenuItem::bring_all_to_front(app, None)?,
        ],
    )?;
    window.set_as_windows_menu_for_nsapp()?;
    let help = Submenu::new(app, "Help", true)?;
    help.set_as_help_menu_for_nsapp()?;

    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])
}

/// `Native` for the app.
#[derive(Clone)]
pub struct MacNative {
    shell: Arc<Shell>,
    /// The standard defaults domain, for `newTicket.assignToMe` (PERS-8).
    standard: crate::prefs::Prefs,
}

impl MacNative {
    pub fn new(shell: Arc<Shell>, standard: crate::prefs::Prefs) -> Self {
        Self { shell, standard }
    }

    fn app(&self) -> &AppHandle<Wry> {
        &self.shell.app
    }
}

impl Native for MacNative {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn copy_text(&self, text: &str) {
        let text = text.to_owned();
        on_main_async(self.app(), move |mtm| appkit::copy_text(mtm, &text));
    }

    fn open_url(&self, url: &str) {
        if !crate::commands::is_openable_url(url) {
            eprintln!("open_url refused: {url}");
            return;
        }
        let url = url.to_owned();
        on_main_async(self.app(), move |mtm| appkit::open_url(mtm, &url));
    }

    /// Blocks the calling thread while the panel is up; call it from a blocking-friendly context
    /// (e.g. `tokio::task::spawn_blocking`) when on the async runtime. Safe on the main thread.
    fn save_panel(&self, suggested_name: &str) -> Option<PathBuf> {
        let name = suggested_name.to_owned();
        on_main(self.app(), move |mtm| appkit::save_panel(mtm, &name)).flatten()
    }

    fn convert_image(&self, data: &[u8], to: ConvertTo) -> Option<ConvertedImage> {
        image::convert(data, to)
    }

    fn show_main_window(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |mtm| shell.show_main(mtm));
    }

    fn show_new_ticket_window(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |mtm| shell.show_new_ticket(mtm));
    }

    fn close_new_ticket_window(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |_| shell.close_new_ticket());
    }

    fn show_resolve_window(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |mtm| shell.show_resolve(mtm));
    }

    fn close_resolve_window(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |_| shell.close_resolve());
    }

    fn hide_menubar_panel(&self) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |_| shell.hide_panel());
    }

    fn session_changed(&self, signed_in: bool) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |_| shell.set_signed_in(signed_in));
    }

    fn sidebar_changed(&self, visible: bool) {
        let shell = self.shell.clone();
        on_main_async(self.app(), move |_| {
            shell.state().sidebar_hidden = !visible;
            shell.rebuild_menu();
        });
    }

    fn set_tray(&self, title: &str, accessibility_label: &str) {
        let shell = self.shell.clone();
        let (title, label) = (title.to_owned(), accessibility_label.to_owned());
        on_main_async(self.app(), move |mtm| shell.update_tray(mtm, title, label));
    }

    fn new_ticket_assign_to_me(&self) -> bool {
        self.standard
            .bool(crate::prefs::keys::NEW_TICKET_ASSIGN_TO_ME)
            .unwrap_or(true)
    }

    fn set_new_ticket_assign_to_me(&self, on: bool) {
        self.standard
            .set_bool(crate::prefs::keys::NEW_TICKET_ASSIGN_TO_ME, on);
    }

    fn format_date_template(&self, date: DateTime<Utc>, template: &str) -> String {
        locale::date_template(date, template)
    }

    fn format_date_time(&self, date: DateTime<Utc>, with_date: bool) -> String {
        locale::date_time(date, with_date)
    }

    fn format_relative_named(&self, date: DateTime<Utc>, now: DateTime<Utc>) -> String {
        locale::relative_named(date, now)
    }

    fn format_byte_count(&self, bytes: i64) -> String {
        locale::byte_count(bytes)
    }

    fn format_list(&self, items: &[String]) -> String {
        locale::list(items)
    }

    /// Hops to the main thread and waits; call it from a blocking-friendly context.
    fn pasteboard_images(&self, max_file_bytes: u64) -> Option<Vec<PastedImage>> {
        on_main(self.app(), move |_| {
            files::pasteboard_images(max_file_bytes)
        })
        .flatten()
    }

    fn write_file(&self, path: &std::path::Path, data: &[u8]) -> Result<(), String> {
        files::write_file(path, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn layouts_follow_the_session() {
        assert_eq!(
            Layout::for_session(&json!({"type": "restoring"})),
            Some(Layout::Small)
        );
        assert_eq!(
            Layout::for_session(&json!({"type": "unreachable", "message": "x"})),
            Some(Layout::Small)
        );
        assert_eq!(
            Layout::for_session(&json!({"type": "signedOut", "form": {}})),
            Some(Layout::SignIn)
        );
        assert_eq!(
            Layout::for_session(&json!({"type": "signedIn"})),
            Some(Layout::Shell)
        );
        assert_eq!(Layout::for_session(&Value::Null), None);
        assert_eq!(Layout::Small.fixed_size(), Some((480.0, 320.0)));
        assert_eq!(Layout::SignIn.fixed_size(), Some((480.0, 520.0)));
    }

    #[test]
    fn panel_sits_under_the_icon_inside_the_screen() {
        let screen = (0.0, 0.0, 1512.0, 982.0);
        assert_eq!(
            panel_origin((1000.0, 0.0, 40.0, 24.0), screen),
            (840.0, 28.0)
        );
        assert_eq!(
            panel_origin((1490.0, 0.0, 22.0, 24.0), screen),
            (1512.0 - 360.0 - 8.0, 28.0)
        );
        assert_eq!(panel_origin((2.0, 0.0, 22.0, 24.0), screen), (8.0, 28.0));
    }
}
