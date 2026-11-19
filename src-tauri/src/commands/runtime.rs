//! Launch dependency installation in a visible system window so administrator
//! credentials never pass through IPC: Terminal + Homebrew on macOS, a winget
//! console on Windows (the app already runs elevated there).
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::app_state::AppState;
use crate::error::CommandError;

#[cfg(any(target_os = "macos", all(test, unix)))]
const INSTALL_SCRIPT: &str = include_str!("../../../scripts/share/install-openvpn.sh");
#[cfg(target_os = "macos")]
const REPAIR_SCRIPT: &str = include_str!("../../../scripts/repair-openvpn.sh");
/// Where Homebrew links OpenVPN; only these installs can be repaired with it.
const HOMEBREW_PREFIXES: [&str; 2] = ["/opt/homebrew", "/usr/local"];
#[cfg(target_os = "macos")]
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
    /// Version reported by the selected binary, when it starts.
    pub openvpn_version: Option<String>,
    /// Why the selected binary cannot start, when it is installed but broken
    /// (typically after a Homebrew update).
    pub openvpn_problem: Option<String>,
    /// The selected binary comes from Homebrew, so it can be updated in place.
    pub repair_available: bool,
}

fn setup_error(message: impl Into<String>) -> CommandError {
    let message = message.into();
    CommandError {
        message: message.clone(),
        user_facing: openwrap_core::errors::UserFacingError {
            code: "runtime_install_failed".into(),
            title: "OpenVPN setup did not finish".into(),
            message,
            suggested_fix: Some(
                if cfg!(windows) {
                    "Run \"winget install --id OpenVPNTechnologies.OpenVPN --exact\" in an elevated Command Prompt, or install OpenVPN Community manually from https://openvpn.net/community-downloads/ and restart OpenWrap."
                } else {
                    "Check the output in Terminal, then try again. Keep the Terminal window open until setup finishes."
                }
                .into(),
            ),
            details_safe: None,
        },
    }
}

#[tauri::command]
pub async fn get_runtime_dependencies(
    state: tauri::State<'_, AppState>,
) -> Result<RuntimeDependencies, CommandError> {
    let override_path = state
        .profile_repository()
        .get_settings()?
        .openvpn_path_override;
    // Detection starts OpenVPN to read its version, so keep it off the main thread.
    tauri::async_runtime::spawn_blocking(move || detect(override_path))
        .await
        .map_err(|error| setup_error(error.to_string()))
}

fn detect(override_path: Option<PathBuf>) -> RuntimeDependencies {
    let openvpn_path = openwrap_core::detect_openvpn_binaries(override_path).selected_path;
    let probe = openvpn_path.as_deref().map(openwrap_core::probe_openvpn);
    RuntimeDependencies {
        repair_available: openvpn_path.as_deref().is_some_and(is_homebrew_path),
        openvpn_version: probe.as_ref().and_then(|result| result.clone().ok()),
        openvpn_problem: probe.and_then(Result::err),
        openvpn_path,
    }
}

fn is_homebrew_path(path: &Path) -> bool {
    HOMEBREW_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
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

/// Updates Homebrew's OpenVPN (reinstalling it if it still cannot start) in
/// Terminal. Used when OpenVPN is installed but broken or out of date.
#[tauri::command]
pub async fn repair_runtime_dependencies(
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
        if !detect(override_path.clone()).repair_available {
            return Err(setup_error(
                "OpenWrap can only repair an OpenVPN that was installed with Homebrew.",
            ));
        }
        run_repair()?;
        let repaired = detect(override_path);
        if repaired.openvpn_version.is_none() {
            return Err(setup_error(
                "The update finished, but OpenVPN still does not start.",
            ));
        }
        Ok(repaired)
    })
    .await
    .map_err(|error| setup_error(error.to_string()))?
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(any(target_os = "macos", all(test, unix)))]
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

/// CREATE_NEW_CONSOLE: show setup progress in its own window. The child
/// inherits OpenWrap's elevated token, so it needs no extra prompt.
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Prefers the bundled `install-openvpn.ps1` (pinned, checksum-verified MSI,
/// also used by the NSIS installer), which works without winget. It sits next
/// to `openwrap-app.exe` in the admin-only install directory; builds without
/// the script fall back to winget.
#[cfg(windows)]
fn run_installer(override_path: Option<&Path>) -> Result<(), CommandError> {
    use std::os::windows::process::CommandExt;

    let script = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("install-openvpn.ps1")))
        .filter(|path| path.is_file());
    let Some(script) = script else {
        return run_winget_installer(override_path);
    };
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let powershell = system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let status = std::process::Command::new(powershell)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-PauseOnError")
        .creation_flags(CREATE_NEW_CONSOLE)
        .status()
        .map_err(|error| setup_error(format!("Could not start PowerShell: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(setup_error(
            "The OpenVPN installer failed or was cancelled.",
        ))
    }
}

