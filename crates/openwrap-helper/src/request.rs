use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub use openwrap_core::openvpn::ConnectRequest;
pub use openwrap_core::openvpn::ReconcileDnsRequest;

pub fn read_json_request<T>() -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| format!("failed to read request: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid request payload: {error}"))
}

pub fn validate_request(request: &ConnectRequest) -> Result<(), String> {
    let home_dir = real_user_home_dir()?;
    let base_dir = openwrap_base_dir(&home_dir);
    let profiles_dir = base_dir.join("profiles");
    let runtime_root = base_dir.join("runtime");

    validate_config_path(&request.config_path, &profiles_dir, &runtime_root)?;
    validate_scoped_path("runtime", &request.runtime_dir, &runtime_root)?;
    if let Some(auth_file) = &request.auth_file {
        validate_scoped_path("auth file", auth_file, &request.runtime_dir)?;
    }
    validate_openvpn_binary(&request.openvpn_binary)?;
    Ok(())
}

pub fn validate_runtime_root(path: &Path) -> Result<(), String> {
    let home_dir = real_user_home_dir()?;
    let expected_root = openwrap_base_dir(&home_dir).join("runtime");
    validate_scoped_path("runtime root", path, &expected_root)
}

pub fn validate_config_path(
    path: &Path,
    profiles_dir: &Path,
    runtime_root: &Path,
) -> Result<(), String> {
    validate_scoped_path("config", path, runtime_root)
        .or_else(|_| validate_scoped_path("config", path, profiles_dir))
}

pub fn validate_scoped_path(label: &str, path: &Path, root: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} path must be absolute"));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve allowed {label} root: {error}"))?;
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} path: {error}"))?;
    if canonical_path.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(format!(
            "{label} path escapes the OpenWrap managed directory: {}",
            path.display()
        ))
    }
}

pub fn validate_openvpn_binary(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("openvpn binary path must be absolute".into());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve openvpn binary: {error}"))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect openvpn binary: {error}"))?;
    let mode = metadata.permissions().mode();
    if metadata.is_file() && (mode & 0o111) != 0 {
        Ok(())
    } else {
        Err(format!(
            "openvpn binary is not executable: {}",
            canonical.display()
        ))
    }
}

pub fn real_user_home_dir() -> Result<PathBuf, String> {
    let uid = unsafe { libc::getuid() };
    let mut passwd = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 4096];

    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &mut passwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };

    if status != 0 || result.is_null() {
        return Err("failed to resolve the invoking user's home directory".into());
    }

    let home = unsafe { std::ffi::CStr::from_ptr(passwd.pw_dir) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(home.to_bytes())))
}

pub fn openwrap_base_dir(home_dir: &Path) -> PathBuf {
    home_dir.join("Library/Application Support/OpenWrap")
}
