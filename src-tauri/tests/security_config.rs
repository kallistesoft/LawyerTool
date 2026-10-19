//! Frozen security configuration: any widening of the window's permissions,
//! the CSP or the global API surface must be a deliberate change to this test.

use serde_json::Value;

fn json(path: &str) -> Value {
    let text = std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn window_permissions_are_exactly_our_commands() {
    let cap = json("capabilities/default.json");
    let mut perms: Vec<&str> = cap["permissions"]
        .as_array()
        .unwrap_or_else(|| unreachable!())
        .iter()
        .filter_map(Value::as_str)
        .collect();
    perms.sort_unstable();
    let mut expected = vec![
        "allow-activity",
        "allow-app-info",
        "allow-assess",
        "allow-client-delete",
        "allow-client-get",
        "allow-client-list",
        "allow-client-save",
        "allow-deid-preview",
        "allow-export-report",
        "allow-inbox-import",
        "allow-inbox-list",
        "allow-settings-clear",
        "allow-settings-get",
        "allow-settings-pick",
        "allow-update-check",
        "allow-update-install",
        "allow-vault-create",
        "allow-vault-lock",
        "allow-vault-recover",
        "allow-vault-status",
        "allow-vault-unlock",
        "allow-watch-approve",
        "allow-watch-check-now",
        "allow-watch-reject",
        "allow-watch-status",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
    ];
    expected.sort_unstable();
    assert_eq!(
        perms, expected,
        "no plugin (fs, shell, http, dialog…) permissions in the WebView"
    );
    assert_eq!(cap["windows"], serde_json::json!(["main"]));
}

#[test]
fn csp_and_globals_stay_locked_down() {
    let conf = json("tauri.conf.json");
    let app = &conf["app"];
    assert_eq!(app["withGlobalTauri"], false);
    assert_eq!(app["security"]["freezePrototype"], true);
    let csp = &app["security"]["csp"];
    assert_eq!(csp["default-src"], "'self'");
    assert_eq!(
        csp["script-src"], "'self'",
        "no unsafe-eval / inline scripts / remote scripts"
    );
    assert_eq!(csp["object-src"], "'none'");
    assert_eq!(csp["frame-ancestors"], "'none'");
    let connect = csp["connect-src"].as_str().unwrap_or_default();
    assert!(
        !connect.contains("http://") || connect == "ipc: http://ipc.localhost",
        "{connect}"
    );
    assert!(
        !connect.contains("https://"),
        "the WebView never talks to the network: {connect}"
    );
    // No remote content and no extra windows.
    assert_eq!(app["windows"].as_array().map(Vec::len), Some(1));
    assert!(
        conf["build"]["frontendDist"]
            .as_str()
            .is_some_and(|d| !d.starts_with("http"))
    );
}

#[test]
fn windows_installer_is_standalone() {
    let conf = json("tauri.conf.json");
    assert_eq!(
        conf["bundle"]["windows"]["webviewInstallMode"]["type"],
        "offlineInstaller"
    );
}
