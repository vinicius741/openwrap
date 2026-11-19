//! Tauri commands for log management

use crate::app_state::AppState;
use crate::error::CommandError;

#[tauri::command]
pub fn reveal_logs_folder(state: tauri::State<'_, AppState>) -> Result<(), CommandError> {
    let logs_dir = state.paths.sessions_logs_dir();

    super::reveal::reveal_in_file_manager(&logs_dir)
}

#[tauri::command]
pub fn get_recent_sessions(
    state: tauri::State<'_, AppState>,
    limit: usize,
) -> Result<Vec<openwrap_core::logging::SessionSummary>, CommandError> {
    state
        .connection_manager
        .session_log()
        .get_recent_sessions(limit)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn cleanup_old_logs(
    state: tauri::State<'_, AppState>,
    max_age_days: u32,
) -> Result<u64, CommandError> {
    state
        .connection_manager
        .session_log()
        .cleanup_old_sessions(max_age_days)
        .map_err(CommandError::from)
}
