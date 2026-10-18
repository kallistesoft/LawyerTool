//! Tauri shell. Commands are thin: validation and logic live in the crates.

mod session;

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

pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let session = Arc::new(session::Session::new(
                dir.join("lawyertool.db"),
                session::DEFAULT_IDLE,
            ));
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
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        // No sensitive data reaches this point; the message is the error kind.
        eprintln!("LawyerTool failed to start: {e}");
        std::process::exit(1);
    }
}
