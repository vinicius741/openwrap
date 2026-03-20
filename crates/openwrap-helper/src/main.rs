use std::ffi::CStr;
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use openwrap_core::openvpn::config_working_dir;
use openwrap_core::openvpn::helper_protocol::HelperEvent;
use openwrap_core::openvpn::{ConnectRequest, ReconcileDnsRequest};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let exit_code = match std::env::args().nth(1).as_deref() {
        Some("connect") => run_connect().await,
        Some("reconcile-dns") => run_reconcile_dns(),
        _ => {
            eprintln!("usage: openwrap-helper connect|reconcile-dns");
            64
        }
    };
    std::process::exit(exit_code);
}

async fn run_connect() -> i32 {
    let request = match read_request() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return 64;
        }
    };

    if let Err(error) = validate_request(&request) {
        eprintln!("{error}");
        return 78;
    }

    let mut command = Command::new(&request.openvpn_binary);
    let working_dir = match config_working_dir(&request.config_path) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 78;
        }
    };
    command.arg("--config").arg(&request.config_path);
    command.arg("--auth-nocache");
    command.arg("--verb").arg("3");
    command.env_clear();
    command.current_dir(working_dir);

    if let Some(auth_file) = &request.auth_file {
        command.arg("--auth-user-pass").arg(auth_file);
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("failed to launch openvpn: {error}");
            return 70;
        }
    };

    let openvpn_pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let writer = Arc::new(Mutex::new(BufWriter::new(tokio::io::stdout())));

    if let Err(error) = emit_event(&writer, &HelperEvent::Started { pid: openvpn_pid }).await {
        eprintln!("failed to emit helper startup event: {error}");
        return 70;
    }

    let stdout_task = stdout.map(|stdout| {
        let writer = writer.clone();
        tokio::spawn(async move { pipe_lines(stdout, writer, true).await })
    });
    let stderr_task = stderr.map(|stderr| {
        let writer = writer.clone();
        tokio::spawn(async move { pipe_lines(stderr, writer, false).await })
    });

    let mut sigterm = signal(SignalKind::terminate()).ok();
    let mut sigint = signal(SignalKind::interrupt()).ok();
    let status = tokio::select! {
        status = child.wait() => status.ok(),
        _ = recv_signal(&mut sigterm, &mut sigint) => {
            terminate_pid(openvpn_pid);
            child.wait().await.ok()
        }
    };

    if let Some(task) = stdout_task {
        let _ = task.await;
    }
    if let Some(task) = stderr_task {
        let _ = task.await;
    }

    status.and_then(|status| status.code()).unwrap_or(1)
}

fn read_request() -> Result<ConnectRequest, String> {
    read_json_request()
}

fn read_json_request<T>() -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| format!("failed to read request: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid request payload: {error}"))
}

fn validate_request(request: &ConnectRequest) -> Result<(), String> {
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

fn run_reconcile_dns() -> i32 {
    let request = match read_json_request::<ReconcileDnsRequest>() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return 64;
        }
    };

    if let Err(error) = validate_runtime_root(&request.runtime_root) {
        eprintln!("{error}");
        return 78;
    }

    if let Err(error) = reconcile_dns_state(&request.runtime_root) {
        eprintln!("{error}");
        return 70;
    }

    0
}

fn validate_config_path(
    path: &Path,
    profiles_dir: &Path,
    runtime_root: &Path,
) -> Result<(), String> {
    validate_scoped_path("config", path, runtime_root)
        .or_else(|_| validate_scoped_path("config", path, profiles_dir))
}

fn validate_runtime_root(path: &Path) -> Result<(), String> {
    let home_dir = real_user_home_dir()?;
    let expected_root = openwrap_base_dir(&home_dir).join("runtime");
    validate_scoped_path("runtime root", path, &expected_root)
}

