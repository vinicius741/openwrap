use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::backoff::retry_delay_seconds;
use crate::connection::log_parser::{classify_signal, sanitize_log};
use crate::connection::ConnectionState;
use crate::dns::{extract_dns_directives, DnsObserver, DnsPolicy};
use crate::errors::UserFacingError;
use crate::logging::{SessionOutcome, SharedSessionLogManager};
use crate::openvpn::ReconcileDnsRequest;
use crate::profiles::ProfileId;
use crate::{ProfileRepository, VpnBackend};

use super::errors::{
    apply_reconcile_result, apply_terminal_error, dns_restore_error, persist_failed_connection_log,
    process_exit_error, push_manager_warning,
};
use super::runtime::cleanup_runtime_artifacts;
use super::state::{
    session_is_current, ActiveSession, ConnectionPlan, ManagerState, MAX_LOG_ENTRIES,
};

const AUTO_PROMOTION_PERSIST_FAILED_MESSAGE: &str =
    "OpenWrap switched this connection to Full override, but could not save that policy for future connections.";

pub enum ExitAction {
    Stop,
    Retry {
        delay_seconds: u64,
        plan: ConnectionPlan,
    },
}

pub fn handle_log(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<super::state::CoreEvent>,
    repository: &Arc<dyn ProfileRepository>,
    profile_id: &ProfileId,
    backend: &Arc<dyn VpnBackend>,
    dns_observer: &Arc<dyn DnsObserver>,
    active_session: &ActiveSession,
    observation: &mut crate::dns::DnsObservation,
    session_log: &SharedSessionLogManager,
    stream: &str,
    line: &str,
) {
    let entry = sanitize_log(stream, line);
    let signal = classify_signal(line);
    let mut disconnect_session = None;
    let mut emit_state_changed = false;
    let mut persist_auto_promoted_policy = false;

    session_log.log_openvpn(&format!("[{}] {}", stream, line));

    {
        let mut state_guard = state.lock();
        if !session_is_current(&state_guard, active_session) {
            return;
        }

        state_guard.logs.push_back(entry.clone());
        while state_guard.logs.len() > MAX_LOG_ENTRIES {
            state_guard.logs.pop_front();
        }

        match signal {
            crate::connection::log_parser::ParsedLogSignal::Connected => {
                state_guard.snapshot.state = ConnectionState::Connected;
                state_guard.snapshot.substate = None;
                state_guard.snapshot.log_file_path = None;
                state_guard.snapshot.last_error = None;
                emit_state_changed = true;
                session_log.log_core("State transition: Connected");
            }
            crate::connection::log_parser::ParsedLogSignal::AuthFailed => {
                let error = UserFacingError {
                    code: "auth_failed".into(),
                    title: "Authentication failed".into(),
                    message: "OpenVPN reported an authentication failure.".into(),
                    suggested_fix: Some("Re-enter your username and password.".into()),
                    details_safe: None,
                };
                let log_file_path = persist_failed_connection_log(paths, &state_guard.logs);
                apply_terminal_error(&mut state_guard.snapshot, log_file_path, profile_id, error);
                state_guard.pending_credentials = None;
                state_guard.reconnect_plan = None;
                state_guard.active_session = None;
                disconnect_session = Some(active_session.session_id.clone());
                emit_state_changed = true;
                session_log.log_core("State transition: Error (auth_failed)");
            }
            crate::connection::log_parser::ParsedLogSignal::RetryableFailure => {
                if state_guard.snapshot.state != ConnectionState::Disconnecting {
                    state_guard.snapshot.state = ConnectionState::Reconnecting;
                    state_guard.snapshot.substate = Some("OpenVPN requested a restart.".into());
                    emit_state_changed = true;
                    session_log.log_core("State transition: Reconnecting (retryable failure)");
                }
            }
            crate::connection::log_parser::ParsedLogSignal::DnsHint => {
                let extracted = extract_dns_directives(line);
                let changed = dns_observer.update_from_log(observation, line);
                if is_runtime_dns_diagnostic(line) {
                    session_log.log_dns(&format!("runtime: {}", line));
                }
                if !extracted.is_empty() {
                    session_log.log_dns(&format!(
                        "DNS hint extracted: {} (changed={})",
                        extracted.join(", "),
                        changed
                    ));
                }
                if changed {
                    state_guard.snapshot.dns_observation = observation.clone();
                    session_log.log_dns(&format_dns_observation(observation));
                    if observation.auto_promoted_policy == Some(DnsPolicy::FullOverride)
                        && !state_guard.auto_promoted_policy_persisted
                    {
                        state_guard.auto_promoted_policy_persisted = true;
                        if let Some(plan) = state_guard.reconnect_plan.as_mut() {
                            plan.detail.profile.dns_policy = DnsPolicy::FullOverride;
                        }
                        persist_auto_promoted_policy = true;
                        session_log.log_dns("DNS policy auto-promoted to FullOverride");
                    }
                    let _ = events.send(super::state::CoreEvent::DnsObserved(observation.clone()));
                }
            }
            crate::connection::log_parser::ParsedLogSignal::None => {}
        }

        if emit_state_changed {
            let _ = events.send(super::state::CoreEvent::StateChanged(
                state_guard.snapshot.clone(),
            ));
        }
    }

    let _ = events.send(super::state::CoreEvent::LogLine(entry));

    if let Some(session_id) = disconnect_session {
        let _ = backend.disconnect(session_id);
    }

    if persist_auto_promoted_policy {
        if let Err(error) =
            repository.update_profile_dns_policy(profile_id, DnsPolicy::FullOverride)
        {
            push_manager_warning(
                state,
                events,
                format!("{AUTO_PROMOTION_PERSIST_FAILED_MESSAGE} {error}"),
            );
        }
    }
}

