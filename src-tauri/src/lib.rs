//! iTop Wrap: the Tauri app. Models, commands, tray and native pieces.

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .run(tauri::generate_context!())
        .expect("iTop Wrap failed to start");
}
