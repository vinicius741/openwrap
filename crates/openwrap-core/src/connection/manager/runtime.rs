use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::app_state::AppPaths;
use crate::config::{parse_profile, rewrite_profile, validate_openvpn_config_text};
use crate::errors::AppError;
use crate::openvpn::VpnBackendMode;
use crate::profiles::import::{blocked_findings, validate_directives};
use crate::profiles::{CredentialMode, CredentialStrategy, ProfileDetail, ProfileId};

const AUTH_FILE_NAME: &str = "auth.txt";
const LAUNCH_CONFIG_FILE_NAME: &str = "profile.ovpn";

pub fn prepare_runtime_dir(
    paths: &AppPaths,
    profile_id: &ProfileId,
    session_id: &crate::connection::SessionId,
) -> Result<PathBuf, AppError> {
    let profile_dir = paths.runtime_dir.join(profile_id.to_string());
    if profile_dir.exists() {
        #[cfg(not(windows))]
        fs::remove_dir_all(&profile_dir)?;

        #[cfg(windows)]
        // Best-effort cleanup of stale session directories. On Windows a
        // recently terminated OpenVPN process can briefly keep its previous
        // session directory locked as CWD, which must not block a new session.
        sweep_directory_entries(&profile_dir);
    }

    let runtime_dir = profile_dir.join(session_id.to_string());
    fs::create_dir_all(&runtime_dir)?;
    tighten_dir_permissions(&profile_dir)?;
    tighten_dir_permissions(&runtime_dir)?;
    Ok(runtime_dir)
}

#[cfg(any(windows, test))]
pub fn sweep_stale_runtime_dir(runtime_root: &Path) {
    if runtime_root.exists() {
        sweep_directory_entries(runtime_root);
    }
}

#[cfg(any(windows, test))]
fn sweep_directory_entries(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }
}

