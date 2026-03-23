use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::app_state::AppPaths;
use crate::config::{parse_profile, rewrite_profile};
use crate::errors::AppError;
use crate::profiles::{ProfileDetail, ProfileId};

const AUTH_FILE_NAME: &str = "auth.txt";
const LAUNCH_CONFIG_FILE_NAME: &str = "profile.ovpn";

pub fn prepare_runtime_dir(
    paths: &AppPaths,
    profile_id: &ProfileId,
    session_id: &crate::connection::SessionId,
) -> Result<PathBuf, AppError> {
    let profile_dir = paths.runtime_dir.join(profile_id.to_string());
    if profile_dir.exists() {
        fs::remove_dir_all(&profile_dir)?;
    }

    let runtime_dir = profile_dir.join(session_id.to_string());
    fs::create_dir_all(&runtime_dir)?;
    tighten_dir_permissions(&profile_dir)?;
    tighten_dir_permissions(&runtime_dir)?;
    Ok(runtime_dir)
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
) -> Result<(PathBuf, Vec<PathBuf>), AppError> {
    let source = fs::read_to_string(&detail.profile.managed_ovpn_path)?;
    let parsed = parse_profile(&source, &detail.profile.managed_dir)?;
    let rewritten_assets = detail
        .assets
        .iter()
        .map(|asset| {
            (
                asset.kind.clone(),
                quote_openvpn_arg(&detail.profile.managed_dir.join(&asset.relative_path)),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut launch_config = rewrite_profile(&parsed, &rewritten_assets);
    #[cfg(target_os = "macos")]
    let extra_cleanup_paths = crate::dns::append_macos_launch_dns_config(
        &mut launch_config,
        runtime_dir,
        &detail.profile.id,
        &detail.profile.dns_policy,
    )?;

    #[cfg(not(target_os = "macos"))]
    let extra_cleanup_paths = Vec::new();

    let launch_config_path = runtime_dir.join(LAUNCH_CONFIG_FILE_NAME);
    fs::write(&launch_config_path, launch_config)?;
    Ok((launch_config_path, extra_cleanup_paths))
}

pub fn quote_openvpn_arg(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
}

pub fn cleanup_runtime_artifacts(active_session: &crate::connection::manager::state::ActiveSession) {
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

#[cfg(not(unix))]
pub fn tighten_dir_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}
