use std::path::{Path, PathBuf};

use crate::system::{kill_process, run_ps};

pub fn reconcile_runtime_processes(runtime_root: &Path) -> Result<(), String> {
    let output = run_ps()
        .map_err(|error| format!("failed to inspect running OpenWrap processes: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to inspect running OpenWrap processes: ps exited with status {}",
            output.status
        ));
    }

    let mut orphan_openvpn = Vec::new();
    let mut helper_parents = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((pid, ppid, command)) = parse_ps_line(line) else {
            continue;
        };

        if let Some(config_path) = extract_managed_openvpn_config(&command, runtime_root) {
            if !config_path.exists() {
                orphan_openvpn.push(pid);
                helper_parents.push(ppid);
            }
        }
    }

    let mut errors = Vec::new();
    for pid in orphan_openvpn {
        if let Err(error) = kill_process(pid) {
            if error.raw_os_error() != Some(libc::ESRCH) {
                errors.push(format!(
                    "failed to terminate orphaned openvpn process {pid}: {error}"
                ));
            }
        }
    }

    helper_parents.sort_unstable();
    helper_parents.dedup();
    for pid in helper_parents {
        if pid <= 1 {
            continue;
        }
        if let Err(error) = kill_process(pid) {
            if error.raw_os_error() != Some(libc::ESRCH) {
                errors.push(format!(
                    "failed to terminate orphaned openwrap-helper process {pid}: {error}"
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub fn parse_ps_line(line: &str) -> Option<(i32, i32, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let pid_end = trimmed.find(char::is_whitespace)?;
    let pid = trimmed[..pid_end].parse::<i32>().ok()?;

    let rest = trimmed[pid_end..].trim_start();
    let ppid_end = rest.find(char::is_whitespace)?;
    let ppid = rest[..ppid_end].parse::<i32>().ok()?;

    let command = rest[ppid_end..].trim_start();
    if command.is_empty() {
        return None;
    }

    Some((pid, ppid, command.to_string()))
}

pub fn extract_managed_openvpn_config(command: &str, runtime_root: &Path) -> Option<PathBuf> {
    if !command.contains("openvpn") {
        return None;
    }

    let (_, remainder) = command.split_once("--config ")?;
    let end = remainder.find(" --auth-nocache").unwrap_or(remainder.len());
    let config_path = PathBuf::from(remainder[..end].trim());
    if config_path.starts_with(runtime_root) {
        Some(config_path)
    } else {
        None
    }
}
