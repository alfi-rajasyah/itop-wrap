//! Small AppKit calls Tauri doesn't offer. Everything that touches AppKit objects runs on the main
//! thread: [`on_main`] / [`on_main_async`] hop there through `AppHandle::run_on_main_thread`.

use std::path::PathBuf;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSModalResponseOK,
    NSPanel, NSPasteboard, NSPasteboardTypeString, NSResponder, NSSavePanel, NSStatusItem,
    NSStatusWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString, NSURL};
use tauri::{AppHandle, Runtime, WebviewWindow};

/// Runs `f` on the main thread and waits for its result. Runs it inline when already there, so it
/// never deadlocks the main thread; from another thread it blocks that thread until `f` is done.
pub fn on_main<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(MainThreadMarker) -> T + Send + 'static,
) -> Option<T> {
    if let Some(mtm) = MainThreadMarker::new() {
        return Some(f(mtm));
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        if let Some(mtm) = MainThreadMarker::new() {
            let _ = tx.send(f(mtm));
        }
    })
    .ok()?;
    rx.recv().ok()
}

/// Runs `f` on the main thread without waiting (inline when already there).
pub fn on_main_async<R: Runtime>(
    app: &AppHandle<R>,
    f: impl FnOnce(MainThreadMarker) + Send + 'static,
) {
    if let Some(mtm) = MainThreadMarker::new() {
        f(mtm);
        return;
    }
    if let Err(e) = app.run_on_main_thread(move || {
        if let Some(mtm) = MainThreadMarker::new() {
            f(mtm);
        }
    }) {
        eprintln!("couldn't reach the main thread: {e}");
    }
}

/// Plain text onto the general pasteboard (DET-10, CMDB-11).
pub fn copy_text(_mtm: MainThreadMarker, text: &str) {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    // SAFETY: an immutable NSString constant exported by AppKit.
    let kind = unsafe { NSPasteboardTypeString };
    pb.setString_forType(&NSString::from_str(text), kind);
}

/// Opens a URL in its default app (the caller has checked the scheme).
pub fn open_url(_mtm: MainThreadMarker, url: &str) {
    match NSURL::URLWithString(&NSString::from_str(url)) {
        Some(u) => {
            NSWorkspace::sharedWorkspace().openURL(&u);
        }
        None => eprintln!("open_url: not a URL: {url}"),
    }
}

/// The Save panel (DET-33): modal, with `suggested_name`; `None` on Cancel.
pub fn save_panel(mtm: MainThreadMarker, suggested_name: &str) -> Option<PathBuf> {
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&NSString::from_str(suggested_name));
    activate(mtm);
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel.URL()?.path().map(|p| PathBuf::from(p.to_string()))
}

/// `ITOPWRAP_STDERR=<file>` (debug builds): `freopen(path, "a", stderr)`.
pub fn redirect_stderr(path: &str) {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn dup2(fd: i32, fd2: i32) -> i32;
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        // SAFETY: both are open descriptors; dup2 makes 2 (stderr) refer to the file, which
        // stays open through fd 2 after `file` is dropped.
        Ok(file) => unsafe {
            dup2(file.as_raw_fd(), 2);
        },
        Err(e) => eprintln!("ITOPWRAP_STDERR: can't open {path}: {e}"),
    }
}

/// `ITOPWRAP_APPEARANCE=light|dark` (debug builds): `NSApp.appearance = .aqua / .darkAqua`.
pub fn force_appearance(mtm: MainThreadMarker, dark: bool) {
    // SAFETY: immutable NSString constants exported by AppKit.
    let name = unsafe {
        if dark {
            objc2_app_kit::NSAppearanceNameDarkAqua
        } else {
            objc2_app_kit::NSAppearanceNameAqua
        }
    };
    let appearance = objc2_app_kit::NSAppearance::appearanceNamed(name);
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
}

