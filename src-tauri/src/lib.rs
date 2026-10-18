//! Tauri shell. Commands are thin: validation and logic live in the crates.

mod clients;
#[cfg(debug_assertions)]
mod dev;
mod session;
mod updates;

use std::sync::Arc;

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Release builds: the real vault (passphrase, keychain, auto-lock).
#[cfg(not(debug_assertions))]
fn new_session(dir: &std::path::Path) -> Result<session::Session, Box<dyn std::error::Error>> {
    Ok(session::Session::new(
        dir.join("lawyertool.db"),
        session::DEFAULT_IDLE,
    ))
}

/// Dev builds: an automatic test vault in `<data>/dev/`, unless
/// `LAWYERTOOL_REAL_VAULT` is set.
#[cfg(debug_assertions)]
fn new_session(dir: &std::path::Path) -> Result<session::Session, Box<dyn std::error::Error>> {
    if !dev::enabled() {
        return Ok(session::Session::new(
            dir.join("lawyertool.db"),
            session::DEFAULT_IDLE,
        ));
    }
    let dev_dir = dir.join("dev");
    let v = dev::open(&dev_dir)?;
    Ok(session::Session::dev(dev::db_path(&dev_dir), v))
}

pub fn run() {
    let context = tauri::generate_context!();
    let mut builder = tauri::Builder::default();
    // Registered only once the public key and endpoint are configured
    // (see docs/releasing.md); the plugin refuses to start without them.
    if updates::configured(&context.config().plugins) {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    let result = builder
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let session = Arc::new(new_session(&dir)?);
            // The only HTTP client; every call is audited into the vault.
            let client = net::Client::new(
                net::Allowlist::new(session::ALLOWED_HOSTS.iter().copied()),
                session.clone(),
            )?;
            app.manage(session);
            app.manage(client);
            session::spawn_idle_watch(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            session::vault_status,
            session::vault_create,
            session::vault_unlock,
            session::vault_recover,
            session::vault_lock,
            session::activity,
            clients::client_list,
            clients::client_get,
            clients::client_save,
            clients::client_delete,
            clients::assess,
            updates::update_check,
            updates::update_install,
        ])
        .run(context);
    if let Err(e) = result {
        // No sensitive data reaches this point; the message is the error kind.
        eprintln!("LawyerTool failed to start: {e}");
        std::process::exit(1);
    }
}
