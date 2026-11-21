//! Settings (iTop Wrap addition, not in the Swift app): whether the app shows in the Dock and
//! whether it has a menu bar item. The window is iTop Wrap ▸ Settings… (⌘,) or the menu bar
//! panel's gear. Unlike the workspace models it exists signed in and signed out.
//!
//! At least one of the two always stays on, so the app can't end up with no icon to reach it by.
//! Hiding the Dock icon only takes effect while the menu bar item is actually there (signed in);
//! the shell decides that, this model only holds the choice.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::hub::Hub;
use crate::native::{AppIcons, Native};

pub const MODEL_ID: &str = "settings";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct SettingsSnapshot {
    pub show_in_dock: bool,
    pub show_menu_bar_icon: bool,
    /// False while the menu bar item is off: the Dock icon is then the only way back.
    pub can_hide_dock: bool,
    /// False while the Dock icon is off.
    pub can_hide_menu_bar_icon: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum SettingsIntent {
    /// Opens (or brings forward) the Settings window; from the menu bar panel it also closes the panel.
    Open,
    SetShowInDock {
        on: bool,
    },
    SetShowMenuBarIcon {
        on: bool,
    },
}

/// The settings model: the choice lives in the native layer's defaults.
pub struct Settings {
    hub: Hub,
    native: Arc<dyn Native>,
}

impl Settings {
    pub fn new(hub: Hub, native: Arc<dyn Native>) -> Self {
        let s = Self { hub, native };
        s.publish();
        s
    }

    pub fn snapshot(&self) -> SettingsSnapshot {
        let icons = self.native.app_icons().normalized();
        SettingsSnapshot {
            show_in_dock: icons.dock,
            show_menu_bar_icon: icons.menu_bar,
            can_hide_dock: !icons.dock || icons.menu_bar,
            can_hide_menu_bar_icon: !icons.menu_bar || icons.dock,
        }
    }

    fn publish(&self) {
        self.hub.publish(MODEL_ID, &self.snapshot());
    }

    pub fn dispatch(&self, intent: SettingsIntent) {
        let icons = self.native.app_icons().normalized();
        let next = match intent {
            SettingsIntent::Open => {
                self.native.hide_menubar_panel();
                self.native.show_settings_window();
                return;
            }
            SettingsIntent::SetShowInDock { on } => AppIcons { dock: on, ..icons },
            SettingsIntent::SetShowMenuBarIcon { on } => AppIcons {
                menu_bar: on,
                ..icons
            },
        };
        // Both off would leave no way back to the app: refuse.
        if next != icons && (next.dock || next.menu_bar) {
            self.native.set_app_icons(next);
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::NoSink;
    use crate::native::fake::{Call, FakeNative};

    fn model() -> (Settings, Arc<FakeNative>, Hub) {
        let native = Arc::new(FakeNative::new(chrono::Utc::now()));
        let hub = Hub::new(Arc::new(NoSink));
        (Settings::new(hub.clone(), native.clone()), native, hub)
    }

    fn published(hub: &Hub) -> SettingsSnapshot {
        serde_json::from_value(hub.get(MODEL_ID)).expect("published")
    }

    #[test]
    fn both_icons_show_by_default() {
        let (_, _, hub) = model();
        assert_eq!(
            published(&hub),
            SettingsSnapshot {
                show_in_dock: true,
                show_menu_bar_icon: true,
                can_hide_dock: true,
                can_hide_menu_bar_icon: true,
            }
        );
    }

    #[test]
    fn hiding_one_icon_locks_the_other() {
        let (m, native, hub) = model();
        m.dispatch(SettingsIntent::SetShowInDock { on: false });
        let s = published(&hub);
        assert!(!s.show_in_dock && s.show_menu_bar_icon);
        assert!(s.can_hide_dock && !s.can_hide_menu_bar_icon);
        assert!(native.calls().contains(&Call::SetAppIcons(AppIcons {
            dock: false,
            menu_bar: true
        })));

        // Refused: nothing would be left to click.
        m.dispatch(SettingsIntent::SetShowMenuBarIcon { on: false });
        assert!(published(&hub).show_menu_bar_icon);

        m.dispatch(SettingsIntent::SetShowInDock { on: true });
        m.dispatch(SettingsIntent::SetShowMenuBarIcon { on: false });
        let s = published(&hub);
        assert!(s.show_in_dock && !s.show_menu_bar_icon);
        assert!(!s.can_hide_dock && s.can_hide_menu_bar_icon);
    }

    #[test]
    fn both_off_in_the_defaults_reads_as_dock_on() {
        let icons = AppIcons {
            dock: false,
            menu_bar: false,
        };
        assert_eq!(
            icons.normalized(),
            AppIcons {
                dock: true,
                menu_bar: false
            }
        );
    }

    #[test]
    fn open_shows_the_window_and_closes_the_panel() {
        let (m, native, _) = model();
        m.dispatch(SettingsIntent::Open);
        let calls = native.calls();
        assert!(calls.contains(&Call::HideMenubarPanel));
        assert!(calls.contains(&Call::ShowSettingsWindow));
    }
}
