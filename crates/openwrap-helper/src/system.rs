use std::ffi::CStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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

    let home = unsafe { CStr::from_ptr(passwd.pw_dir) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(home.to_bytes())))
}

pub fn openwrap_base_dir(home_dir: &Path) -> PathBuf {
    home_dir.join("Library/Application Support/OpenWrap")
}

pub fn flush_dns_cache() {
    let _ = std::process::Command::new("/usr/bin/dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = std::process::Command::new("/usr/bin/killall")
        .arg("-HUP")
        .arg("mDNSResponder")
        .status();
}

pub fn restore_service_dns(service: &str, desired_dns: &[String]) -> Result<(), String> {
    let mut command = std::process::Command::new("/usr/sbin/networksetup");
    command.arg("-setdnsservers").arg(service);

    if desired_dns.is_empty() {
        command.arg("Empty");
    } else {
        for server in desired_dns {
            command.arg(server);
        }
    }

    let status = command
        .status()
        .map_err(|error| format!("failed to invoke networksetup: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("networksetup exited with status {status}"))
    }
}

pub fn verify_service_dns(service: &str, expected_dns: &str) -> Result<(), String> {
    let output = std::process::Command::new("/usr/sbin/networksetup")
        .arg("-getdnsservers")
        .arg(service)
        .output()
        .map_err(|error| format!("failed to invoke networksetup: {error}"))?;

    if !output.status.success() {
        return Err(format!("networksetup exited with status {}", output.status));
    }

    let actual_dns = normalize_networksetup_dns_output(&String::from_utf8_lossy(&output.stdout));
    if actual_dns == expected_dns {
        Ok(())
    } else {
        Err(format!("expected {expected_dns:?}, got {actual_dns:?}"))
    }
}

pub fn delete_dns_route(dns_server: &str, gateway: Option<&str>) -> bool {
    let mut command = std::process::Command::new("/sbin/route");
    command.arg("-n").arg("delete").arg("-host").arg(dns_server);
    if let Some(gateway) = gateway.filter(|gateway| !gateway.is_empty()) {
        command.arg(gateway);
    }

    command.status().is_ok_and(|status| status.success())
}

pub fn normalize_networksetup_dns_output(output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "__EMPTY__".into();
    }

    if trimmed.contains("There aren't any DNS Servers set on") {
        return "__EMPTY__".into();
    }

    let joined = trimmed
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.is_empty() {
        "__EMPTY__".into()
    } else {
        joined
    }
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

pub fn dir_is_empty(path: &Path) -> Result<bool, String> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to inspect DNS state directory {}: {error}",
            path.display()
        )
    })?;
    Ok(entries.next().is_none())
}

pub fn terminate_pid(pid: Option<u32>) {
    if let Some(pid) = pid {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}