pub fn write_auth_file(
    runtime_dir: &Path,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Option<PathBuf>, AppError> {
    match (username, password) {
        (Some(username), Some(password)) => {
            let auth_path = runtime_dir.join(AUTH_FILE_NAME);
            let mut file = auth_file_options().open(&auth_path)?;
            file.write_all(format!("{username}\n{password}\n").as_bytes())?;
            Ok(Some(auth_path))
        }
        _ => Ok(None),
    }
}

pub fn write_launch_config(
    detail: &ProfileDetail,
    runtime_dir: &Path,
    backend_mode: VpnBackendMode,
) -> Result<(PathBuf, Vec<PathBuf>), AppError> {
    let source = fs::read_to_string(&detail.profile.managed_ovpn_path)?;
    let parsed = parse_profile(&source, &detail.profile.managed_dir)?;
    // The stored profile is a file on disk; check it again rather than trust
    // that it is still what the import accepted.
    if let Some(finding) = blocked_findings(&validate_directives(&parsed))
        .into_iter()
        .next()
    {
        return Err(AppError::Validation {
            title: "Profile contains a blocked directive".into(),
            message: format!(
                "{} Re-import the profile from its original file.",
                finding.message
            ),
            directive: Some(finding.directive),
            line: Some(finding.line),
        });
    }
    let rewritten_assets = match backend_mode {
        VpnBackendMode::ExternalProcess => detail
            .assets
            .iter()
            .map(|asset| {
                (
                    asset.kind.clone(),
                    quote_openvpn_arg(&detail.profile.managed_dir.join(&asset.relative_path)),
                )
            })
            .collect::<HashMap<_, _>>(),
        VpnBackendMode::PacketTunnel => {
            let assets_dir = runtime_dir.join("assets");
            fs::create_dir_all(&assets_dir)?;
            tighten_dir_permissions(&assets_dir)?;
            detail
                .assets
                .iter()
                .map(|asset| {
                    let source = detail.profile.managed_dir.join(&asset.relative_path);
                    // Use the kind-based canonical filename so shared basenames
                    // from future relative paths cannot silently overwrite.
                    let file_name = asset.kind.file_name();
                    let target = assets_dir.join(file_name);
                    fs::copy(source, &target)?;
                    tighten_file_permissions(&target)?;
                    Ok((
                        asset.kind.clone(),
                        quote_openvpn_arg(&Path::new("assets").join(file_name)),
                    ))
                })
                .collect::<Result<HashMap<_, _>, AppError>>()?
        }
    };
    let mut launch_config = rewrite_profile(&parsed, &rewritten_assets);
    if backend_mode == VpnBackendMode::ExternalProcess && uses_generated_password(detail) {
        // OpenVPN re-reads auth.txt on a soft restart (ping-restart after
        // sleep, network change), but the one-time password in it has expired
        // by then. Exit instead, so OpenWrap retries with a fresh password.
        launch_config.push_str("remap-usr1 SIGTERM\n");
    }
    #[cfg(target_os = "macos")]
    let extra_cleanup_paths = if backend_mode == VpnBackendMode::ExternalProcess {
        crate::dns::append_macos_launch_dns_config(
            &mut launch_config,
            runtime_dir,
            &detail.profile.id,
            &detail.profile.dns_policy,
            &detail.profile.dns_match_domains,
        )?
    } else {
        Vec::new()
    };

    #[cfg(windows)]
    if backend_mode == VpnBackendMode::ExternalProcess {
        crate::dns::windows::apply_launch_config(&mut launch_config, &detail.profile.dns_policy);
    }

    #[cfg(not(target_os = "macos"))]
    let extra_cleanup_paths = Vec::new();

    if let Err(error) = validate_openvpn_config_text(&launch_config) {
        for path in &extra_cleanup_paths {
            let _ = fs::remove_dir_all(path);
        }
        return Err(error);
    }
    let launch_config_path = runtime_dir.join(LAUNCH_CONFIG_FILE_NAME);
    fs::write(&launch_config_path, launch_config)?;
    Ok((launch_config_path, extra_cleanup_paths))
}

fn uses_generated_password(detail: &ProfileDetail) -> bool {
    detail.profile.credential_mode == CredentialMode::UserPass
        && detail.profile.credential_strategy == CredentialStrategy::PinTotp
}

pub fn quote_openvpn_arg(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
}

pub fn cleanup_runtime_artifacts(
    active_session: &crate::connection::manager::state::ActiveSession,
) {
    cleanup_auth_file(active_session);
    cleanup_runtime_bridge(active_session);
}

pub fn cleanup_auth_file(active_session: &crate::connection::manager::state::ActiveSession) {
    if let Some(auth_file) = &active_session.auth_file {
        let _ = fs::remove_file(auth_file);
    }
}

pub fn cleanup_runtime_bridge(active_session: &crate::connection::manager::state::ActiveSession) {
    for path in &active_session.extra_cleanup_paths {
        let _ = fs::remove_dir_all(path);
    }
    cleanup_runtime_dir(&active_session.runtime_dir);
}

pub fn cleanup_runtime_dir(runtime_dir: &Path) {
    let _ = fs::remove_dir_all(runtime_dir);
    if let Some(parent) = runtime_dir.parent() {
        if parent
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(parent);
        }
    }
}

#[cfg(unix)]
pub fn auth_file_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    options
}

#[cfg(not(unix))]
pub fn auth_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    options
}

#[cfg(unix)]
pub fn tighten_dir_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(unix)]
pub fn tighten_file_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
pub fn tighten_file_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(unix))]
pub fn tighten_dir_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::sweep_stale_runtime_dir;

    #[test]
    fn sweep_stale_runtime_dir_removes_leftover_session_auth_files() {
        let temp = tempdir().unwrap();
        let runtime_root = temp.path().join("runtime");
        let stale_session = runtime_root.join("profile-1").join("session-1");
        fs::create_dir_all(&stale_session).unwrap();
        fs::write(stale_session.join("auth.txt"), "user\npass\n").unwrap();
        fs::write(stale_session.join("profile.ovpn"), "client\n").unwrap();

        sweep_stale_runtime_dir(&runtime_root);

        assert!(runtime_root.exists());
        assert_eq!(fs::read_dir(&runtime_root).unwrap().count(), 0);
    }
}
