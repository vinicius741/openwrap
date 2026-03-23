use std::process::Command;

pub fn flush_dns_cache() {
    let _ = Command::new("/usr/bin/dscacheutil")
        .arg("-flushcache")
        .status();
    let _ = Command::new("/usr/bin/killall")
        .arg("-HUP")
        .arg("mDNSResponder")
        .status();
}

pub fn restore_service_dns(service: &str, desired_dns: &[String]) -> Result<(), String> {
    let mut command = Command::new("/usr/sbin/networksetup");
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
    let output = Command::new("/usr/sbin/networksetup")
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
    let mut command = Command::new("/sbin/route");
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

pub fn kill_process(pid: i32) -> std::io::Result<()> {
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn run_ps() -> std::io::Result<std::process::Output> {
    Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
}
