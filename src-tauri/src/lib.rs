//! iTop Wrap: the Tauri app. Models, commands, tray and native pieces.

pub mod error;
pub mod hub;
pub mod models;
pub mod native;
pub mod router;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("iTop Wrap failed to start");
}
