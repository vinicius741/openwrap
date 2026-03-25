use crate::app_state::AppState;
use crate::error::CommandError;
use crate::tray;

use super::parse::parse_profile_id;

#[tauri::command]
pub fn list_profiles(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<openwrap_core::profiles::ProfileSummary>, CommandError> {
    state
        .profile_repository()
        .list_profiles()
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_profile(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<openwrap_core::profiles::ProfileDetail, CommandError> {
    let profile_id = parse_profile_id(&profile_id)?;
    state
        .profile_repository()
        .get_profile(&profile_id)
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_last_selected_profile(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, CommandError> {
    state
        .profile_repository()
        .get_last_selected_profile()
        .map(|profile_id| profile_id.map(|profile_id| profile_id.to_string()))
        .map_err(Into::into)
}

#[tauri::command]
pub fn set_last_selected_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    profile_id: Option<String>,
) -> Result<(), CommandError> {
    let parsed = profile_id
        .as_deref()
        .map(str::parse::<openwrap_core::profiles::ProfileId>)
        .transpose()
        .map_err(|error: uuid::Error| {
            openwrap_core::AppError::ConnectionState(error.to_string())
        })?;
    state
        .profile_repository()
        .set_last_selected_profile(parsed.as_ref())?;
    tray::sync_selected_profile(&app, parsed.as_ref());
    Ok(())
}
