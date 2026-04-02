mod app_state;
mod bootstrap;
mod commands;
mod error;
mod event_forwarder;
mod events;
mod invoke;
mod tray;

use std::panic::{catch_unwind, AssertUnwindSafe};

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
            let result = catch_unwind(AssertUnwindSafe(|| {
                let app_state = bootstrap::bootstrap_app(base_dir)?;
                app.manage(app_state);
                bootstrap::setup_app(app)?;
                Ok(())
            }));

            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(panic_payload) => {
                    let msg = panic_payload
                        .downcast_ref::<String>()
                        .map(|s| s.as_str())
                        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                        .unwrap_or("unknown panic during setup");
                    eprintln!("Fatal: setup panicked: {msg}");
                    Err(Box::new(std::io::Error::other(msg)))
                }
            }
        })
        .invoke_handler(invoke_handlers!())
        .run(tauri::generate_context!())
        .expect("failed to run OpenWrap");
}
