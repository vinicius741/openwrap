mod app_state;
mod bootstrap;
mod commands;
mod error;
mod event_forwarder;
mod events;
mod invoke;
mod tray;

use tauri::Manager;

pub fn run() {
    let base_dir = dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("OpenWrap");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_state = bootstrap::bootstrap_app(base_dir)?;
            app.manage(app_state);
            bootstrap::setup_app(app)?;
            Ok(())
        })
        .invoke_handler(invoke_handlers!())
        .run(tauri::generate_context!())
        .expect("failed to run OpenWrap");
}
