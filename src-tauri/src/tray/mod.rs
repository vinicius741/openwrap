mod actions;
mod menu;
mod presenter;
mod target;

pub use presenter::{sync_connection_state, sync_selected_profile};

use crate::app_state::AppState;
use tauri::Manager;

pub fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let (menu, tray_state) = menu::build_tray_menu(app)?;
    app.manage(tray_state);
    menu::attach_tray_icon(app, &menu)?;

    sync_connection_state(app, &app.state::<AppState>().connection_manager.snapshot());
    Ok(())
}