pub fn handle_exit(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<super::state::CoreEvent>,
    profile_id: &ProfileId,
    backend: &Arc<dyn VpnBackend>,
    active_session: &ActiveSession,
    session_log: &SharedSessionLogManager,
    code: Option<i32>,
) -> ExitAction {
    cleanup_runtime_artifacts(active_session);

    let dns_before_reconcile = {
        let state_guard = state.lock();
        state_guard.snapshot.dns_observation.clone()
    };
    session_log.log_dns(&format!(
        "DNS before reconciliation: effective_mode={:?}, restore_status={:?}",
        dns_before_reconcile.effective_mode, dns_before_reconcile.restore_status
    ));

    let reconcile_result = backend.reconcile_dns(ReconcileDnsRequest {
        runtime_root: paths.runtime_dir.clone(),
    });

    match &reconcile_result {
        Ok(()) => {
            session_log.log_dns("DNS reconciliation succeeded");
        }
        Err(e) => {
            session_log.log_dns(&format!("DNS reconciliation failed: {}", e));
        }
    }

    let mut state_guard = state.lock();
    if !session_is_current(&state_guard, active_session) {
        return ExitAction::Stop;
    }

    state_guard.active_session = None;
    state_guard.snapshot.pid = None;
    apply_reconcile_result(&mut state_guard.snapshot, &reconcile_result);

    if state_guard.snapshot.state == ConnectionState::Disconnecting {
        state_guard.pending_credentials = None;
        state_guard.reconnect_plan = None;
        state_guard.auto_promoted_policy_persisted = false;
        if let Err(error) = reconcile_result {
            session_log.log_core(&format!("DNS reconciliation failed: {}", error));
            apply_terminal_error(
                &mut state_guard.snapshot,
                None,
                profile_id,
                dns_restore_error(error),
            );
            let _ = events.send(super::state::CoreEvent::StateChanged(
                state_guard.snapshot.clone(),
            ));
            let _ = events.send(super::state::CoreEvent::DnsObserved(
                state_guard.snapshot.dns_observation.clone(),
            ));
            session_log.end_session(SessionOutcome::Failed);
            return ExitAction::Stop;
        }

        state_guard.snapshot = crate::connection::ConnectionSnapshot::default();
        let _ = events.send(super::state::CoreEvent::StateChanged(
            state_guard.snapshot.clone(),
        ));
        let _ = events.send(super::state::CoreEvent::DnsObserved(
            state_guard.snapshot.dns_observation.clone(),
        ));
        session_log.log_core("State transition: Idle (clean disconnect)");
        session_log.end_session(SessionOutcome::Success);
        return ExitAction::Stop;
    }

    if state_guard
        .snapshot
        .last_error
        .as_ref()
        .map(|error| error.code.as_str())
        == Some("auth_failed")
    {
        state_guard.snapshot.state = ConnectionState::Error;
        state_guard.snapshot.substate = None;
        let _ = events.send(super::state::CoreEvent::StateChanged(
            state_guard.snapshot.clone(),
        ));
        session_log.end_session(SessionOutcome::Failed);
        return ExitAction::Stop;
    }

    if let Some(delay_seconds) = retry_delay_seconds(state_guard.snapshot.retry_count) {
        if let Some(plan) = state_guard.reconnect_plan.clone() {
            state_guard.snapshot.state = ConnectionState::Reconnecting;
            state_guard.snapshot.profile_id = Some(profile_id.clone());
            state_guard.snapshot.retry_count += 1;
            state_guard.snapshot.substate = Some(format!("Retrying in {delay_seconds} seconds"));
            state_guard.snapshot.last_error = None;
            let _ = events.send(super::state::CoreEvent::StateChanged(
                state_guard.snapshot.clone(),
            ));
            return ExitAction::Retry {
                delay_seconds,
                plan,
            };
        }
    }

    let error = process_exit_error(code, &state_guard.logs);
    let log_file_path = persist_failed_connection_log(paths, &state_guard.logs);
    apply_terminal_error(&mut state_guard.snapshot, log_file_path, profile_id, error);
    state_guard.pending_credentials = None;
    state_guard.reconnect_plan = None;
    state_guard.auto_promoted_policy_persisted = false;
    let _ = events.send(super::state::CoreEvent::DnsObserved(
        state_guard.snapshot.dns_observation.clone(),
    ));
    let _ = events.send(super::state::CoreEvent::StateChanged(
        state_guard.snapshot.clone(),
    ));
    session_log.end_session(SessionOutcome::Failed);
    ExitAction::Stop
}