fn validate_scoped_path(label: &str, path: &Path, root: &Path) -> Result<(), String> {
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

fn validate_openvpn_binary(path: &Path) -> Result<(), String> {
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

fn real_user_home_dir() -> Result<PathBuf, String> {
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

fn terminate_pid(pid: Option<u32>) {
    if let Some(pid) = pid {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

fn openwrap_base_dir(home_dir: &Path) -> PathBuf {
    home_dir.join("Library/Application Support/OpenWrap")
}

fn reconcile_dns_state(runtime_root: &Path) -> Result<(), String> {
    reconcile_runtime_processes(runtime_root)?;

    let state_root = runtime_root.join("dns-state");
    if !state_root.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(&state_root)
        .map_err(|error| format!("failed to read DNS state directory: {error}"))?;
    let mut errors = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to inspect DNS state entry: {error}"))?;
        let profile_dir = entry.path();
        if !profile_dir.is_dir() {
            continue;
        }

        let profile_id = profile_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();

        if let Err(error) = reconcile_global_override(&profile_dir.join("global.tsv")) {
            errors.push(format!(
                "{}: {error}",
                profile_dir.join("global.tsv").display()
            ));
        }

        if let Err(error) = reconcile_dns_routes(&profile_dir.join("dns-routes.tsv")) {
            errors.push(format!(
                "{}: {error}",
                profile_dir.join("dns-routes.tsv").display()
            ));
        }

        if let Err(error) = reconcile_scoped_resolvers(&profile_dir.join("scoped.tsv"), &profile_id)
        {
            errors.push(format!(
                "{}: {error}",
                profile_dir.join("scoped.tsv").display()
            ));
        }

        if let Err(error) = cleanup_transient_dns_files(&profile_dir) {
            errors.push(format!(
                "failed to clean transient DNS state in {}: {error}",
                profile_dir.display()
            ));
        }

        match dir_is_empty(&profile_dir) {
            Ok(true) => {
                if let Err(error) = fs::remove_dir(&profile_dir) {
                    errors.push(format!(
                        "failed to remove DNS state directory {}: {error}",
                        profile_dir.display()
                    ));
                }
            }
            Ok(false) => {}
            Err(error) => {
                errors.push(format!(
                    "failed to inspect DNS state directory {}: {error}",
                    profile_dir.display()
                ));
            }
        }
    }

    match dir_is_empty(&state_root) {
        Ok(true) => {
            if let Err(error) = fs::remove_dir(&state_root) {
                errors.push(format!(
                    "failed to remove DNS state root {}: {error}",
                    state_root.display()
                ));
            }
        }
        Ok(false) => {}
        Err(error) => {
            errors.push(format!(
                "failed to inspect DNS state root {}: {error}",
                state_root.display()
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn reconcile_runtime_processes(runtime_root: &Path) -> Result<(), String> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
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
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            let error = std::io::Error::last_os_error();
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
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            let error = std::io::Error::last_os_error();
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

fn parse_ps_line(line: &str) -> Option<(i32, i32, String)> {
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

fn extract_managed_openvpn_config(command: &str, runtime_root: &Path) -> Option<PathBuf> {
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

fn reconcile_global_override(state_file: &Path) -> Result<(), String> {
    if !state_file.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(state_file)
        .map_err(|error| format!("failed to read DNS restore state: {error}"))?;
    let mut errors = Vec::new();
    for line in contents.lines() {
        let Some((service, current_dns)) = line.split_once('\t') else {
            errors.push(format!("malformed restore state line: {line:?}"));
            continue;
        };
        if service.is_empty() {
            errors.push(format!(
                "restore state entry missing service name: {line:?}"
            ));
            continue;
        }

        let desired_dns = if current_dns == "__EMPTY__" {
            Vec::new()
        } else {
            current_dns
                .split_whitespace()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        };

        match restore_service_dns(service, &desired_dns) {
            Ok(()) => {}
            Err(error) => {
                errors.push(format!(
                    "failed to restore DNS for service {service}: {error}"
                ));
                continue;
            }
        }

        if let Err(error) = verify_service_dns(service, current_dns) {
            errors.push(format!(
                "DNS verification failed for service {service}: {error}"
            ));
        }
    }

    flush_dns_cache();

    if errors.is_empty() {
        let _ = fs::remove_file(state_file);
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn reconcile_scoped_resolvers(state_file: &Path, profile_id: &str) -> Result<(), String> {
    if !state_file.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(state_file)
        .map_err(|error| format!("failed to read scoped DNS state: {error}"))?;
    let profile_marker = format!("# profile_id={profile_id}");
    let mut errors = Vec::new();
    for line in contents.lines() {
        let Some((_, resolver_path)) = line.split_once('\t') else {
            errors.push(format!("malformed scoped state line: {line:?}"));
            continue;
        };
        let resolver_path = PathBuf::from(resolver_path);
        if !resolver_path.exists() {
            continue;
        }

        if !is_openwrap_owned_resolver(&resolver_path, &profile_marker) {
            errors.push(format!(
                "resolver {} is no longer OpenWrap-owned",
                resolver_path.display()
            ));
            continue;
        }

        if fs::remove_file(&resolver_path).is_err() {
            errors.push(format!(
                "failed to remove resolver {}",
                resolver_path.display()
            ));
        }
    }

    flush_dns_cache();

    if errors.is_empty() {
        let _ = fs::remove_file(state_file);
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn reconcile_dns_routes(state_file: &Path) -> Result<(), String> {
    if !state_file.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(state_file)
        .map_err(|error| format!("failed to read DNS route state: {error}"))?;
    let mut errors = Vec::new();
    for line in contents.lines() {
        let Some((dns_server, dns_gateway)) = line.split_once('\t') else {
            errors.push(format!("malformed DNS route state line: {line:?}"));
            continue;
        };
        if dns_server.is_empty() {
            errors.push(format!("DNS route entry missing server: {line:?}"));
            continue;
        }

        let mut deleted = delete_dns_route(dns_server, Some(dns_gateway));
        if !deleted {
            deleted = delete_dns_route(dns_server, None);
        }

        if !deleted {
            errors.push(format!("failed to remove DNS host route for {dns_server}"));
        }
    }

    if errors.is_empty() {
        let _ = fs::remove_file(state_file);
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn is_openwrap_owned_resolver(path: &Path, profile_marker: &str) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    contents.contains("# OpenWrap managed DNS") && contents.contains(profile_marker)
}

fn flush_dns_cache() {
    let _ = std::process::Command::new("/usr/bin/dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = std::process::Command::new("/usr/bin/killall")
        .arg("-HUP")
        .arg("mDNSResponder")
        .status();
}

fn restore_service_dns(service: &str, desired_dns: &[String]) -> Result<(), String> {
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

fn verify_service_dns(service: &str, expected_dns: &str) -> Result<(), String> {
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

fn delete_dns_route(dns_server: &str, gateway: Option<&str>) -> bool {
    let mut command = std::process::Command::new("/sbin/route");
    command.arg("-n").arg("delete").arg("-host").arg(dns_server);
    if let Some(gateway) = gateway.filter(|gateway| !gateway.is_empty()) {
        command.arg(gateway);
    }

    command.status().is_ok_and(|status| status.success())
}

fn normalize_networksetup_dns_output(output: &str) -> String {
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

fn cleanup_transient_dns_files(profile_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(profile_dir).map_err(|error| {
        format!(
            "failed to inspect DNS state directory {}: {error}",
            profile_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect DNS state entry in {}: {error}",
                profile_dir.display()
            )
        })?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let is_transient = file_name.ends_with(".tmp")
            || file_name.contains(".targets.")
            || file_name.contains(".services.")
            || file_name.contains(".devices.");
        if is_transient {
            fs::remove_file(entry.path()).map_err(|error| {
                format!(
                    "failed to remove transient DNS state {}: {error}",
                    entry.path().display()
                )
            })?;
        }
    }

    Ok(())
}

fn dir_is_empty(path: &Path) -> Result<bool, String> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to inspect DNS state directory {}: {error}",
            path.display()
        )
    })?;
    Ok(entries.next().is_none())
}

async fn recv_signal(
    sigterm: &mut Option<tokio::signal::unix::Signal>,
    sigint: &mut Option<tokio::signal::unix::Signal>,
) {
    match (sigterm.as_mut(), sigint.as_mut()) {
        (Some(sigterm), Some(sigint)) => {
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sigint.recv() => {}
            }
        }
        (Some(sigterm), None) => {
            let _ = sigterm.recv().await;
        }
        (None, Some(sigint)) => {
            let _ = sigint.recv().await;
        }
        (None, None) => std::future::pending::<()>().await,
    }
}

async fn pipe_lines<T>(
    stream: T,
    writer: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    is_stdout: bool,
) -> io::Result<()>
where
    T: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await? {
        let event = if is_stdout {
            HelperEvent::Stdout { line }
        } else {
            HelperEvent::Stderr { line }
        };
        emit_event(&writer, &event).await?;
    }
    Ok(())
}

async fn emit_event(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    event: &HelperEvent,
) -> io::Result<()> {
    let serialized = serde_json::to_string(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut writer = writer.lock().await;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{
        cleanup_transient_dns_files, extract_managed_openvpn_config,
        normalize_networksetup_dns_output, parse_ps_line, validate_config_path,
    };

    #[test]
    fn accepts_runtime_launch_configs_anywhere_under_runtime_root() {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().join("OpenWrap");
        let profiles_dir = base_dir.join("profiles");
        let runtime_root = base_dir.join("runtime");
        let stale_runtime_dir = runtime_root.join("profile-a").join("stale-session");
        let active_runtime_dir = runtime_root.join("profile-a").join("active-session");
        let config_path = active_runtime_dir.join("profile.ovpn");

        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&stale_runtime_dir).unwrap();
        fs::create_dir_all(&active_runtime_dir).unwrap();
        fs::write(&config_path, "client\n").unwrap();

        assert!(validate_config_path(&config_path, &profiles_dir, &runtime_root).is_ok());
    }

    #[test]
    fn rejects_configs_outside_managed_roots() {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().join("OpenWrap");
        let profiles_dir = base_dir.join("profiles");
        let runtime_root = base_dir.join("runtime");
        let external_dir = temp.path().join("external");
        let config_path = external_dir.join("profile.ovpn");

        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&runtime_root).unwrap();
        fs::create_dir_all(&external_dir).unwrap();
        fs::write(&config_path, "client\n").unwrap();

        let error = validate_config_path(&config_path, &profiles_dir, &runtime_root).unwrap_err();
        assert!(error.contains("config path escapes the OpenWrap managed directory"));
    }

    #[test]
    fn normalizes_networksetup_empty_dns_output() {
        assert_eq!(
            normalize_networksetup_dns_output("There aren't any DNS Servers set on Wi-Fi.\n"),
            "__EMPTY__"
        );
    }

    #[test]
    fn normalizes_networksetup_dns_list() {
        assert_eq!(
            normalize_networksetup_dns_output("10.0.1.50\n10.0.1.51\n"),
            "10.0.1.50 10.0.1.51"
        );
    }

    #[test]
    fn extracts_managed_openvpn_config_under_runtime_root() {
        let runtime_root = Path::new("/Users/test/Library/Application Support/OpenWrap/runtime");
        let command = "/opt/homebrew/sbin/openvpn --config /Users/test/Library/Application Support/OpenWrap/runtime/profile-a/session/profile.ovpn --auth-nocache --verb 3";

        let extracted = extract_managed_openvpn_config(command, runtime_root).unwrap();
        assert_eq!(
            extracted,
            Path::new(
                "/Users/test/Library/Application Support/OpenWrap/runtime/profile-a/session/profile.ovpn"
            )
        );
    }

    #[test]
    fn ignores_openvpn_configs_outside_runtime_root() {
        let runtime_root = Path::new("/Users/test/Library/Application Support/OpenWrap/runtime");
        let command =
            "/opt/homebrew/sbin/openvpn --config /tmp/profile.ovpn --auth-nocache --verb 3";

        assert!(extract_managed_openvpn_config(command, runtime_root).is_none());
    }

    #[test]
    fn parses_ps_lines_with_variable_spacing() {
        let parsed = parse_ps_line("1     0 /sbin/launchd").unwrap();
        assert_eq!(parsed.0, 1);
        assert_eq!(parsed.1, 0);
        assert_eq!(parsed.2, "/sbin/launchd");
    }

    #[test]
    fn ignores_malformed_ps_lines() {
        assert!(parse_ps_line("garbage").is_none());
        assert!(parse_ps_line("123 onlypid").is_none());
    }

    #[test]
    fn removes_transient_dns_state_files() {
        let temp = tempdir().unwrap();
        let profile_dir = temp.path().join("dns-state").join("profile-a");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("global.tsv.targets.123"), "Wi-Fi\n").unwrap();
        fs::write(profile_dir.join("global.tsv.tmp"), "").unwrap();
        fs::write(profile_dir.join("global.tsv"), "Wi-Fi\t__EMPTY__\n").unwrap();

        cleanup_transient_dns_files(&profile_dir).unwrap();

        assert!(!profile_dir.join("global.tsv.targets.123").exists());
        assert!(!profile_dir.join("global.tsv.tmp").exists());
        assert!(profile_dir.join("global.tsv").exists());
    }
}
