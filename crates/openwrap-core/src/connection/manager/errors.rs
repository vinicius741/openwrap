use std::collections::VecDeque;
use std::io::Write;

use crate::app_state::AppPaths;
use crate::connection::{ConnectionSnapshot, LogEntry};
use crate::dns::DnsRestoreStatus;
use crate::errors::{AppError, UserFacingError};

const FAILED_CONNECTION_LOG_NAME: &str = "last-failed-openvpn.log";
const DNS_RESTORE_PENDING_MESSAGE: &str =
    "DNS restore failed; OpenWrap will retry reconciliation on next launch.";

pub fn process_exit_error(code: Option<i32>, logs: &VecDeque<LogEntry>) -> UserFacingError {
    match code {
        Some(126) => UserFacingError {
            code: "process_permission_denied".into(),
            title: "OpenVPN could not execute".into(),
            message: "The selected OpenVPN binary is not executable.".into(),
            suggested_fix: Some("Check the binary path and file permissions in Settings.".into()),
            details_safe: None,
        },
        Some(127) => UserFacingError {
            code: "process_not_found".into(),
            title: "OpenVPN command failed".into(),
            message: "OpenVPN exited as if the binary or a dependency could not be found.".into(),
            suggested_fix: Some(
                "Verify the installed OpenVPN binary and any required libraries.".into(),
            ),
            details_safe: None,
        },
        _ => crate::connection::log_parser::diagnose_exit_error(code, logs.iter()).unwrap_or_else(
            || {
                let has_logs = !logs.is_empty();
                match code {
                    Some(code) => UserFacingError {
                        code: "process_exit".into(),
                        title: "Connection failed".into(),
                        message: format!("OpenVPN exited with status {code}."),
                        suggested_fix: has_logs
                            .then_some("Use Show logs to inspect the last OpenVPN output.".into()),
                        details_safe: None,
                    },
                    None => UserFacingError {
                        code: "process_terminated".into(),
                        title: "Connection terminated".into(),
                        message: "OpenVPN terminated without reporting an exit status.".into(),
                        suggested_fix: has_logs
                            .then_some("Use Show logs to inspect the last OpenVPN output.".into()),
                        details_safe: None,
                    },
                }
            },
        ),
    }
}

pub fn apply_reconcile_result(
    snapshot: &mut ConnectionSnapshot,
    reconcile_result: &Result<(), AppError>,
) {
    match reconcile_result {
        Ok(()) => {
            if snapshot.dns_observation.restore_status.is_some() {
                snapshot.dns_observation.restore_status = Some(DnsRestoreStatus::Ok);
            }
        }
        Err(_) => {
            snapshot.dns_observation.restore_status = Some(DnsRestoreStatus::PendingReconcile);
            if !snapshot
                .dns_observation
                .warnings
                .iter()
                .any(|warning| warning == DNS_RESTORE_PENDING_MESSAGE)
            {
                snapshot
                    .dns_observation
                    .warnings
                    .push(DNS_RESTORE_PENDING_MESSAGE.into());
            }
        }
    }
}

pub fn dns_restore_error(error: AppError) -> UserFacingError {
    UserFacingError {
        code: "dns_restore_failed".into(),
        title: "DNS restore needs reconciliation".into(),
        message:
            "The VPN disconnected, but OpenWrap could not fully restore your previous system DNS."
                .into(),
        suggested_fix: Some(
            "Relaunch OpenWrap to retry DNS reconciliation, or reconnect and disconnect again after fixing local permissions."
                .into(),
        ),
        details_safe: Some(error.to_string()),
    }
}

pub fn apply_terminal_error(
    snapshot: &mut ConnectionSnapshot,
    log_file_path: Option<String>,
    profile_id: &crate::profiles::ProfileId,
    error: UserFacingError,
) {
    snapshot.state = crate::connection::ConnectionState::Error;
    snapshot.profile_id = Some(profile_id.clone());
    snapshot.pid = None;
    snapshot.substate = None;
    snapshot.log_file_path = log_file_path;
    snapshot.last_error = Some(error);
}

pub fn persist_failed_connection_log(
    paths: &AppPaths,
    logs: &VecDeque<LogEntry>,
) -> Option<String> {
    if logs.is_empty() {
        return None;
    }

    if std::fs::create_dir_all(&paths.logs_dir).is_err() {
        return None;
    }

    let path = paths.failed_connection_log_path();
    debug_assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(FAILED_CONNECTION_LOG_NAME)
    );

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .ok()?;

    for entry in logs {
        let _ = writeln!(
            file,
            "{} [{}] {}",
            entry.ts.to_rfc3339(),
            entry.stream,
            entry.message
        );
    }

    Some(path.to_string_lossy().into_owned())
}

pub fn push_manager_warning(
    state: &std::sync::Arc<parking_lot::Mutex<crate::connection::manager::state::ManagerState>>,
    events: &tokio::sync::broadcast::Sender<crate::connection::manager::state::CoreEvent>,
    message: String,
) {
    let entry = LogEntry {
        ts: chrono::Utc::now(),
        stream: "app".into(),
        level: crate::connection::LogLevel::Warn,
        message,
        sanitized: false,
        classification: "dns_warning".into(),
    };

    {
        let mut state = state.lock();
        state.logs.push_back(entry.clone());
        while state.logs.len() > crate::connection::manager::state::MAX_LOG_ENTRIES {
            state.logs.pop_front();
        }
    }

    let _ = events.send(crate::connection::manager::state::CoreEvent::LogLine(entry));
}
