mod dns;
mod processes;

use std::fs;
use std::path::Path;

use dns::reconcile_scoped_resolvers;
use processes::reconcile_runtime_processes;

use crate::system::dir_is_empty;

pub fn run_reconcile_dns() -> i32 {
    let request =
        match crate::request::read_json_request::<openwrap_core::openvpn::ReconcileDnsRequest>() {
            Ok(request) => request,
            Err(error) => {
                eprintln!("{error}");
                return 64;
            }
        };

    if let Err(error) = crate::request::validate_runtime_root(&request.runtime_root) {
        eprintln!("{error}");
        return 78;
    }

    if let Err(error) = reconcile_dns_state(&request.runtime_root) {
        eprintln!("{error}");
        return 70;
    }

    0
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

        if let Err(error) = dns::reconcile_global_override(&profile_dir.join("global.tsv")) {
            errors.push(format!(
                "{}: {error}",
                profile_dir.join("global.tsv").display()
            ));
        }

        if let Err(error) = dns::reconcile_dns_routes(&profile_dir.join("dns-routes.tsv")) {
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

        if let Err(error) = dns::cleanup_transient_dns_files(&profile_dir) {
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

#[cfg(test)]
pub use dns::{cleanup_transient_dns_files, parse_global_override_state_line, reconcile_global_override};
#[cfg(test)]
pub use processes::{extract_managed_openvpn_config, parse_ps_line};
