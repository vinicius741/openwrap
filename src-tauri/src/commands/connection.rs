use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::CommandError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSubmissionDto {
    pub profile_id: String,
    pub username: String,
    pub password: String,
    pub remember_in_keychain: bool,
}

#[tauri::command]
pub async fn connect(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<openwrap_core::connection::ConnectionSnapshot, CommandError> {
    state.connection_manager.connect(profile_id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn submit_credentials(
    state: tauri::State<'_, AppState>,
    request: CredentialSubmissionDto,
) -> Result<openwrap_core::connection::ConnectionSnapshot, CommandError> {
    state
        .connection_manager
        .submit_credentials(openwrap_core::connection::CredentialSubmission {
            profile_id: request
                .profile_id
                .parse::<openwrap_core::profiles::ProfileId>()
                .map_err(|error| openwrap_core::AppError::ConnectionState(error.to_string()))?,
            username: request.username,
            password: request.password,
            remember_in_keychain: request.remember_in_keychain,
        })
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn disconnect(
    state: tauri::State<'_, AppState>,
) -> Result<openwrap_core::connection::ConnectionSnapshot, CommandError> {
    state.connection_manager.disconnect().await.map_err(Into::into)
}

#[tauri::command]
pub fn get_connection_state(
    state: tauri::State<'_, AppState>,
) -> Result<openwrap_core::connection::ConnectionSnapshot, CommandError> {
    Ok(state.connection_manager.snapshot())
}

#[tauri::command]
pub fn get_recent_logs(
    state: tauri::State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<openwrap_core::connection::LogEntry>, CommandError> {
    Ok(state.connection_manager.recent_logs(limit.unwrap_or(200)))
}
