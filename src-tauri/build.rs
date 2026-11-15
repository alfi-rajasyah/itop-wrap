fn main() {
    // An app ACL manifest: the three commands are reachable only where a capability allows them
    // (capabilities/*.json), which also keeps them from any non-app origin.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["get_snapshot", "dispatch", "open_url"]),
    ))
    .expect("tauri build");
}
