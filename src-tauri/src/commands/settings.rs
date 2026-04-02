use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperStatus {
    pub helper_path: String,
    pub installed: bool,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn check_helper_status() -> HelperStatus {
    use crate::app_state::backend_factory::resolve_helper_binary;

    let helper_path = resolve_helper_binary();
    let path_str = helper_path.display().to_string();

    if !helper_path.exists() {
        return HelperStatus {
            helper_path: path_str,
            installed: false,
            reason: Some(
                "Helper binary not found. Build it with: cargo build -p openwrap-helper".into(),
            ),
        };
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        match std::fs::metadata(&helper_path) {
            Ok(metadata) => {
                let owner_is_root = metadata.uid() == 0;
                let setuid = (metadata.permissions().mode() & 0o4000) != 0;
                if owner_is_root && setuid {
                    HelperStatus {
                        helper_path: path_str,
                        installed: true,
                        reason: None,
                    }
                } else {
                    let mut reasons = Vec::new();
                    if !owner_is_root {
                        reasons.push("not owned by root");
                    }
                    if !setuid {
                        reasons.push("setuid bit not set");
                    }
                    HelperStatus {
                        helper_path: path_str,
                        installed: false,
                        reason: Some(format!("Helper exists but is {}", reasons.join(" and "))),
                    }
                }
            }
            Err(err) => HelperStatus {
                helper_path: path_str,
                installed: false,
                reason: Some(format!("Cannot read helper metadata: {err}")),
            },
        }
    }

    #[cfg(not(unix))]
    {
        HelperStatus {
            helper_path: path_str,
            installed: false,
            reason: Some("Privileged helper is only supported on macOS.".into()),
        }
    }
}

#[tauri::command]
pub fn install_helper() -> Result<HelperStatus, CommandError> {
    use crate::app_state::backend_factory::resolve_helper_binary;

    let helper_path = resolve_helper_binary();
    let path_str = helper_path.display().to_string();

    if !helper_path.exists() {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            format!(
                "Helper binary not found at {path_str}. Build it first with: cargo build -p openwrap-helper"
            ),
        )));
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            "Privileged helper installation is only supported on macOS.".into(),
        )));
    }

    #[cfg(target_os = "macos")]
    {
        let escaped = path_str.replace('\'', "'\\''");
        let script = format!(
        "do shell script \"chown root:wheel '{}' && chmod 4755 '{}'\" with administrator privileges",
        escaped, escaped
    );

        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|err| {
                CommandError::from(openwrap_core::AppError::Settings(format!(
                    "Failed to run osascript: {err}"
                )))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.contains("User canceled") {
                "Authentication was cancelled.".into()
            } else if stderr.is_empty() {
                "Failed to install helper.".into()
            } else {
                stderr
            };
            return Err(CommandError::from(openwrap_core::AppError::Settings(
                message,
            )));
        }

        Ok(check_helper_status())
    } // cfg(target_os = "macos")
}
