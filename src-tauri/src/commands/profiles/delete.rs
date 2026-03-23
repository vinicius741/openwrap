use std::fs;


use crate::app_state::AppState;
use crate::error::CommandError;
use crate::tray;

use super::parse::parse_profile_id;

#[tauri::command]
pub async fn delete_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), CommandError> {
    let raw_id = parse_profile_id(&profile_id)?;

    let profile = state.profile_repository().get_profile(&raw_id)?;

    state
        .connection_manager
        .disconnect_if_connected(&raw_id)
        .await?;

    if let Err(e) = state.secret_store().delete_password(&raw_id) {
        eprintln!(
            "Failed to delete stored password for profile {}: {}",
            raw_id, e
        );
    }

    if let Some(last_selected) = state.profile_repository().get_last_selected_profile()? {
        if last_selected == raw_id {
            state.profile_repository().set_last_selected_profile(None)?;
        }
    }

    state.profile_repository().delete_profile(&raw_id)?;

    if let Err(e) = fs::remove_dir_all(&profile.profile.managed_dir) {
        eprintln!(
            "Failed to remove managed directory for profile {}: {}",
            raw_id, e
        );
    }

    tray::sync_selected_profile(&app, None);

    Ok(())
}
