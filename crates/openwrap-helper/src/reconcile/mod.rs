pub mod dns;
pub mod processes;

use std::fs;
use std::path::Path;

pub use processes::reconcile_runtime_processes;

use crate::request::{read_json_request, validate_runtime_root, ReconcileDnsRequest};

pub fn run_reconcile_dns() -> i32 {
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

pub fn reconcile_dns_state(runtime_root: &Path) -> Result<(), String> {
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

        if let Err(error) =
            dns::reconcile_scoped_resolvers(&profile_dir.join("scoped.tsv"), &profile_id)
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

pub fn dir_is_empty(path: &Path) -> Result<bool, String> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to inspect DNS state directory {}: {error}",
            path.display()
        )
    })?;
    Ok(entries.next().is_none())
}
