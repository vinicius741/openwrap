use serde::Deserialize;
use tauri::Emitter;

use crate::app_state::AppState;
use crate::error::CommandError;
use crate::events::PROFILES_IMPORT_COMPLETED;

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
