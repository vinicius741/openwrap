use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::log_parser::{classify_signal, sanitize_log, ParsedLogSignal};
use crate::connection::ConnectionState;
use crate::dns::{extract_dns_directives, DnsObserver, DnsPolicy};
use crate::logging::{SessionOutcome, SharedSessionLogManager};
use crate::openvpn::ReconcileDnsRequest;
use crate::profiles::ProfileId;
use crate::{ProfileRepository, VpnBackend};

use super::errors::{
    apply_reconcile_result, dns_restore_error, format_dns_observation, is_runtime_dns_diagnostic,
    persist_failed_connection_log, process_exit_error, push_manager_warning,
    AUTO_PROMOTION_PERSIST_FAILED_MESSAGE,
};
use super::runtime::cleanup_runtime_artifacts;
use super::state::{
    apply_terminal_error, push_log, session_is_current, with_logs, ActiveSession, ConnectionPlan,
    CoreEvent, ManagerState,
};

pub(crate) fn handle_log(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
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
        let mut state = state.lock();
        if !session_is_current(&state, active_session) {
            return;
        }

        push_log(&mut state, entry.clone());

        match signal {
            ParsedLogSignal::Connected => {
                state.snapshot.state = ConnectionState::Connected;
                state.snapshot.substate = None;
                state.snapshot.log_file_path = None;
                state.snapshot.last_error = None;
                emit_state_changed = true;
                session_log.log_core("State transition: Connected");
            }
            ParsedLogSignal::AuthFailed => {
                let error = crate::errors::UserFacingError {
                    code: "auth_failed".into(),
                    title: "Authentication failed".into(),
                    message: "OpenVPN reported an authentication failure.".into(),
                    suggested_fix: Some("Re-enter your username and password.".into()),
                    details_safe: None,
                };
                let log_file_path = with_logs(&mut state, |logs| {
                    persist_failed_connection_log(paths, logs)
                });
                apply_terminal_error(&mut state.snapshot, log_file_path, profile_id, error);
                state.pending_credentials = None;
                state.reconnect_plan = None;
                state.active_session = None;
                disconnect_session = Some(active_session.session_id.clone());
                emit_state_changed = true;
                session_log.log_core("State transition: Error (auth_failed)");
            }
            ParsedLogSignal::RetryableFailure => {
                if state.snapshot.state != ConnectionState::Disconnecting {
                    state.snapshot.state = ConnectionState::Reconnecting;
                    state.snapshot.substate = Some("OpenVPN requested a restart.".into());
                    emit_state_changed = true;
                    session_log.log_core("State transition: Reconnecting (retryable failure)");
                }
            }
            ParsedLogSignal::DnsHint => {
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
                    state.snapshot.dns_observation = observation.clone();
                    session_log.log_dns(&format_dns_observation(observation));
                    if observation.auto_promoted_policy == Some(DnsPolicy::FullOverride)
                        && !state.auto_promoted_policy_persisted
                    {
                        state.auto_promoted_policy_persisted = true;
                        if let Some(plan) = state.reconnect_plan.as_mut() {
                            plan.detail.profile.dns_policy = DnsPolicy::FullOverride;
                        }
                        persist_auto_promoted_policy = true;
                        session_log.log_dns("DNS policy auto-promoted to FullOverride");
                    }
                    let _ = events.send(CoreEvent::DnsObserved(observation.clone()));
                }
            }
            ParsedLogSignal::None => {}
        }

        if emit_state_changed {
            let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        }
    }

    let _ = events.send(CoreEvent::LogLine(entry));

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

pub(crate) enum ExitAction {
    Stop,
    Retry {
        delay_seconds: u64,
        plan: ConnectionPlan,
    },
}

pub(crate) fn handle_exit(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
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

    let mut state = state.lock();
    if !session_is_current(&state, active_session) {
        return ExitAction::Stop;
    }

    state.active_session = None;
    state.snapshot.pid = None;
    apply_reconcile_result(&mut state.snapshot, &reconcile_result);

    if state.snapshot.state == ConnectionState::Disconnecting {
        state.pending_credentials = None;
        state.reconnect_plan = None;
        state.auto_promoted_policy_persisted = false;
        if let Err(error) = reconcile_result {
            session_log.log_core(&format!("DNS reconciliation failed: {}", error));
            apply_terminal_error(
                &mut state.snapshot,
                None,
                profile_id,
                dns_restore_error(error),
            );
            let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
            let _ = events.send(CoreEvent::DnsObserved(
                state.snapshot.dns_observation.clone(),
            ));
            session_log.end_session(SessionOutcome::Failed);
            return ExitAction::Stop;
        }

        state.snapshot = crate::connection::ConnectionSnapshot::default();
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        let _ = events.send(CoreEvent::DnsObserved(
            state.snapshot.dns_observation.clone(),
        ));
        session_log.log_core("State transition: Idle (clean disconnect)");
        session_log.end_session(SessionOutcome::Success);
        return ExitAction::Stop;
    }

    if state
        .snapshot
        .last_error
        .as_ref()
        .map(|error| error.code.as_str())
        == Some("auth_failed")
    {
        state.snapshot.state = ConnectionState::Error;
        state.snapshot.substate = None;
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        session_log.end_session(SessionOutcome::Failed);
        return ExitAction::Stop;
    }

    if let Some(delay_seconds) =
        crate::connection::backoff::retry_delay_seconds(state.snapshot.retry_count)
    {
        if let Some(plan) = state.reconnect_plan.clone() {
            state.snapshot.state = ConnectionState::Reconnecting;
            state.snapshot.profile_id = Some(profile_id.clone());
            state.snapshot.retry_count += 1;
            state.snapshot.substate = Some(format!("Retrying in {delay_seconds} seconds"));
            state.snapshot.last_error = None;
            let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
            return ExitAction::Retry {
                delay_seconds,
                plan,
            };
        }
    }

    let error = with_logs(&mut state, |logs| process_exit_error(code, logs));
    let log_file_path = with_logs(&mut state, |logs| {
        persist_failed_connection_log(paths, logs)
    });
    apply_terminal_error(&mut state.snapshot, log_file_path, profile_id, error);
    state.pending_credentials = None;
    state.reconnect_plan = None;
    state.auto_promoted_policy_persisted = false;
    let _ = events.send(CoreEvent::DnsObserved(
        state.snapshot.dns_observation.clone(),
    ));
    let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    session_log.end_session(SessionOutcome::Failed);
    ExitAction::Stop
}
