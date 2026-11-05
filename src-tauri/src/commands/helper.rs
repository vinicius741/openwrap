//! Install only the helper bundled with this app, using macOS authorization.
//! The administrator password stays in the system dialog and never enters IPC.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use openwrap_core::errors::UserFacingError;

use crate::error::CommandError;

use super::updates::AppInfo;
#[cfg(target_os = "macos")]
use super::updates::{get_app_info, HelperStatus};

const BUNDLED_HELPER_RESOURCE: &str = "openwrap-helper-bundled";

#[cfg(target_os = "macos")]
const AUTHORIZE_INSTALL: &str = r#"
on run argv
    try
        do shell script (item 1 of argv) with administrator privileges with prompt "OpenWrap needs to install or update its VPN helper."
        return "installed"
    on error number -128
        return "cancelled"
    end try
end run
"#;

pub(super) fn bundled_helper_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join(BUNDLED_HELPER_RESOURCE))
        .filter(|path| path.is_file())
}

fn install_error(message: impl std::fmt::Display) -> CommandError {
    let message = message.to_string();
    CommandError {
        message: message.clone(),
        user_facing: UserFacingError {
            code: "helper_install_failed".into(),
            title: "Helper installation failed".into(),
            message,
            suggested_fix: Some(
                "Try again and authorize the installation in the macOS dialog.".into(),
            ),
            details_safe: None,
        },
    }
}

/// No paths or shell text are accepted from the frontend. `None` means the
/// user cancelled the system authorization dialog; no error needs to be shown.
#[tauri::command]
pub async fn install_helper(app: AppHandle) -> Result<Option<AppInfo>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || install_bundled_helper(&app))
        .await
        .map_err(install_error)?
}

#[cfg(target_os = "macos")]
fn install_bundled_helper(app: &AppHandle) -> Result<Option<AppInfo>, CommandError> {
    use std::process::Command;
    use std::sync::Mutex;

    // Keep concurrent IPC requests from opening multiple authorization dialogs.
    static INSTALL_LOCK: Mutex<()> = Mutex::new(());
    let _guard = INSTALL_LOCK
        .try_lock()
        .map_err(|_| install_error("A helper installation is already in progress."))?;

    if std::env::var_os("OPENWRAP_HELPER_PATH").is_some() {
        return Err(install_error(
            "A custom helper path is configured. Update that helper manually.",
        ));
    }

    let bundled = bundled_helper_path(app).ok_or_else(|| {
        install_error("This build does not include a VPN helper. Install a packaged OpenWrap app.")
    })?;
    let info = get_app_info(app.clone());
    if matches!(info.helper_status, HelperStatus::Current) {
        return Ok(Some(info));
    }

    let command = helper_install_command(&bundled)?;
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(AUTHORIZE_INSTALL)
        .arg("--")
        .arg(command)
        .output()
        .map_err(install_error)?;

    if !output.status.success() {
        return Err(install_error(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "cancelled" => return Ok(None),
        "installed" => {}
        _ => {
            return Err(install_error(
                "macOS did not confirm the helper installation.",
            ))
        }
    }

    // Check both the bytes and root/setuid permissions before reporting success.
    let info = get_app_info(app.clone());
    if !matches!(info.helper_status, HelperStatus::Current) {
        return Err(install_error(
            "The installed helper could not be verified. Try installing it again.",
        ));
    }
    Ok(Some(info))
}

#[cfg(not(target_os = "macos"))]
fn install_bundled_helper(_app: &AppHandle) -> Result<Option<AppInfo>, CommandError> {
    Err(install_error(
        "Helper installation is only available on macOS.",
    ))
}

#[cfg(any(target_os = "macos", test))]
fn helper_install_command(bundled: &std::path::Path) -> Result<String, CommandError> {
    use crate::app_state::backend_factory::INSTALLED_HELPER_PATH;

    let source = bundled.to_str().ok_or_else(|| {
        install_error(
            "The app path is not valid UTF-8. Move OpenWrap to Applications and try again.",
        )
    })?;
    // AppleScript source is constant; the shell command is passed as an argv
    // value. Quote paths for the shell independently of AppleScript syntax.
    let quoted_source = format!("'{}'", source.replace('\'', "'\\''"));
    Ok(format!(
        "/usr/bin/install -d -o root -g wheel -m 755 /Library/PrivilegedHelperTools && /usr/bin/install -S -o root -g wheel -m 4755 {quoted_source} '{INSTALLED_HELPER_PATH}'"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_quotes_paths_without_executing_shell_text() {
        // Run the generated command with an unprivileged stand-in for install,
        // so this tests the shell's actual argument parsing without elevation.
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("injected");
        let source = format!("/Applications/My 'App' \" $(touch {}) `false` \\\n.app/Contents/Resources/openwrap-helper-bundled", marker.display());
        let command = helper_install_command(std::path::Path::new(&source)).unwrap();
        let command = command.replace("/usr/bin/install", "capture_install");
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "capture_install() {{ for arg do printf '%s\\000' \"$arg\"; done; }}; {command}"
            ))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!marker.exists());
        let args: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
        assert!(args.contains(&source.as_bytes()));
        assert!(args.contains(&b"4755".as_slice()));
        assert!(args.contains(&crate::app_state::backend_factory::INSTALLED_HELPER_PATH.as_bytes()));
    }
}