pub async fn schedule_retry(
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
    backend: Arc<dyn VpnBackend>,
    dns_observer: Arc<dyn DnsObserver>,
    events: broadcast::Sender<super::state::CoreEvent>,
    state: Arc<Mutex<ManagerState>>,
    session_log: SharedSessionLogManager,
    plan: ConnectionPlan,
    delay_seconds: u64,
    previous_generation: u64,
) {
    tokio::time::sleep(Duration::from_secs(delay_seconds)).await;

    {
        let state_guard = state.lock();
        if state_guard.active_session.is_some()
            || state_guard.snapshot.state != ConnectionState::Reconnecting
            || state_guard.snapshot.profile_id.as_ref() != Some(&plan.detail.profile.id)
            || state_guard.next_generation != previous_generation
        {
            return;
        }
    }

    let _ = super::connect::start_connect_attempt(
        paths,
        repository,
        backend,
        dns_observer,
        events.clone(),
        state.clone(),
        session_log,
        plan.clone(),
        true,
    );
}

fn format_dns_observation(obs: &crate::dns::DnsObservation) -> String {
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

fn is_runtime_dns_diagnostic(line: &str) -> bool {
    line.contains("OPENWRAP_DNS_DEBUG:")
        || line.contains("OPENWRAP_DNS_ERROR:")
        || line.contains("OPENWRAP_DNS_WARNING:")
}
