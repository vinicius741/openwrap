use crate::app_state::AppState;
use std::path::PathBuf;
use tauri::Manager;

pub fn bootstrap_app(base_dir: PathBuf) -> Result<AppState, std::io::Error> {
    AppState::new(base_dir).map_err(|e| std::io::Error::other(e.to_string()))
}

pub fn setup_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();
    crate::event_forwarder::spawn_event_forwarder(app_handle);
    crate::tray::setup_tray(app.handle())?;
    spawn_log_retention(
        app.state::<AppState>()
            .connection_manager
            .session_log()
            .clone(),
    );
    Ok(())
}

fn spawn_log_retention(logs: openwrap_core::logging::SharedSessionLogManager) {
    tauri::async_runtime::spawn(async move {
        loop {
            let logs = logs.clone();
            let result =
                tauri::async_runtime::spawn_blocking(move || logs.enforce_retention()).await;
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => eprintln!("[logging] Retention failed: {error}"),
                Err(error) => eprintln!("[logging] Retention task failed: {error}"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}
