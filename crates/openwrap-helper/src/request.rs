use std::io::{self, Read};
use std::path::Path;

use openwrap_core::openvpn::ConnectRequest;

use crate::system::{
    openwrap_base_dir, real_user_home_dir, validate_openvpn_binary, validate_scoped_path,
};

pub fn read_request() -> Result<ConnectRequest, String> {
    read_json_request()
}

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

pub fn validate_config_path(
    path: &Path,
    profiles_dir: &Path,
    runtime_root: &Path,
) -> Result<(), String> {
    validate_scoped_path("config", path, runtime_root)
        .or_else(|_| validate_scoped_path("config", path, profiles_dir))
}

pub fn validate_runtime_root(path: &Path) -> Result<(), String> {
    let home_dir = real_user_home_dir()?;
    let expected_root = openwrap_base_dir(&home_dir).join("runtime");
    validate_scoped_path("runtime root", path, &expected_root)
}
