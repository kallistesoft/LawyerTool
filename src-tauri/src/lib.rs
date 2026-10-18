//! Tauri shell. Commands are thin: validation and logic live in the crates.

use serde::Serialize;

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
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        // No sensitive data reaches this point; the message is the error kind.
        eprintln!("LawyerTool failed to start: {e}");
        std::process::exit(1);
    }
}
