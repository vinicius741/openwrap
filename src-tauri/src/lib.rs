mod app_state;
mod commands;
mod error;
mod events;
mod tray;

use app_state::AppState;
use openwrap_core::connection::CoreEvent;
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let base_dir = dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("OpenWrap");
            let app_state = AppState::new(base_dir)
                .map_err(|error| std::io::Error::other(error.to_string()))?;

            let events_app = app.handle().clone();
            let mut event_rx = app_state.connection_manager.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    match event {
                        CoreEvent::StateChanged(payload) => {
                            crate::tray::sync_connection_state(&events_app, &payload);
                            let _ =
                                events_app.emit(crate::events::CONNECTION_STATE_CHANGED, payload);
                        }
                        CoreEvent::LogLine(payload) => {
                            let _ = events_app.emit(crate::events::CONNECTION_LOG_LINE, payload);
                        }
                        CoreEvent::CredentialsRequested(payload) => {
                            let _ = events_app
                                .emit(crate::events::CONNECTION_CREDENTIALS_REQUESTED, payload);
                        }
                        CoreEvent::DnsObserved(payload) => {
                            let _ =
                                events_app.emit(crate::events::CONNECTION_DNS_OBSERVED, payload);
                        }
                    }
                }
            });

            app.manage(app_state);
            tray::setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles::import_profile,
            commands::profiles::list_profiles,
            commands::profiles::get_profile,
            commands::profiles::get_last_selected_profile,
            commands::profiles::set_last_selected_profile,
            commands::connection::connect,
            commands::connection::submit_credentials,
            commands::connection::disconnect,
            commands::connection::get_connection_state,
            commands::connection::get_recent_logs,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::detect_openvpn,
            commands::settings::reveal_profile_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run OpenWrap");
}
