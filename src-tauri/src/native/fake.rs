//! `Native` for tests: a clock you set, and a record of everything the models asked the Mac to do.

use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

use super::{ConvertTo, ConvertedImage, Native};

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
}

impl FakeNative {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
            calls: Mutex::default(),
            save_to: Mutex::default(),
            converted: Mutex::default(),
            assign_to_me: Mutex::new(true),
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
}
