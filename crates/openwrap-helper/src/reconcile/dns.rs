use std::fs;
use std::io::{Read, Write};
use std::net::IpAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::system::{delete_dns_route, flush_dns_cache, restore_service_dns, verify_service_dns};

/// Directory resolver files live in; the only place reconcile deletes from.
const RESOLVER_DIR: &str = "/etc/resolver";

/// Reads a state file from the user-writable runtime directory.
///
/// The helper runs as root, so it refuses to follow a symlink there:
/// otherwise a link to a root-only file would be read (and echoed back in
/// error messages) or overwritten. It also refuses hard links (see
/// [`ensure_single_link_file`]), and opens with `O_NONBLOCK` so a FIFO
/// planted at the path cannot block the helper.
pub fn read_state_file(path: &Path) -> Result<Option<String>, String> {
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "refusing to read DNS state {} (not a regular file?): {error}",
                path.display()
            ))
        }
    };
    ensure_single_link_file(&file, path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|error| format!("failed to read DNS state {}: {error}", path.display()))?;
    Ok(Some(contents))
}

/// The state file must be a regular file with a single link. A hard link
/// from the runtime directory to another file (for example a root-owned
/// system file) would otherwise make root read it and then truncate and
/// rewrite it.
fn ensure_single_link_file(file: &fs::File, path: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "DNS state {} is not a regular file",
            path.display()
        ));
    }
    if metadata.nlink() != 1 {
        return Err(format!(
            "refusing DNS state {}: it has {} hard links",
            path.display(),
            metadata.nlink()
        ));
    }
    Ok(())
}

fn write_state_file(path: &Path, contents: &str) -> Result<(), String> {
    // Open without O_TRUNC and check the file before changing it.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("failed to update DNS restore state: {error}"))?;
    ensure_single_link_file(&file, path)?;
    file.set_len(0)
        .and_then(|()| file.write_all(contents.as_bytes()))
        .map_err(|error| format!("failed to update DNS restore state: {error}"))
}

pub fn reconcile_global_override(state_file: &Path) -> Result<(), String> {
    let Some(contents) = read_state_file(state_file)? else {
        return Ok(());
    };
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
        write_state_file(state_file, &rewritten)?;
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
    let Some(contents) = read_state_file(state_file)? else {
        return Ok(());
    };
    let profile_marker = format!("# profile_id={profile_id}");
    let mut errors = Vec::new();
    for line in contents.lines() {
        let Some((_, resolver_path)) = line.split_once('\t') else {
            errors.push(format!("malformed scoped state line: {line:?}"));
            continue;
        };
        let resolver_path = PathBuf::from(resolver_path);
        if !is_resolver_file_path(&resolver_path) {
            errors.push(format!(
                "refusing to remove {}: not a file in {RESOLVER_DIR}",
                resolver_path.display()
            ));
            continue;
        }
        match fs::symlink_metadata(&resolver_path) {
            Err(_) => continue,
            Ok(metadata) if !metadata.is_file() => {
                errors.push(format!(
                    "refusing to remove {}: not a regular file",
                    resolver_path.display()
                ));
                continue;
            }
            Ok(_) => {}
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
    let Some(contents) = read_state_file(state_file)? else {
        return Ok(());
    };
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
        // Only host routes to DNS server IPs were ever recorded here.
        if dns_server.parse::<IpAddr>().is_err()
            || (!dns_gateway.is_empty() && dns_gateway.parse::<IpAddr>().is_err())
        {
            errors.push(format!("DNS route entry is not an IP address: {line:?}"));
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

/// `/etc/resolver/<name>` with a plain file name, nothing else.
pub fn is_resolver_file_path(path: &Path) -> bool {
    path.parent() == Some(Path::new(RESOLVER_DIR))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| !name.is_empty() && name != ".." && name != ".")
}

pub fn is_openwrap_owned_resolver(path: &Path, profile_marker: &str) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    contents.contains("# OpenWrap managed DNS") && contents.contains(profile_marker)
}
