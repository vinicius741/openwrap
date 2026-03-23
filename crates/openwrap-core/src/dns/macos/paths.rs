use std::path::{Path, PathBuf};

use crate::errors::AppError;
use crate::profiles::ProfileId;

pub fn persistent_state_dir(
    runtime_dir: &Path,
    profile_id: &ProfileId,
) -> Result<PathBuf, AppError> {
    let runtime_root = runtime_dir
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| {
            AppError::ConnectionState("runtime directory is missing an expected root".into())
        })?;
    Ok(runtime_root.join("dns-state").join(profile_id.to_string()))
}

pub fn bridge_dir(runtime_dir: &Path) -> PathBuf {
    std::env::temp_dir()
        .join("openwrap-dns")
        .join(runtime_dir.file_name().unwrap_or_default())
}

pub fn quote_openvpn_arg(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\ ")
}

pub fn shell_single_quote(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', r#"'\''"#);
    format!("'{escaped}'")
}

pub fn shell_single_quote_str(value: &str) -> String {
    let escaped = value.replace('\'', r#"'\''"#);
    format!("'{escaped}'")
}
