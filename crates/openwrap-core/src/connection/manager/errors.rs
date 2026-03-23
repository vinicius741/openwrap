use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;

use chrono::Utc;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::{LogEntry, LogLevel};
use crate::dns::DnsRestoreStatus;
use crate::errors::{AppError, UserFacingError};
use crate::profiles::ProfileId;

use super::state::{self, ManagerState};
use super::CoreEvent;

const FAILED_CONNECTION_LOG_NAME: &str = "last-failed-openvpn.log";
pub(crate) const AUTO_PROMOTION_PERSIST_FAILED_MESSAGE: &str =
    "OpenWrap switched this connection to Full override, but could not save that policy for future connections.";
const DNS_RESTORE_PENDING_MESSAGE: &str =
    "DNS restore failed; OpenWrap will retry reconciliation on next launch.";

pub(crate) fn process_exit_error(code: Option<i32>, logs: &VecDeque<LogEntry>) -> UserFacingError {
    use crate::connection::log_parser::diagnose_exit_error;

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
        _ => diagnose_exit_error(code, logs.iter()).unwrap_or_else(|| {
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
        }),
    }
}

pub(crate) fn apply_reconcile_result(
    snapshot: &mut crate::connection::ConnectionSnapshot,
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

pub(crate) fn dns_restore_error(error: AppError) -> UserFacingError {
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

pub(crate) fn push_manager_warning(
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    message: String,
) {
    let entry = LogEntry {
        ts: Utc::now(),
        stream: "app".into(),
        level: LogLevel::Warn,
        message,
        sanitized: false,
        classification: "dns_warning".into(),
    };

    {
        let mut state = state.lock();
        state::push_log(&mut state, entry.clone());
    }

    let _ = events.send(CoreEvent::LogLine(entry));
}

pub(crate) fn set_terminal_error(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    profile_id: &ProfileId,
    error: &AppError,
) {
    let mut state = state.lock();
    let log_file_path = state::with_logs(&mut state, |logs| {
        persist_failed_connection_log(paths, logs)
    });
    state::apply_terminal_error(
        &mut state.snapshot,
        log_file_path,
        profile_id,
        UserFacingError::from(error),
    );
    state.active_session = None;
    state.pending_credentials = None;
    state.reconnect_plan = None;
    let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
}

pub(crate) fn persist_failed_connection_log(
    paths: &AppPaths,
    logs: &VecDeque<LogEntry>,
) -> Option<String> {
    if logs.is_empty() {
        return None;
    }

    if fs::create_dir_all(&paths.logs_dir).is_err() {
        return None;
    }

    let path = paths.failed_connection_log_path();
    debug_assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(FAILED_CONNECTION_LOG_NAME)
    );

    let mut file = OpenOptions::new()
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

pub(crate) fn format_dns_observation(obs: &crate::dns::DnsObservation) -> String {
    let mut parts = Vec::new();
    parts.push(format!("effective_mode={:?}", obs.effective_mode));
    if !obs.config_requested.is_empty() {
        parts.push(format!("requested={}", obs.config_requested.join("|")));
    }
    if !obs.runtime_pushed.is_empty() {
        parts.push(format!("pushed={}", obs.runtime_pushed.join("|")));
    }
    if let Some(auto) = &obs.auto_promoted_policy {
        parts.push(format!("auto_promoted={:?}", auto));
    }
    if let Some(restore) = &obs.restore_status {
        parts.push(format!("restore_status={:?}", restore));
    }
    if !obs.warnings.is_empty() {
        parts.push(format!("warnings={}", obs.warnings.join(";")));
    }
    format!("DNS: {}", parts.join(", "))
}

pub(crate) fn is_runtime_dns_diagnostic(line: &str) -> bool {
    line.contains("OPENWRAP_DNS_DEBUG:")
        || line.contains("OPENWRAP_DNS_ERROR:")
        || line.contains("OPENWRAP_DNS_WARNING:")
}
