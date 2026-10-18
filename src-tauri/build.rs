fn main() {
    // Only the commands listed here are callable, and only when a capability
    // grants their `allow-*` permission (see capabilities/default.json).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["app_info"])),
    )
    .unwrap_or_else(|e| panic!("tauri build: {e}"));
}
