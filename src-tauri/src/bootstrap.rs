use crate::app_state::AppState;
use std::path::PathBuf;

pub fn bootstrap_app(base_dir: PathBuf) -> Result<AppState, std::io::Error> {
    AppState::new(base_dir).map_err(|e| std::io::Error::other(e.to_string()))
}

pub fn setup_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.handle().clone();
    crate::event_forwarder::spawn_event_forwarder(app_handle);
    crate::tray::setup_tray(app.handle())?;
    Ok(())
}