#[cfg(windows)]
fn run_winget_installer(_override_path: Option<&Path>) -> Result<(), CommandError> {
    use std::os::windows::process::CommandExt;

    let winget = resolve_winget_exe()
        .ok_or_else(|| setup_error("winget (App Installer) is not available on this PC."))?;

    let status = std::process::Command::new(winget)
        .args([
            "install",
            "--id",
            "OpenVPNTechnologies.OpenVPN",
            "--exact",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .creation_flags(CREATE_NEW_CONSOLE)
        .status()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                setup_error("winget (App Installer) is not available on this PC.")
            }
            _ => setup_error(error.to_string()),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(setup_error(
            "The OpenVPN installer failed or was cancelled.",
        ))
    }
}

#[cfg(windows)]
fn resolve_winget_exe() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let mut dirs = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("WindowsApps"),
        );
    }
    if let Some(path_env) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_env));
    }
    resolve_winget_in_dirs(dirs, exe_dir.as_deref())
}

#[cfg(any(windows, test))]
fn resolve_winget_in_dirs(
    dirs: impl IntoIterator<Item = PathBuf>,
    excluded_dir: Option<&Path>,
) -> Option<PathBuf> {
    let excluded_norm = excluded_dir.map(normalize_dir_for_compare);
    for dir in dirs {
        if !dir.is_absolute() {
            continue;
        }
        if let Some(excluded) = &excluded_norm {
            if normalize_dir_for_compare(&dir) == *excluded {
                continue;
            }
        }
        let candidate = dir.join("winget.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(any(windows, test))]
fn normalize_dir_for_compare(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn run_installer(_override_path: Option<&Path>) -> Result<(), CommandError> {
    Err(setup_error(
        "Automatic installation is available on macOS and Windows.",
    ))
}

#[cfg(target_os = "macos")]
fn run_installer(override_path: Option<&Path>) -> Result<(), CommandError> {
    run_in_terminal(INSTALL_SCRIPT, override_path)
}

#[cfg(target_os = "macos")]
fn run_repair() -> Result<(), CommandError> {
    run_in_terminal(REPAIR_SCRIPT, None)
}

#[cfg(not(target_os = "macos"))]
fn run_repair() -> Result<(), CommandError> {
    Err(setup_error(
        "OpenWrap can only repair an OpenVPN that was installed with Homebrew.",
    ))
}

/// Runs `script` in a Terminal window, passing `argument` as its first
/// argument, and waits for it to finish.
#[cfg(target_os = "macos")]
fn run_in_terminal(script: &str, argument: Option<&Path>) -> Result<(), CommandError> {
    // A private directory prevents another user from replacing the script or result.
    let dir = tempfile::Builder::new()
        .prefix("openwrap-setup-")
        .tempdir()
        .map_err(|error| setup_error(error.to_string()))?;
    let script_path = dir.path().join("install.sh");
    let result_path = dir.path().join("result");
    let wrapper = installer_wrapper(script, &result_path);
    std::fs::write(&script_path, wrapper).map_err(|error| setup_error(error.to_string()))?;
    let command = format!(
        "/bin/bash {} {}",
        shell_quote(&script_path.to_string_lossy()),
        shell_quote(
            &argument
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

// These tests run the macOS installer wrapper through /bin/bash.
#[cfg(all(test, unix))]
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
    fn only_homebrew_installs_can_be_repaired() {
        assert!(is_homebrew_path(Path::new("/opt/homebrew/sbin/openvpn")));
        assert!(is_homebrew_path(Path::new("/usr/local/sbin/openvpn")));
        assert!(!is_homebrew_path(Path::new("/usr/bin/openvpn")));
        assert!(!is_homebrew_path(Path::new("/Users/me/bin/openvpn")));
    }

    #[cfg(unix)]
    #[test]
    fn detection_reports_a_binary_that_exists_but_cannot_start() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let binary_path = dir.path().join("openvpn");
        std::fs::write(
            &binary_path,
            "#!/bin/sh\necho 'dyld: Library not loaded: liblz4.1.dylib' >&2\nexit 6\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let broken = detect(Some(binary_path.clone()));
        assert_eq!(broken.openvpn_path.as_deref(), Some(binary_path.as_path()));
        assert_eq!(broken.openvpn_version, None);
        assert!(broken
            .openvpn_problem
            .unwrap()
            .contains("Library not loaded"));
        assert!(!broken.repair_available);

        std::fs::write(&binary_path, "#!/bin/sh\necho 'OpenVPN 2.7.9 test'\n").unwrap();
        let healthy = detect(Some(binary_path));
        assert_eq!(healthy.openvpn_version.as_deref(), Some("2.7.9"));
        assert_eq!(healthy.openvpn_problem, None);
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

#[cfg(test)]
mod winget_resolution_tests {
    use std::path::PathBuf;

    use super::resolve_winget_in_dirs;

    #[test]
    fn skips_relative_and_application_directories() {
        let temp = tempfile::tempdir().unwrap();
        let app_dir = temp.path().join("app");
        let system_dir = temp.path().join("windows-apps");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::create_dir_all(&system_dir).unwrap();

        std::fs::write(app_dir.join("winget.exe"), b"planted").unwrap();
        let expected = system_dir.join("winget.exe");
        std::fs::write(&expected, b"real").unwrap();

        let resolved = resolve_winget_in_dirs(
            [PathBuf::from("."), app_dir.clone(), system_dir],
            Some(&app_dir),
        );
        assert_eq!(resolved, Some(expected));
    }
}
