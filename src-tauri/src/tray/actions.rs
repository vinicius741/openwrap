use tauri::Manager;

use crate::app_state::AppState;

pub fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn handle_connect(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        if let Some((profile_id, _)) = crate::tray::target::resolve_connect_target(&state) {
            let _ = state
                .connection_manager
                .connect(profile_id.to_string())
                .await;
        }
    });
}

pub fn handle_disconnect(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let _ = state.connection_manager.disconnect().await;
    });
}
