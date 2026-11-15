//! The app's models: Rust ports of the Swift view models (contract.md §3, §5).

pub mod app_router;
pub mod cmdb;
pub mod debug;
pub mod detail;
pub mod format;
pub mod list;
pub mod menubar;
pub mod new_ticket;
pub mod options;
pub mod orgs;
pub mod people;
pub mod prefs;
pub mod records;
pub mod seams;
pub mod session;
pub mod sheets;
pub mod shell;
pub mod timer;
pub mod workspace;

pub use app_router::{AppConfig, AppRouter, session_service};
pub use workspace::{Standalone, Workspace};

#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod parity_fix_tests;
