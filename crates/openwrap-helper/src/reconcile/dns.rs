use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use crate::system::{delete_dns_route, flush_dns_cache, restore_service_dns, verify_service_dns};

pub fn reconcile_global_override(state_file: &Path) -> Result<(), String> {
    if !state_file.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(state_file)
        .map_err(|error| format!("failed to read DNS restore state: {error}"))?;
    let mut errors = Vec::new();
    let mut remaining_entries = Vec::new();
    for line in contents.lines() {
        let Some((service, current_dns)) = parse_global_override_state_line(line) else {
            continue;
        };

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
                remaining_entries.push(line.to_string());
                errors.push(format!(
                    "failed to restore DNS for service {service}: {error}"
                ));
                continue;
            }
        }

        if let Err(error) = verify_service_dns(service, current_dns) {
            remaining_entries.push(line.to_string());
            errors.push(format!(
                "DNS verification failed for service {service}: {error}"
            ));
        }
    }

    flush_dns_cache();

    if remaining_entries.is_empty() {
        let _ = fs::remove_file(state_file);
    } else {
        let rewritten = remaining_entries.join("\n") + "\n";
        fs::write(state_file, rewritten)
            .map_err(|error| format!("failed to update DNS restore state: {error}"))?;
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub fn parse_global_override_state_line(line: &str) -> Option<(&str, &str)> {
    let (service, current_dns) = line.split_once('\t')?;
    if !is_plausible_network_service_name(service) {
        return None;
    }

    if current_dns != "__EMPTY__"
        && current_dns
            .split_whitespace()
            .any(|value| value.parse::<IpAddr>().is_err())
    {
        return None;
    }

    Some((service, current_dns))
}

pub fn is_plausible_network_service_name(service: &str) -> bool {
    if service.is_empty() || service.starts_with("OPENWRAP_DNS_") {
        return false;
    }

    !service.chars().any(|ch| ch.is_control())
}

pub fn reconcile_scoped_resolvers(state_file: &Path, profile_id: &str) -> Result<(), String> {
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

pub fn reconcile_dns_routes(state_file: &Path) -> Result<(), String> {
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

pub fn is_openwrap_owned_resolver(path: &Path, profile_marker: &str) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    contents.contains("# OpenWrap managed DNS") && contents.contains(profile_marker)
}

pub fn cleanup_transient_dns_files(profile_dir: &Path) -> Result<(), String> {
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
