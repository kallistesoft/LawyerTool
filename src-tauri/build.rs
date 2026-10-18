fn main() {
    // Only the commands listed here are callable, and only when a capability
    // grants their `allow-*` permission (see capabilities/default.json).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_info",
            "vault_status",
            "vault_create",
            "vault_unlock",
            "vault_recover",
            "vault_lock",
            "activity",
            "client_list",
            "client_get",
            "client_save",
            "client_delete",
            "assess",
            "deid_preview",
            "export_report",
            "watch_status",
            "watch_check_now",
            "watch_approve",
            "watch_reject",
            "update_check",
            "update_install",
        ]),
    ))
    .unwrap_or_else(|e| panic!("tauri build: {e}"));
}
