use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::state_machine::ConnectionIntent;
use crate::connection::{ConnectionSnapshot, SessionId};
use crate::dns::DnsObserver;
use crate::errors::AppError;
use crate::logging::SharedSessionLogManager;
use crate::openvpn::{BackendEvent, ConnectRequest};
use crate::profiles::ProfileId;
use crate::{ProfileRepository, VpnBackend};

use super::errors::{apply_terminal_error, persist_failed_connection_log};
use super::events::{handle_exit, schedule_retry, ExitAction};
use super::runtime::{cleanup_runtime_dir, write_auth_file, write_launch_config};
use super::state::{session_is_current, ActiveSession, ConnectionPlan, ManagerState};

pub async fn submit_credentials(
    manager: &super::state::ConnectionManager,
    submission: crate::connection::CredentialSubmission,
) -> Result<ConnectionSnapshot, AppError> {
    manager.submit_credentials(submission).await
}

pub fn start_connect_attempt(
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
    backend: Arc<dyn VpnBackend>,
    dns_observer: Arc<dyn DnsObserver>,
    events: broadcast::Sender<super::state::CoreEvent>,
    state: Arc<Mutex<ManagerState>>,
    session_log: SharedSessionLogManager,
    plan: ConnectionPlan,
    is_retry: bool,
) -> Result<ConnectionSnapshot, AppError> {
    let profile_id = plan.detail.profile.id.clone();
    let profile_name = plan.detail.profile.name.clone();
    let prepare_intent = if is_retry {
        ConnectionIntent::PrepareRetry
    } else {
        ConnectionIntent::PrepareRuntime
    };

    {
        let mut state_guard = state.lock();
        state_guard.pending_credentials = None;
        state_guard.reconnect_plan = Some(plan.clone());
        if !is_retry {
            state_guard.logs.clear();
            state_guard.auto_promoted_policy_persisted = false;
        }
        state_guard.snapshot.state = crate::connection::state_machine::transition(
            state_guard.snapshot.state.clone(),
            prepare_intent,
        )?;
        state_guard.snapshot.profile_id = Some(profile_id.clone());
        state_guard.snapshot.substate = None;
        state_guard.snapshot.pid = None;
        state_guard.snapshot.log_file_path = None;
        state_guard.snapshot.last_error = None;
        state_guard.snapshot.started_at.get_or_insert_with(Utc::now);
        if !is_retry {
            state_guard.snapshot.retry_count = 0;
        }
        state_guard.snapshot.dns_observation = dns_observer.from_profile(
            &plan.detail.profile.dns_intent,
            plan.detail.profile.dns_policy.clone(),
        );
        let dns_obs = &state_guard.snapshot.dns_observation;
        session_log.log_dns(&format!(
            "Initial DNS: {} servers, effective_mode={:?}",
            dns_obs.config_requested.len(),
            dns_obs.effective_mode
        ));
        if !dns_obs.config_requested.is_empty() {
            session_log.log_dns(&format!(
                "DNS config: {}",
                dns_obs.config_requested.join(", ")
            ));
        }
        if !dns_obs.warnings.is_empty() {
            session_log.log_dns(&format!("DNS warnings: {}", dns_obs.warnings.join("; ")));
        }
        let _ = events.send(super::state::CoreEvent::StateChanged(
            state_guard.snapshot.clone(),
        ));
        let _ = events.send(super::state::CoreEvent::DnsObserved(
            state_guard.snapshot.dns_observation.clone(),
        ));
    }

    let settings = match repository.get_settings() {
        Ok(settings) => settings,
        Err(error) => {
            set_terminal_error(&paths, &state, &events, &profile_id, &error);
            return Err(error);
        }
    };
    let detection = crate::detect_openvpn_binaries(settings.openvpn_path_override);
    let openvpn_binary = match detection.selected_path {
        Some(path) => path,
        None => {
            let error = AppError::OpenVpnBinaryNotFound;
            set_terminal_error(&paths, &state, &events, &profile_id, &error);
            return Err(error);
        }
    };

    let session_id = SessionId::new();

    session_log.start_session(&session_id, &profile_id, &profile_name)?;
    session_log.log_core(&format!("Connection attempt started (retry={})", is_retry));

    let runtime_dir = match super::runtime::prepare_runtime_dir(&paths, &profile_id, &session_id) {
        Ok(runtime_dir) => runtime_dir,
        Err(error) => {
            set_terminal_error(&paths, &state, &events, &profile_id, &error);
            return Err(error);
        }
    };
    let auth_file = match write_auth_file(
        &runtime_dir,
        plan.username.as_deref(),
        plan.password.as_deref(),
    ) {
        Ok(auth_file) => auth_file,
        Err(error) => {
            cleanup_runtime_dir(&runtime_dir);
            set_terminal_error(&paths, &state, &events, &profile_id, &error);
            return Err(error);
        }
    };
    let (launch_config_path, extra_cleanup_paths) =
        match write_launch_config(&plan.detail, &runtime_dir) {
            Ok(paths) => paths,
            Err(error) => {
                cleanup_runtime_dir(&runtime_dir);
                set_terminal_error(&paths, &state, &events, &profile_id, &error);
                return Err(error);
            }
        };
    let request = ConnectRequest {
        session_id: session_id.clone(),
        profile_id: profile_id.clone(),
        openvpn_binary,
        config_path: launch_config_path,
        auth_file: auth_file.clone(),
        runtime_dir: runtime_dir.clone(),
    };

    let spawned = match backend.connect(request) {
        Ok(spawned) => spawned,
        Err(error) => {
            cleanup_runtime_dir(&runtime_dir);
            set_terminal_error(&paths, &state, &events, &profile_id, &error);
            return Err(error);
        }
    };

    if let Err(error) = repository.touch_last_used(&profile_id) {
        let _ = backend.disconnect(session_id);
        cleanup_runtime_dir(&runtime_dir);
        set_terminal_error(&paths, &state, &events, &profile_id, &error);
        return Err(error);
    }

    let mut event_rx = spawned.event_rx;
    let observation = dns_observer.from_profile(
        &plan.detail.profile.dns_intent,
        plan.detail.profile.dns_policy.clone(),
    );
    let active_session = {
        let mut state_guard = state.lock();
        state_guard.next_generation += 1;
        let generation = state_guard.next_generation;
        let active_session = ActiveSession {
            session_id: spawned.session_id.clone(),
            generation,
            runtime_dir,
            auth_file,
            extra_cleanup_paths,
        };
        state_guard.active_session = Some(active_session.clone());
        state_guard.snapshot.state = crate::connection::state_machine::transition(
            state_guard.snapshot.state.clone(),
            ConnectionIntent::Spawned,
        )?;
        state_guard.snapshot.pid = spawned.pid;
        state_guard.snapshot.substate = None;
        let _ = events.send(super::state::CoreEvent::StateChanged(
            state_guard.snapshot.clone(),
        ));
        state_guard.snapshot.state = crate::connection::state_machine::transition(
            state_guard.snapshot.state.clone(),
            ConnectionIntent::ProcessStarted,
        )?;
        let _ = events.send(super::state::CoreEvent::StateChanged(
            state_guard.snapshot.clone(),
        ));
        active_session
    };

    let task_state = state.clone();
    let task_session_log = session_log.clone();
    let task_paths = paths.clone();
    let task_repository = repository.clone();
    let task_backend = backend.clone();
    let task_dns_observer = dns_observer.clone();
    let task_profile_id = profile_id.clone();
    let task_events = events.clone();
    tokio::spawn(async move {
        let mut observation = observation;
        while let Some(event) = event_rx.recv().await {
            match event {
                BackendEvent::Started(pid) => {
                    let mut state = task_state.lock();
                    if session_is_current(&state, &active_session) {
                        state.snapshot.pid = pid;
                        let _ = task_events.send(super::state::CoreEvent::StateChanged(
                            state.snapshot.clone(),
                        ));
                    }
                    task_session_log.log_core(&format!(
                        "OpenVPN process started with PID {}",
                        pid.map(|p| p.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    ));
                }
                BackendEvent::Stdout(line) => super::events::handle_log(
                    &task_paths,
                    &task_state,
                    &task_events,
                    &task_repository,
                    &task_profile_id,
                    &task_backend,
                    &task_dns_observer,
                    &active_session,
                    &mut observation,
                    &task_session_log,
                    "stdout",
                    &line,
                ),
                BackendEvent::Stderr(line) => super::events::handle_log(
                    &task_paths,
                    &task_state,
                    &task_events,
                    &task_repository,
                    &task_profile_id,
                    &task_backend,
                    &task_dns_observer,
                    &active_session,
                    &mut observation,
                    &task_session_log,
                    "stderr",
                    &line,
                ),
                BackendEvent::Exited(code) => {
                    task_session_log
                        .log_core(&format!("OpenVPN process exited with code {:?}", code));
                    match handle_exit(
                        &task_paths,
                        &task_state,
                        &task_events,
                        &task_profile_id,
                        &task_backend,
                        &active_session,
                        &task_session_log,
                        code,
                    ) {
                        ExitAction::Stop => break,
                        ExitAction::Retry {
                            delay_seconds,
                            plan,
                        } => {
                            task_session_log.log_core(&format!(
                                "Scheduling retry in {} seconds",
                                delay_seconds
                            ));
                            tokio::spawn(schedule_retry(
                                task_paths.clone(),
                                task_repository.clone(),
                                task_backend.clone(),
                                task_dns_observer.clone(),
                                task_events.clone(),
                                task_state.clone(),
                                task_session_log.clone(),
                                plan,
                                delay_seconds,
                                active_session.generation,
                            ));
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(state.lock().snapshot.clone())
}

fn set_terminal_error(
    paths: &AppPaths,
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<super::state::CoreEvent>,
    profile_id: &ProfileId,
    error: &AppError,
) {
    let mut state_guard = state.lock();
    let log_file_path = persist_failed_connection_log(paths, &state_guard.logs);
    apply_terminal_error(
        &mut state_guard.snapshot,
        log_file_path,
        profile_id,
        crate::errors::UserFacingError::from(error),
    );
    state_guard.active_session = None;
    state_guard.pending_credentials = None;
    state_guard.reconnect_plan = None;
    let _ = events.send(super::state::CoreEvent::StateChanged(
        state_guard.snapshot.clone(),
    ));
}
