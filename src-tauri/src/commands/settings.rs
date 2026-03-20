use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::CommandError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub openvpn_path_override: Option<String>,
    #[serde(default)]
    pub verbose_logging: bool,
}

#[tauri::command]
pub fn get_settings(
    state: tauri::State<'_, AppState>,
) -> Result<openwrap_core::openvpn::runtime::Settings, CommandError> {
    state
        .profile_repository()
        .get_settings()
        .map_err(Into::into)
}

#[tauri::command]
pub fn update_settings(
    state: tauri::State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<openwrap_core::openvpn::runtime::Settings, CommandError> {
    let settings = openwrap_core::openvpn::runtime::Settings {
        openvpn_path_override: patch.openvpn_path_override.map(PathBuf::from),
        verbose_logging: patch.verbose_logging,
    };
    state.profile_repository().save_settings(&settings)?;

    // Propagate verbose logging setting to connection manager
    state
        .connection_manager
        .set_verbose_logging(settings.verbose_logging);

    Ok(settings)
}

#[tauri::command]
pub fn detect_openvpn(
    state: tauri::State<'_, AppState>,
) -> Result<openwrap_core::openvpn::runtime::OpenVpnDetection, CommandError> {
    let settings = state.profile_repository().get_settings()?;
    Ok(openwrap_core::detect_openvpn_binaries(
        settings.openvpn_path_override,
    ))
}

#[tauri::command]
pub fn reveal_profile_in_finder(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<(), CommandError> {
    let profile_id: openwrap_core::profiles::ProfileId =
        profile_id.parse().map_err(|error: uuid::Error| {
            openwrap_core::AppError::ConnectionState(error.to_string())
        })?;
    let profile = state.profile_repository().get_profile(&profile_id)?;
    Command::new("/usr/bin/open")
        .arg("-R")
        .arg(profile.profile.managed_ovpn_path)
        .status()
        .map_err(|error| CommandError::from(openwrap_core::AppError::Io(error)))?;
    Ok(())
}
