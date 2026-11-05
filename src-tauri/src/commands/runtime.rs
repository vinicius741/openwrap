//! Launch dependency installation in Terminal so sudo never passes through IPC.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::app_state::AppState;
use crate::error::CommandError;

const INSTALL_SCRIPT: &str = include_str!("../../../scripts/share/install-openvpn.sh");
const TERMINAL_SCRIPT: &str = r#"
on run argv
    tell application "Terminal"
        activate
        set setupTab to do script (item 1 of argv)
        delay 1
        repeat while busy of setupTab
            delay 1
        end repeat
    end tell
end run
"#;

#[derive(Default)]
pub struct RuntimeInstaller(Arc<Mutex<()>>);

#[derive(Debug, Serialize)]
pub struct RuntimeDependencies {
    pub openvpn_path: Option<PathBuf>,
}

fn setup_error(message: impl Into<String>) -> CommandError {
    let message = message.into();
    CommandError {
        message: message.clone(),
        user_facing: openwrap_core::errors::UserFacingError {
            code: "runtime_install_failed".into(),
            title: "OpenVPN setup did not finish".into(),
            message,
            suggested_fix: Some("Check the output in Terminal, then try again. Keep the Terminal window open until setup finishes.".into()),
            details_safe: None,
        },
    }
}

#[tauri::command]
pub fn get_runtime_dependencies(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeDependencies, CommandError> {
    let settings = state.profile_repository().get_settings()?;
    Ok(detect(settings.openvpn_path_override))
}

fn detect(override_path: Option<PathBuf>) -> RuntimeDependencies {
    RuntimeDependencies {
        openvpn_path: openwrap_core::detect_openvpn_binaries(override_path).selected_path,
    }
}

#[tauri::command]
pub async fn install_runtime_dependencies(
    state: tauri::State<'_, AppState>,
    installer: tauri::State<'_, RuntimeInstaller>,
) -> Result<RuntimeDependencies, CommandError> {
    let override_path = state
        .profile_repository()
        .get_settings()?
        .openvpn_path_override;
    let lock = installer.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock
            .try_lock()
            .map_err(|_| setup_error("Setup is already running."))?;
        let current = detect(override_path.clone());
        if current.openvpn_path.is_some() {
            return Ok(current);
        }
        run_installer(override_path.as_deref())?;
        let installed = detect(override_path);
        if installed.openvpn_path.is_none() {
            return Err(setup_error(
                "Setup finished, but OpenWrap still cannot find OpenVPN.",
            ));
        }
        Ok(installed)
    })
    .await
    .map_err(|error| setup_error(error.to_string()))?
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn installer_wrapper(script: &str, result_path: &Path) -> String {
    let on_exit = format!(
        "result=$?; printf '%s\\n' \"$result\" > {}; if [ \"$result\" -ne 0 ]; then echo 'OpenVPN setup failed. Review the output above, then retry in OpenWrap.'; fi",
        shell_quote(&result_path.to_string_lossy()),
    );
    format!(
        "trap {} EXIT\ntrap 'exit 130' HUP INT TERM\n{}",
        shell_quote(&on_exit),
        script.strip_prefix("#!/bin/bash\n").unwrap_or(script),
    )
}

fn run_installer(override_path: Option<&Path>) -> Result<(), CommandError> {
    if !cfg!(target_os = "macos") {
        return Err(setup_error("Automatic installation is available on macOS."));
    }
    // A private directory prevents another user from replacing the script or result.
    let dir = tempfile::Builder::new()
        .prefix("openwrap-setup-")
        .tempdir()
        .map_err(|error| setup_error(error.to_string()))?;
    let script_path = dir.path().join("install.sh");
    let result_path = dir.path().join("result");
    let wrapper = installer_wrapper(INSTALL_SCRIPT, &result_path);
    std::fs::write(&script_path, wrapper).map_err(|error| setup_error(error.to_string()))?;
    let command = format!(
        "/bin/bash {} {}",
        shell_quote(&script_path.to_string_lossy()),
        shell_quote(
            &override_path
                .map(|path| path.to_string_lossy())
                .unwrap_or_default()
        )
    );
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", TERMINAL_SCRIPT, "--", &command])
        .output()
        .map_err(|error| setup_error(error.to_string()))?;
    // Prefer the script's result even if Terminal closed immediately afterward.
    match std::fs::read_to_string(&result_path).ok().as_deref().map(str::trim) {
        Some("0") => Ok(()),
        Some(_) => Err(setup_error("The installer failed or was cancelled.")),
        None if !output.status.success() => Err(setup_error(
            "Terminal could not run setup. Allow OpenWrap to control Terminal in macOS System Settings > Privacy & Security > Automation, then try again.")),
        None => Err(setup_error("The installer closed before reporting a result.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arguments_preserve_special_characters() {
        let value = "/tmp/My 'App' $(echo unsafe) `echo unsafe`/install.sh";
        let output = std::process::Command::new("/bin/bash")
            .args(["-c", &format!("printf '%s' {}", shell_quote(value))])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
    }

    #[test]
    fn installer_records_success_failure_and_interruption() {
        let dir = tempfile::tempdir().unwrap();
        let result_path = dir.path().join("result 'with' $special `characters`");
        for (script, expected) in [
            ("exit 0", "0"),
            ("set -e\nfalse", "1"),
            ("kill -TERM $$", "130"),
        ] {
            let output = std::process::Command::new("/bin/bash")
                .args(["-c", &installer_wrapper(script, &result_path)])
                .output()
                .unwrap();
            assert_eq!(output.status.code().unwrap().to_string(), expected);
            assert_eq!(
                std::fs::read_to_string(&result_path).unwrap().trim(),
                expected
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn embedded_installer_reuses_custom_openvpn_and_reports_completion() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let binary_path = dir.path().join("My 'OpenVPN'");
        std::fs::write(&binary_path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let result_path = dir.path().join("result");
        let script_path = dir.path().join("install.sh");
        std::fs::write(
            &script_path,
            installer_wrapper(INSTALL_SCRIPT, &result_path),
        )
        .unwrap();

        let output = std::process::Command::new("/bin/bash")
            .arg(script_path)
            .arg(&binary_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read_to_string(result_path).unwrap().trim(), "0");
        assert!(String::from_utf8_lossy(&output.stdout).contains("OpenVPN is ready"));
    }
}
