//! Everything the models need from the Mac, behind one trait so the models run in tests without
//! Tauri, AppKit or a Keychain. `native::mac` implements it for the app; `native::fake` for tests.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

pub mod fake;
#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(all(test, target_os = "macos"))]
mod quarantine_tests {
    #[test]
    fn saved_files_are_marked_as_downloaded() {
        let dir = std::env::temp_dir().join(format!("itopwrap-q-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("vpn.log");
        std::fs::write(&file, b"x").unwrap();
        super::quarantine_download(&file);
        let out = std::process::Command::new("/usr/bin/xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(&file)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.starts_with("0081;") && text.contains(";iTop Wrap;"),
            "{text}"
        );
    }
}

/// Marks a file saved from iTop as downloaded (`com.apple.quarantine`), as a browser does, so
/// Gatekeeper asks before an app or script from a ticket first runs. Best effort; no-op elsewhere.
pub fn quarantine_download(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        unsafe extern "C" {
            fn setxattr(
                path: *const std::ffi::c_char,
                name: *const std::ffi::c_char,
                value: *const std::ffi::c_void,
                size: usize,
                position: u32,
                options: i32,
            ) -> i32;
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let value = format!("0081;{secs:08x};iTop Wrap;");
        let (Ok(p), Ok(name)) = (
            CString::new(path.as_os_str().as_bytes()),
            CString::new("com.apple.quarantine"),
        ) else {
            return;
        };
        // SAFETY: valid NUL-terminated strings and a buffer of `value.len()` bytes for the call.
        let rc = unsafe {
            setxattr(
                p.as_ptr(),
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                0,
            )
        };
        if rc != 0 {
            eprintln!("couldn't mark the saved file as downloaded");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
}

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
    /// Opens (or brings forward) the separate Resolve window the menu bar uses.
    fn show_resolve_window(&self);
    /// Closes the Resolve window if it is open.
    fn close_resolve_window(&self);
    /// Hides the menu bar panel (after one of its actions, MB-17).
    fn hide_menubar_panel(&self);
    /// The session state changed: resize the main window, rebuild the menus, add or remove the tray.
    fn session_changed(&self, signed_in: bool);
    /// The sidebar was hidden or shown: View ▸ "Hide Sidebar" / "Show Sidebar" follows.
    fn sidebar_changed(&self, visible: bool);
    /// The tray's title and accessibility label (MB-1…6).
    fn set_tray(&self, title: &str, accessibility_label: &str);
    /// New ticket's "Assign to me": UserDefaults `newTicket.assignToMe` in the standard domain,
    /// default true (NEW-12, PERS-8).
    fn new_ticket_assign_to_me(&self) -> bool;
    /// Remembers "Assign to me" at once (NEW-12, PERS-8).
    fn set_new_ticket_assign_to_me(&self, on: bool);

    // Locale-dependent text: Foundation's formatters with the Mac's current locale and time zone,
    // exactly as each Swift call site uses them (contract.md open point 5).

    /// `DateFormatter` with `setLocalizedDateFormatFromTemplate(template)` ("HH:mm" is used as
    /// the fixed format), as `LogDate.format` does (K/CaseLog.swift, DET-39).
    fn format_date_template(&self, date: DateTime<Utc>, template: &str) -> String;
    /// `date.formatted(date: .abbreviated, time: .shortened)` when `with_date`, else
    /// `date.formatted(date: .omitted, time: .shortened)` (LIST-54, TMR-16).
    fn format_date_time(&self, date: DateTime<Utc>, with_date: bool) -> String;
    /// `date.formatted(.relative(presentation: .named))` with `now` as the reference (MB-16).
    fn format_relative_named(&self, date: DateTime<Utc>, now: DateTime<Utc>) -> String;
    /// `ByteCountFormatter.string(fromByteCount:countStyle: .file)` (LOG-20, DET-33).
    fn format_byte_count(&self, bytes: i64) -> String;
    /// `ListFormatter.localizedString(byJoining:)` (CMDB-14).
    fn format_list(&self, items: &[String]) -> String;

    /// The general pasteboard read like `ImageIntake.items(from: .general, preferText: true)`
    /// (W/ComposerTextView.swift, LOG-14): image file URLs first (files over `max_file_bytes`
    /// are not read); file URLs without images, or any text, → `None` (paste normally);
    /// otherwise PNG, then TIFF data. `None` when there are no images.
    fn pasteboard_images(&self, max_file_bytes: u64) -> Option<Vec<PastedImage>>;

    /// Writes a file the way `Data.write(to:)` does; the error is Foundation's
    /// `localizedDescription` (DET-34).
    fn write_file(&self, path: &std::path::Path, data: &[u8]) -> Result<(), String>;
}

/// One image read from the pasteboard (`ImageIntake.Item`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastedImage {
    pub filename: String,
    /// `None` when the file is too big to read, or couldn't be read.
    pub data: Option<Vec<u8>>,
    /// The file's size (0 when unknown); raw data's length.
    pub size: u64,
}
