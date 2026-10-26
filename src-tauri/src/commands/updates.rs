//! In-app updates from GitHub Releases.
//!
//! Releases are built by `.github/workflows/release.yml` and signed with the
//! project's minisign updater key (free, independent of Apple signing). The app
//! only installs archives whose signature matches the public key embedded in
//! `tauri.conf.json`.
//!
//! The setuid helper in `/Library/PrivilegedHelperTools` is NOT replaced by an
//! update (installing it needs `sudo`; see `scripts/share/setup.sh`).
//! `get_app_info` reports when the installed helper differs from the one bundled
//! in the running app so the UI can show the reinstall command.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use openwrap_core::errors::UserFacingError;

use crate::app_state::backend_factory::installed_helper_path;
use crate::app_state::AppState;
use crate::error::CommandError;
use crate::events::UPDATE_DOWNLOAD_PROGRESS;

const BUNDLED_HELPER_RESOURCE: &str = "openwrap-helper-bundled";

/// Update found by the last `check_for_update`, consumed by `install_update`.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperStatus {
    /// Installed helper is byte-identical to the one bundled in this app.
    Current,
    /// Installed helper differs from the bundled one (app was updated).
    Outdated,
    /// No helper installed at the expected path.
    Missing,
    /// Dev build or custom `OPENWRAP_HELPER_PATH`; nothing to compare.
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub helper_status: HelperStatus,
    /// Shell command that installs the bundled helper, when one is bundled.
    pub helper_install_command: Option<String>,
}

fn update_error(title: &str, error: impl std::fmt::Display) -> CommandError {
    let message = error.to_string();
    CommandError {
        message: message.clone(),
        user_facing: UserFacingError {
            code: "update_failed".into(),
            title: title.into(),
            message,
            suggested_fix: Some(
                "Check your internet connection, or download the latest OpenWrap-Share.zip from the GitHub releases page."
                    .into(),
            ),
            details_safe: None,
        },
    }
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, CommandError> {
    let updater = app
        .updater()
        .map_err(|error| update_error("Update check failed", error))?;
    let update = updater
        .check()
        .await
        .map_err(|error| update_error("Update check failed", error))?;

    let info = update.as_ref().map(|update| UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
    });
    *pending.0.lock().unwrap_or_else(|e| e.into_inner()) = update;
    Ok(info)
}

/// Download, verify and install the pending update, then restart the app.
///
/// The restart goes through `ExitRequested`, so `AppState::shutdown()`
/// disconnects any active VPN session before the new version starts.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<(), CommandError> {
    let update = pending
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or_else(|| update_error("No update available", "Check for updates first."))?;

    let mut downloaded: u64 = 0;
    let progress_handle = app.clone();
    update
        .download_and_install(
            move |chunk_length, total| {
                downloaded += chunk_length as u64;
                let _ = progress_handle.emit(
                    UPDATE_DOWNLOAD_PROGRESS,
                    UpdateProgress { downloaded, total },
                );
            },
            || {},
        )
        .await
        .map_err(|error| update_error("Update failed", error))?;

    if let Some(state) = app.try_state::<AppState>() {
        state.shutdown();
    }
    app.request_restart();
    Ok(())
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    let bundled_helper = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join(BUNDLED_HELPER_RESOURCE))
        .filter(|path| path.is_file());

    let helper_status = if std::env::var_os("OPENWRAP_HELPER_PATH").is_some() {
        HelperStatus::Unknown
    } else {
        helper_status(bundled_helper.as_deref(), &installed_helper_path())
    };

    AppInfo {
        version: app.package_info().version.to_string(),
        helper_status,
        helper_install_command: bundled_helper.map(|path| helper_install_command(&path)),
    }
}

fn helper_status(bundled: Option<&Path>, installed: &Path) -> HelperStatus {
    let Some(bundled) = bundled else {
        return HelperStatus::Unknown;
    };
    if !installed.exists() {
        return HelperStatus::Missing;
    }
    match (file_digest(bundled), file_digest(installed)) {
        (Some(a), Some(b)) if a == b => HelperStatus::Current,
        (Some(_), Some(_)) => HelperStatus::Outdated,
        _ => HelperStatus::Unknown,
    }
}

fn file_digest(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path)
        .ok()
        .map(|bytes| Sha256::digest(bytes).to_vec())
}

/// Same install layout as `scripts/share/setup.sh`.
fn helper_install_command(bundled: &Path) -> String {
    let destination: PathBuf = installed_helper_path();
    format!(
        "sudo /usr/bin/install -o root -g wheel -m 4755 '{}' '{}'",
        bundled.display().to_string().replace('\'', r"'\''"),
        destination.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_status_compares_contents() {
        let dir = tempfile::tempdir().unwrap();
        let bundled = dir.path().join("bundled");
        let installed = dir.path().join("installed");
        std::fs::write(&bundled, b"v2").unwrap();

        assert!(matches!(helper_status(None, &installed), HelperStatus::Unknown));
        assert!(matches!(
            helper_status(Some(&bundled), &installed),
            HelperStatus::Missing
        ));

        std::fs::write(&installed, b"v1").unwrap();
        assert!(matches!(
            helper_status(Some(&bundled), &installed),
            HelperStatus::Outdated
        ));

        std::fs::write(&installed, b"v2").unwrap();
        assert!(matches!(
            helper_status(Some(&bundled), &installed),
            HelperStatus::Current
        ));
    }

    #[test]
    fn install_command_quotes_app_path() {
        let command = helper_install_command(Path::new("/Applications/My 'App'.app/x"));
        assert!(command.starts_with("sudo /usr/bin/install -o root -g wheel -m 4755 '"));
        assert!(command.contains(r"My '\''App'\''.app"));
    }
}
