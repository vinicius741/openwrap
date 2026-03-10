use serde::Deserialize;
use tauri::Emitter;

use crate::app_state::AppState;
use crate::error::CommandError;
use crate::events::PROFILES_IMPORT_COMPLETED;
use crate::tray;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProfileRequestDto {
    pub file_path: String,
    pub display_name: Option<String>,
    pub allow_warnings: bool,
}

#[tauri::command]
pub fn import_profile(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: ImportProfileRequestDto,
) -> Result<openwrap_core::profiles::ImportProfileResponse, CommandError> {
    let response =
        state
            .importer
            .import_profile(openwrap_core::profiles::ImportProfileRequest {
                source_path: request.file_path.into(),
                display_name: request.display_name,
                allow_warnings: request.allow_warnings,
            })?;
    app.emit(PROFILES_IMPORT_COMPLETED, &response)
        .map_err(|error| {
            CommandError::from(openwrap_core::AppError::Settings(error.to_string()))
        })?;
    Ok(response)
}

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
    let profile_id: openwrap_core::profiles::ProfileId =
        profile_id.parse().map_err(|error: uuid::Error| {
            openwrap_core::AppError::ConnectionState(error.to_string())
        })?;
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
