//! `Native` for tests: a clock you set, and a record of everything the models asked the Mac to do.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};

use super::{ConvertTo, ConvertedImage, Native, PastedImage};
use crate::models::format::en_us;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    CopyText(String),
    OpenUrl(String),
    SavePanel(String),
    ShowMainWindow,
    ShowNewTicketWindow,
    CloseNewTicketWindow,
    HideMenubarPanel,
    SessionChanged(bool),
    SetTray(String, String),
    ReadPasteboard,
    SidebarChanged(bool),
}

pub struct FakeNative {
    now: Mutex<DateTime<Utc>>,
    calls: Mutex<Vec<Call>>,
    /// What the Save panel answers; `None` is Cancel.
    pub save_to: Mutex<Option<PathBuf>>,
    /// What `convert_image` answers.
    pub converted: Mutex<Option<ConvertedImage>>,
    /// The remembered `newTicket.assignToMe` (default true).
    pub assign_to_me: Mutex<bool>,
    /// What `pasteboard_images` answers (`None`: no images, paste text normally).
    pub pasteboard: Mutex<Option<Vec<PastedImage>>>,
    /// When set, `write_file` fails with this text.
    pub write_failure: Mutex<Option<String>>,
}

impl FakeNative {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
            calls: Mutex::default(),
            save_to: Mutex::default(),
            converted: Mutex::default(),
            assign_to_me: Mutex::new(true),
            pasteboard: Mutex::default(),
            write_failure: Mutex::default(),
        }
    }
    pub fn set_now(&self, now: DateTime<Utc>) {
        *self.now.lock().unwrap_or_else(|e| e.into_inner()) = now;
    }
    pub fn advance(&self, by: chrono::Duration) {
        let mut now = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *now += by;
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn record(&self, call: Call) {
        self.calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(call);
    }
}

impl Native for FakeNative {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn copy_text(&self, text: &str) {
        self.record(Call::CopyText(text.to_owned()));
    }
    fn open_url(&self, url: &str) {
        self.record(Call::OpenUrl(url.to_owned()));
    }
    fn save_panel(&self, suggested_name: &str) -> Option<PathBuf> {
        self.record(Call::SavePanel(suggested_name.to_owned()));
        self.save_to
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn convert_image(&self, _: &[u8], _: ConvertTo) -> Option<ConvertedImage> {
        self.converted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn show_main_window(&self) {
        self.record(Call::ShowMainWindow);
    }
    fn show_new_ticket_window(&self) {
        self.record(Call::ShowNewTicketWindow);
    }
    fn close_new_ticket_window(&self) {
        self.record(Call::CloseNewTicketWindow);
    }
    fn hide_menubar_panel(&self) {
        self.record(Call::HideMenubarPanel);
    }
    fn session_changed(&self, signed_in: bool) {
        self.record(Call::SessionChanged(signed_in));
    }
    fn sidebar_changed(&self, visible: bool) {
        self.record(Call::SidebarChanged(visible));
    }
    fn set_tray(&self, title: &str, accessibility_label: &str) {
        self.record(Call::SetTray(
            title.to_owned(),
            accessibility_label.to_owned(),
        ));
    }
    fn new_ticket_assign_to_me(&self) -> bool {
        *self.assign_to_me.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn set_new_ticket_assign_to_me(&self, on: bool) {
        *self.assign_to_me.lock().unwrap_or_else(|e| e.into_inner()) = on;
    }
    fn format_date_template(&self, date: DateTime<Utc>, template: &str) -> String {
        en_us::date_template(date, template)
    }
    fn format_date_time(&self, date: DateTime<Utc>, with_date: bool) -> String {
        en_us::date_time(date, with_date)
    }
    fn format_relative_named(&self, date: DateTime<Utc>, now: DateTime<Utc>) -> String {
        en_us::relative_named(date, now)
    }
    fn format_byte_count(&self, bytes: i64) -> String {
        en_us::byte_count(bytes)
    }
    fn format_list(&self, items: &[String]) -> String {
        en_us::list(items)
    }
    fn pasteboard_images(&self, _max_file_bytes: u64) -> Option<Vec<PastedImage>> {
        self.record(Call::ReadPasteboard);
        self.pasteboard
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), String> {
        let failure = self
            .write_failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match failure {
            Some(text) => Err(text),
            None => std::fs::write(path, data).map_err(|e| e.to_string()),
        }
    }
}