/// System Settings ▸ Accessibility ▸ Display ▸ Reduce motion.
pub fn reduce_motion(_mtm: MainThreadMarker) -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Brings the app to the front (`NSApp.activate()`).
pub fn activate(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    // `activate()` is macOS 14+; the app supports macOS 13.
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

/// The `NSWindow` behind a Tauri window.
fn ns_window<R: Runtime>(
    _mtm: MainThreadMarker,
    window: &WebviewWindow<R>,
) -> Option<Retained<NSWindow>> {
    let ptr = window.ns_window().ok()?.cast::<NSWindow>();
    // SAFETY: Tauri returns the window's live NSWindow; we are on the main thread, and retaining
    // it keeps it alive for as long as we hold it.
    unsafe { Retained::retain(ptr) }
}

/// Keeps a window out of the Window menu (the New Ticket window and the panel, contract.md §6).
pub fn exclude_from_windows_menu<R: Runtime>(mtm: MainThreadMarker, window: &WebviewWindow<R>) {
    if let Some(w) = ns_window(mtm, window) {
        w.setExcludedFromWindowsMenu(true);
    }
}

/// The name SwiftUI autosaved the main window's frame under ("NSWindow Frame main" in the
/// standard defaults domain), reused so the Rust app opens where the Swift app was (PERS-12).
pub const MAIN_FRAME_AUTOSAVE: &str = "main";

/// Puts the window at its autosaved frame `name`; false when none is saved.
pub fn restore_frame<R: Runtime>(
    mtm: MainThreadMarker,
    window: &WebviewWindow<R>,
    name: &str,
) -> bool {
    ns_window(mtm, window).is_some_and(|w| w.setFrameUsingName(&NSString::from_str(name)))
}

/// Starts (`Some`) or stops (`None`) AppKit saving the window's frame under `name` on every change.
pub fn autosave_frame<R: Runtime>(
    mtm: MainThreadMarker,
    window: &WebviewWindow<R>,
    name: Option<&str>,
) {
    if let Some(w) = ns_window(mtm, window) {
        w.setFrameAutosaveName(&NSString::from_str(name.unwrap_or("")));
    }
}

define_class!(
    // SAFETY: NSPanel has no subclassing requirements for these two overrides, and the class has
    // no ivars and no Drop.
    #[unsafe(super(NSPanel, NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "ITopWrapMenuBarPanel"]
    struct MenuBarPanel;

    unsafe impl NSObjectProtocol for MenuBarPanel {}

    impl MenuBarPanel {
        /// Borderless windows can't become key by default; the panel must, for its search field.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

/// Turns the menu bar window into a non-activating panel, like `MenuBarExtra(.window)`: it takes
/// keyboard focus without bringing the app (and its main window) forward, sits above other
/// windows, follows you to the active Space and shows over full-screen apps, and stays out of
/// the Window menu and ⌘`. Also rounds its corners natively (10 pt).
pub fn make_menubar_panel<R: Runtime>(mtm: MainThreadMarker, window: &WebviewWindow<R>) {
    let Some(w) = ns_window(mtm, window) else {
        return;
    };
    let ptr: *const NSWindow = &*w;
    if !panel_swap_is_sound(w.class()) {
        eprintln!("menu bar panel: the window's class can't be swapped safely; left as it is");
        return;
    }
    // SAFETY: checked by `panel_swap_is_sound`: the live object is tao's `TaoWindow`, an NSWindow
    // subclass whose only ivar (`WindowState { focusable: AtomicBool }`, no Drop side effects)
    // sits after NSWindow's ivars, and NSPanel adds no ivars of its own, so NSPanel's methods never
    // touch tao's ivar and tao still reads it at its fixed offset in the allocation (which keeps
    // TaoWindow's size). TaoWindow's overrides that the swap drops are `canBecomeKeyWindow` /
    // `canBecomeMainWindow` (MenuBarPanel answers those) and `sendEvent:` (only for
    // `movableByWindowBackground` drags, which the panel doesn't use). MenuBarPanel has no ivars.
    // The panel must be an NSPanel for the non-activating style, so it can't subclass TaoWindow.
    // Main thread.
    unsafe {
        objc2::ffi::object_setClass(
            ptr.cast_mut().cast::<AnyObject>(),
            MenuBarPanel::class() as *const AnyClass,
        );
    }
    w.setStyleMask(w.styleMask() | NSWindowStyleMask::NonactivatingPanel);
    w.setLevel(NSStatusWindowLevel);
    w.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::Transient,
    );
    w.setHidesOnDeactivate(false);
    w.setExcludedFromWindowsMenu(true);
    w.setHasShadow(true);
    if let Some(view) = w.contentView() {
        view.setWantsLayer(true);
        if let Some(layer) = view.layer() {
            layer.setCornerRadius(10.0);
            layer.setMasksToBounds(true);
        }
    }
}

/// Whether swapping `original` (the window's runtime class) for `MenuBarPanel` keeps the object
/// valid: `original` is tao's `TaoWindow` directly under NSWindow, NSPanel and MenuBarPanel add no
/// ivars to NSWindow's, and the allocation (TaoWindow's size) is at least as big as the new class.
fn panel_swap_is_sound(original: &AnyClass) -> bool {
    let window = NSWindow::class();
    let is_tao = original.name().to_str() == Ok("TaoWindow")
        && original
            .superclass()
            .is_some_and(|s| std::ptr::eq(s, window));
    is_tao
        && NSPanel::class().instance_size() == window.instance_size()
        && MenuBarPanel::class().instance_size() == window.instance_size()
        && original.instance_size() >= MenuBarPanel::class().instance_size()
}

/// Shows the panel and gives it keyboard focus without activating the app.
pub fn show_panel<R: Runtime>(mtm: MainThreadMarker, window: &WebviewWindow<R>) {
    if let Some(w) = ns_window(mtm, window) {
        w.orderFrontRegardless();
        w.makeKeyWindow();
    }
}

/// Whether the panel is the key window.
pub fn is_key<R: Runtime>(mtm: MainThreadMarker, window: &WebviewWindow<R>) -> bool {
    ns_window(mtm, window).is_some_and(|w| w.isKeyWindow())
}

/// The status item's VoiceOver label (MB-5) and its pressed look while the panel is open.
pub fn status_item_label(mtm: MainThreadMarker, item: &NSStatusItem, label: &str) {
    if let Some(button) = item.button(mtm) {
        button.setAccessibilityLabel(Some(&NSString::from_str(label)));
    }
}

pub fn status_item_highlight(mtm: MainThreadMarker, item: &NSStatusItem, on: bool) {
    if let Some(button) = item.button(mtm) {
        button.highlight(on);
    }
}

/// Which panel shortcut a key-down is (KEY-8), if any: ⌘N, ⌘O, ⌘R with no other modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKey {
    NewTicket,
    OpenMain,
    Refresh,
}

pub fn panel_key(chars: &str, flags: NSEventModifierFlags) -> Option<PanelKey> {
    let mods = flags
        & (NSEventModifierFlags::Command
            | NSEventModifierFlags::Shift
            | NSEventModifierFlags::Option
            | NSEventModifierFlags::Control);
    if mods != NSEventModifierFlags::Command {
        return None;
    }
    match chars {
        "n" => Some(PanelKey::NewTicket),
        "o" => Some(PanelKey::OpenMain),
        "r" => Some(PanelKey::Refresh),
        _ => None,
    }
}

/// Catches ⌘N / ⌘O / ⌘R while `is_panel_key()` says the panel is key, before the web view and the
/// main menu see them, and hands them to `on_key`. Installed once for the app's life.
pub fn install_panel_key_monitor(
    _mtm: MainThreadMarker,
    is_panel_key: impl Fn() -> bool + 'static,
    on_key: impl Fn(PanelKey) + 'static,
) {
    let block = block2::RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        // SAFETY: AppKit passes a valid event for the duration of the handler.
        let e = unsafe { event.as_ref() };
        let chars = e
            .charactersIgnoringModifiers()
            .map(|s| s.to_string().to_lowercase())
            .unwrap_or_default();
        if let Some(key) = panel_key(&chars, e.modifierFlags())
            && is_panel_key()
        {
            on_key(key);
            return std::ptr::null_mut();
        }
        event.as_ptr()
    });
    // SAFETY: the handler returns the event or null (swallow), as the API requires; AppKit keeps
    // the monitor (and the block) for the life of the app since we never remove it.
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
    };
    std::mem::forget(monitor);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review finding V: NSPanel (and our panel class) add no ivars to NSWindow on this macOS,
    /// so the runtime class swap can't overlap tao's ivar.
    #[test]
    fn panel_class_adds_no_ivars() {
        let window = NSWindow::class().instance_size();
        assert_eq!(NSPanel::class().instance_size(), window);
        assert_eq!(MenuBarPanel::class().instance_size(), window);
        assert!(!panel_swap_is_sound(NSWindow::class()), "only tao's window");
    }

    #[test]
    fn panel_shortcuts() {
        let cmd = NSEventModifierFlags::Command;
        assert_eq!(panel_key("n", cmd), Some(PanelKey::NewTicket));
        assert_eq!(
            panel_key("o", cmd | NSEventModifierFlags::CapsLock),
            Some(PanelKey::OpenMain)
        );
        assert_eq!(panel_key("r", cmd), Some(PanelKey::Refresh));
        assert_eq!(panel_key("r", cmd | NSEventModifierFlags::Shift), None);
        assert_eq!(panel_key("r", NSEventModifierFlags::empty()), None);
        assert_eq!(panel_key("t", cmd), None);
    }
}
