//! Everything the models need from the Mac, behind one trait so the models run in tests without
//! Tauri, AppKit or a Keychain. `native::mac` implements it for the app; `native::fake` for tests.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

pub mod fake;
#[cfg(target_os = "macos")]
pub mod mac;

/// An image after intake conversion (LOG-18): what `sips`/ImageIO produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedImage {
    pub data: Vec<u8>,
    /// "png" or "jpg": the new file extension.
    pub ext: &'static str,
}

/// How to convert an image iTop can't take as it is (Sources/ITopWrap/ComposerTextView.swift,
/// ImageIntake): TIFF becomes PNG; everything else JPEG at quality 0.85, at most 2560 px on the
/// long side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertTo {
    Png,
    Jpeg,
}

pub trait Native: Send + Sync {
    /// "Now", so tests can hold the clock still.
    fn now(&self) -> DateTime<Utc>;
    /// Puts a plain string on the general pasteboard (DET-10, CMDB-11).
    fn copy_text(&self, text: &str);
    /// Opens an http, https or mailto URL in the default app. Anything else is refused by the caller.
    fn open_url(&self, url: &str);
    /// Shows the Save panel with `suggested_name` and returns where to write, or `None` on Cancel (DET-33).
    fn save_panel(&self, suggested_name: &str) -> Option<PathBuf>;
    /// Decodes any image macOS can read and re-encodes it; `None` when it isn't an image (LOG-18).
    fn convert_image(&self, data: &[u8], to: ConvertTo) -> Option<ConvertedImage>;
    /// Brings the main window forward and activates the app (KEY-2, MB-18).
    fn show_main_window(&self);
    /// Opens (or brings forward) the separate New Ticket window (NEW-2).
    fn show_new_ticket_window(&self);
    /// Closes the New Ticket window if it is open (NEW-4).
    fn close_new_ticket_window(&self);
    /// Hides the menu bar panel (after one of its actions, MB-17).
    fn hide_menubar_panel(&self);
    /// The session state changed: resize the main window, rebuild the menus, add or remove the tray.
    fn session_changed(&self, signed_in: bool);
    /// The tray's title and accessibility label (MB-1…6).
    fn set_tray(&self, title: &str, accessibility_label: &str);
}
