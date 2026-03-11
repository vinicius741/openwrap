use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::config::{parse_profile, rewrite_profile};
use crate::connection::backoff::retry_delay_seconds;
use crate::connection::log_parser::{
    classify_signal, diagnose_exit_error, sanitize_log, ParsedLogSignal,
};
use crate::connection::state_machine::{transition, ConnectionIntent};
use crate::connection::{
    ConnectionSnapshot, ConnectionState, CredentialPrompt, CredentialSubmission, LogEntry,
    SessionId,
};
use crate::dns::{DnsObserver, PassiveDnsObserver};
use crate::errors::{AppError, UserFacingError};
use crate::openvpn::{BackendEvent, ConnectRequest};
use crate::profiles::{CredentialMode, ProfileDetail, ProfileId};
use crate::{ProfileRepository, SecretStore, VpnBackend};

const MAX_LOG_ENTRIES: usize = 500;
const AUTH_FILE_NAME: &str = "auth.txt";
const LAUNCH_CONFIG_FILE_NAME: &str = "profile.ovpn";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    StateChanged(ConnectionSnapshot),
    LogLine(LogEntry),
    CredentialsRequested(CredentialPrompt),
    DnsObserved(crate::dns::DnsObservation),
}

#[derive(Clone)]
struct PendingCredentials {
    profile: ProfileDetail,
}

#[derive(Clone)]
struct ConnectionPlan {
    detail: ProfileDetail,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Clone)]
struct ActiveSession {
    session_id: SessionId,
    generation: u64,
    runtime_dir: PathBuf,
    auth_file: Option<PathBuf>,
}

struct ManagerState {
    snapshot: ConnectionSnapshot,
    logs: VecDeque<LogEntry>,
    pending_credentials: Option<PendingCredentials>,
    active_session: Option<ActiveSession>,
    reconnect_plan: Option<ConnectionPlan>,
    next_generation: u64,
}

impl Default for ManagerState {
    fn default() -> Self {
        Self {
            snapshot: ConnectionSnapshot::default(),
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            pending_credentials: None,
            active_session: None,
            reconnect_plan: None,
            next_generation: 0,
        }
    }
}

pub struct ConnectionManager {
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
    secret_store: Arc<dyn SecretStore>,
    backend: Arc<dyn VpnBackend>,
    dns_observer: Arc<dyn DnsObserver>,
    events: broadcast::Sender<CoreEvent>,
    state: Arc<Mutex<ManagerState>>,
}

impl ConnectionManager {
    pub fn new(
        paths: AppPaths,
        repository: Arc<dyn ProfileRepository>,
        secret_store: Arc<dyn SecretStore>,
        backend: Arc<dyn VpnBackend>,
    ) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            paths,
            repository,
            secret_store,
            backend,
            dns_observer: Arc::new(PassiveDnsObserver),
            events,
            state: Arc::new(Mutex::new(ManagerState::default())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    pub fn snapshot(&self) -> ConnectionSnapshot {
        self.state.lock().snapshot.clone()
    }

    pub fn recent_logs(&self, limit: usize) -> Vec<LogEntry> {
        self.state
            .lock()
            .logs
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub async fn connect(&self, profile_id: String) -> Result<ConnectionSnapshot, AppError> {
        let profile_id = profile_id
            .parse::<ProfileId>()
            .map_err(|error| AppError::ConnectionState(error.to_string()))?;
        let detail = self.repository.get_profile(&profile_id)?;
        self.repository
            .set_last_selected_profile(Some(&profile_id))?;

        self.transition_snapshot(ConnectionIntent::BeginConnect, Some(profile_id.clone()))?;

        if detail.profile.credential_mode == CredentialMode::UserPass {
            let saved_username = self
                .secret_store
                .get_password(&profile_id)?
                .map(|secret| secret.username);

            {
                let mut state = self.state.lock();
                state.pending_credentials = Some(PendingCredentials {
                    profile: detail.clone(),
                });
            }
            self.transition_snapshot(ConnectionIntent::NeedCredentials, Some(profile_id.clone()))?;
            let prompt = CredentialPrompt {
                profile_id,
                remember_supported: true,
                saved_username,
            };
            let _ = self.events.send(CoreEvent::CredentialsRequested(prompt));
            return Ok(self.snapshot());
        }

        self.start_connect(detail, None, None, false).await
    }

    pub async fn submit_credentials(
        &self,
        submission: CredentialSubmission,
    ) -> Result<ConnectionSnapshot, AppError> {
        let profile = {
            let mut state = self.state.lock();
            let pending = state
                .pending_credentials
                .take()
                .ok_or_else(|| AppError::ConnectionState("no pending credential prompt".into()))?;
            pending.profile
        };

        if submission.remember_in_keychain {
            self.secret_store
                .set_password(crate::secrets::StoredSecret {
                    profile_id: submission.profile_id.clone(),
                    username: submission.username.clone(),
                })?;
            self.repository
                .update_has_saved_credentials(&submission.profile_id, true)?;
        } else {
            self.secret_store.delete_password(&submission.profile_id)?;
            self.repository
                .update_has_saved_credentials(&submission.profile_id, false)?;
        }

        self.start_connect(
            profile,
            Some(submission.username),
            Some(submission.password),
            false,
        )
        .await
    }

    pub async fn disconnect(&self) -> Result<ConnectionSnapshot, AppError> {
        let session = {
            let state = self.state.lock();
            state.active_session.clone()
        };

        if let Some(session) = session {
            self.transition_snapshot(
                ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            self.clear_pending_connect_state();
            self.backend.disconnect(session.session_id.clone())?;
            cleanup_runtime_artifacts(&session);
            return Ok(self.snapshot());
        }

        if self.snapshot().state != ConnectionState::Idle {
            self.transition_snapshot(
                ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            self.finish_disconnect();
        }

        Ok(self.snapshot())
    }

    pub fn shutdown(&self) -> Result<(), AppError> {
        let session = {
            let mut state = self.state.lock();
            state.pending_credentials = None;
            state.reconnect_plan = None;
            state.active_session.clone()
        };

        if let Some(session) = session {
            let _ = self.backend.disconnect(session.session_id.clone());
            cleanup_runtime_artifacts(&session);
        }

        self.finish_disconnect();
        Ok(())
    }

    pub async fn disconnect_if_connected(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        let is_connected = {
            let state = self.state.lock();
            state.snapshot.profile_id.as_ref() == Some(profile_id)
                && state.snapshot.state != ConnectionState::Idle
        };

        if is_connected {
            eprintln!("Disconnecting profile {} before deletion", profile_id);
            self.disconnect().await?;
        }
        Ok(())
    }

    async fn start_connect(
        &self,
        detail: ProfileDetail,
        username: Option<String>,
        password: Option<String>,
        is_retry: bool,
    ) -> Result<ConnectionSnapshot, AppError> {
        let plan = ConnectionPlan {
            detail,
            username,
            password,
        };
        start_connect_attempt(
            self.paths.clone(),
            self.repository.clone(),
            self.backend.clone(),
            self.dns_observer.clone(),
            self.events.clone(),
            self.state.clone(),
            plan,
            is_retry,
        )
    }

    fn transition_snapshot(
        &self,
        intent: ConnectionIntent,
        profile_id: Option<ProfileId>,
    ) -> Result<(), AppError> {
        let mut state = self.state.lock();
        state.snapshot.state = transition(state.snapshot.state.clone(), intent)?;
        state.snapshot.profile_id = profile_id;
        state.snapshot.last_error = None;
        state.snapshot.substate = None;
        state.snapshot.pid = None;
        state.snapshot.started_at.get_or_insert_with(Utc::now);
        let _ = self
            .events
            .send(CoreEvent::StateChanged(state.snapshot.clone()));
        Ok(())
    }

    fn clear_pending_connect_state(&self) {
        let mut state = self.state.lock();
        state.pending_credentials = None;
        state.reconnect_plan = None;
    }

    fn finish_disconnect(&self) {
        let mut state = self.state.lock();
        state.pending_credentials = None;
        state.reconnect_plan = None;
        state.active_session = None;
        state.snapshot = ConnectionSnapshot::default();
        let _ = self
            .events
            .send(CoreEvent::StateChanged(state.snapshot.clone()));
        let _ = self.events.send(CoreEvent::DnsObserved(
            state.snapshot.dns_observation.clone(),
        ));
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn start_connect_attempt(
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
    backend: Arc<dyn VpnBackend>,
    dns_observer: Arc<dyn DnsObserver>,
    events: broadcast::Sender<CoreEvent>,
    state: Arc<Mutex<ManagerState>>,
    plan: ConnectionPlan,
    is_retry: bool,
) -> Result<ConnectionSnapshot, AppError> {
    let profile_id = plan.detail.profile.id.clone();
    let prepare_intent = if is_retry {
        ConnectionIntent::PrepareRetry
    } else {
        ConnectionIntent::PrepareRuntime
    };

    {
        let mut state = state.lock();
        state.pending_credentials = None;
        state.reconnect_plan = Some(plan.clone());
        state.snapshot.state = transition(state.snapshot.state.clone(), prepare_intent)?;
        state.snapshot.profile_id = Some(profile_id.clone());
        state.snapshot.substate = None;
        state.snapshot.pid = None;
        state.snapshot.last_error = None;
        state.snapshot.started_at.get_or_insert_with(Utc::now);
        if !is_retry {
            state.snapshot.retry_count = 0;
        }
        state.snapshot.dns_observation = dns_observer.from_profile(&plan.detail.profile.dns_intent);
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        let _ = events.send(CoreEvent::DnsObserved(
            state.snapshot.dns_observation.clone(),
        ));
    }

    let settings = match repository.get_settings() {
        Ok(settings) => settings,
        Err(error) => {
            set_terminal_error(&state, &events, &profile_id, &error);
            return Err(error);
        }
    };
    let detection = crate::detect_openvpn_binaries(settings.openvpn_path_override);
    let openvpn_binary = match detection.selected_path {
        Some(path) => path,
        None => {
            let error = AppError::OpenVpnBinaryNotFound;
            set_terminal_error(&state, &events, &profile_id, &error);
            return Err(error);
        }
    };

    let session_id = SessionId::new();
    let runtime_dir = match prepare_runtime_dir(&paths, &profile_id, &session_id) {
        Ok(runtime_dir) => runtime_dir,
        Err(error) => {
            set_terminal_error(&state, &events, &profile_id, &error);
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
            set_terminal_error(&state, &events, &profile_id, &error);
            return Err(error);
        }
    };
    let launch_config_path = match write_launch_config(&plan.detail, &runtime_dir) {
        Ok(path) => path,
        Err(error) => {
            cleanup_runtime_dir(&runtime_dir);
            set_terminal_error(&state, &events, &profile_id, &error);
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
            set_terminal_error(&state, &events, &profile_id, &error);
            return Err(error);
        }
    };

    if let Err(error) = repository.touch_last_used(&profile_id) {
        let _ = backend.disconnect(session_id);
        cleanup_runtime_dir(&runtime_dir);
        set_terminal_error(&state, &events, &profile_id, &error);
        return Err(error);
    }

    let mut event_rx = spawned.event_rx;
    let observation = dns_observer.from_profile(&plan.detail.profile.dns_intent);
    let active_session = {
        let mut state = state.lock();
        state.next_generation += 1;
        let generation = state.next_generation;
        let active_session = ActiveSession {
            session_id: spawned.session_id.clone(),
            generation,
            runtime_dir,
            auth_file,
        };
        state.active_session = Some(active_session.clone());
        state.snapshot.state = transition(state.snapshot.state.clone(), ConnectionIntent::Spawned)?;
        state.snapshot.pid = spawned.pid;
        state.snapshot.substate = None;
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        state.snapshot.state = transition(
            state.snapshot.state.clone(),
            ConnectionIntent::ProcessStarted,
        )?;
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
        active_session
    };

    let task_state = state.clone();
    tokio::spawn(async move {
        let mut observation = observation;
        while let Some(event) = event_rx.recv().await {
            match event {
                BackendEvent::Started(pid) => {
                    let mut state = task_state.lock();
                    if session_is_current(&state, &active_session) {
                        state.snapshot.pid = pid;
                        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
                    }
                }
                BackendEvent::Stdout(line) => handle_log(
                    &task_state,
                    &events,
                    &profile_id,
                    &backend,
                    &dns_observer,
                    &active_session,
                    &mut observation,
                    "stdout",
                    &line,
                ),
                BackendEvent::Stderr(line) => handle_log(
                    &task_state,
                    &events,
                    &profile_id,
                    &backend,
                    &dns_observer,
                    &active_session,
                    &mut observation,
                    "stderr",
                    &line,
                ),
                BackendEvent::Exited(code) => {
                    match handle_exit(&task_state, &events, &profile_id, &active_session, code) {
                        ExitAction::Stop => break,
                        ExitAction::Retry {
                            delay_seconds,
                            plan,
                        } => {
                            tokio::spawn(schedule_retry(
                                paths.clone(),
                                repository.clone(),
                                backend.clone(),
                                dns_observer.clone(),
                                events.clone(),
                                task_state.clone(),
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

fn handle_log(
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    profile_id: &ProfileId,
    backend: &Arc<dyn VpnBackend>,
    dns_observer: &Arc<dyn DnsObserver>,
    active_session: &ActiveSession,
    observation: &mut crate::dns::DnsObservation,
    stream: &str,
    line: &str,
) {
    let entry = sanitize_log(stream, line);
    let signal = classify_signal(line);
    let mut disconnect_session = None;

    {
        let mut state = state.lock();
        if !session_is_current(&state, active_session) {
            return;
        }

        state.logs.push_back(entry.clone());
        while state.logs.len() > MAX_LOG_ENTRIES {
            state.logs.pop_front();
        }

        match signal {
            ParsedLogSignal::Connected => {
                state.snapshot.state = ConnectionState::Connected;
                state.snapshot.substate = None;
                state.snapshot.last_error = None;
            }
            ParsedLogSignal::AuthFailed => {
                state.snapshot.state = ConnectionState::Error;
                state.snapshot.substate = None;
                state.snapshot.last_error = Some(UserFacingError {
                    code: "auth_failed".into(),
                    title: "Authentication failed".into(),
                    message: "OpenVPN reported an authentication failure.".into(),
                    suggested_fix: Some("Re-enter your username and password.".into()),
                    details_safe: None,
                });
                state.reconnect_plan = None;
                disconnect_session = Some(active_session.session_id.clone());
            }
            ParsedLogSignal::RetryableFailure => {
                if state.snapshot.state != ConnectionState::Disconnecting {
                    state.snapshot.state = ConnectionState::Reconnecting;
                    state.snapshot.substate = Some("OpenVPN requested a restart.".into());
                }
            }
            ParsedLogSignal::DnsHint => {
                if dns_observer.update_from_log(observation, line) {
                    state.snapshot.dns_observation = observation.clone();
                    let _ = events.send(CoreEvent::DnsObserved(observation.clone()));
                }
            }
            ParsedLogSignal::None => {}
        }

        state.snapshot.profile_id = Some(profile_id.clone());
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    }

    let _ = events.send(CoreEvent::LogLine(entry));

    if let Some(session_id) = disconnect_session {
        let _ = backend.disconnect(session_id);
    }
}

enum ExitAction {
    Stop,
    Retry {
        delay_seconds: u64,
        plan: ConnectionPlan,
    },
}

fn handle_exit(
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    profile_id: &ProfileId,
    active_session: &ActiveSession,
    code: Option<i32>,
) -> ExitAction {
    cleanup_runtime_artifacts(active_session);

    let mut state = state.lock();
    if !session_is_current(&state, active_session) {
        return ExitAction::Stop;
    }

    state.active_session = None;
    state.snapshot.pid = None;

    if state.snapshot.state == ConnectionState::Disconnecting {
        state.pending_credentials = None;
        state.reconnect_plan = None;
        state.snapshot = ConnectionSnapshot::default();
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
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
        return ExitAction::Stop;
    }

    if let Some(delay_seconds) = retry_delay_seconds(state.snapshot.retry_count) {
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

    state.snapshot.state = ConnectionState::Error;
    state.snapshot.profile_id = Some(profile_id.clone());
    state.snapshot.substate = None;
    state.snapshot.last_error = Some(process_exit_error(code, state.logs.iter()));
    let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    ExitAction::Stop
}

async fn schedule_retry(
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
    backend: Arc<dyn VpnBackend>,
    dns_observer: Arc<dyn DnsObserver>,
    events: broadcast::Sender<CoreEvent>,
    state: Arc<Mutex<ManagerState>>,
    plan: ConnectionPlan,
    delay_seconds: u64,
    previous_generation: u64,
) {
    tokio::time::sleep(Duration::from_secs(delay_seconds)).await;

    {
        let state = state.lock();
        if state.active_session.is_some()
            || state.snapshot.state != ConnectionState::Reconnecting
            || state.snapshot.profile_id.as_ref() != Some(&plan.detail.profile.id)
            || state.next_generation != previous_generation
        {
            return;
        }
    }

    let _ = start_connect_attempt(
        paths,
        repository,
        backend,
        dns_observer,
        events.clone(),
        state.clone(),
        plan.clone(),
        true,
    );
}

fn set_terminal_error(
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    profile_id: &ProfileId,
    error: &AppError,
) {
    let mut state = state.lock();
    state.active_session = None;
    state.pending_credentials = None;
    state.reconnect_plan = None;
    state.snapshot.state = ConnectionState::Error;
    state.snapshot.profile_id = Some(profile_id.clone());
    state.snapshot.pid = None;
    state.snapshot.substate = None;
    state.snapshot.last_error = Some(UserFacingError::from(error));
    let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
}

fn session_is_current(state: &ManagerState, active_session: &ActiveSession) -> bool {
    state.active_session.as_ref().is_some_and(|current| {
        current.session_id == active_session.session_id
            && current.generation == active_session.generation
    })
}

fn prepare_runtime_dir(
    paths: &AppPaths,
    profile_id: &ProfileId,
    session_id: &SessionId,
) -> Result<PathBuf, AppError> {
    let profile_dir = paths.runtime_dir.join(profile_id.to_string());
    if profile_dir.exists() {
        fs::remove_dir_all(&profile_dir)?;
    }

    let runtime_dir = profile_dir.join(session_id.to_string());
    fs::create_dir_all(&runtime_dir)?;
    tighten_dir_permissions(&profile_dir)?;
    tighten_dir_permissions(&runtime_dir)?;
    Ok(runtime_dir)
}

fn write_auth_file(
    runtime_dir: &Path,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Option<PathBuf>, AppError> {
    match (username, password) {
        (Some(username), Some(password)) => {
            let auth_path = runtime_dir.join(AUTH_FILE_NAME);
            let mut file = auth_file_options().open(&auth_path)?;
            use std::io::Write;
            file.write_all(format!("{username}\n{password}\n").as_bytes())?;
            Ok(Some(auth_path))
        }
        _ => Ok(None),
    }
}

fn write_launch_config(detail: &ProfileDetail, runtime_dir: &Path) -> Result<PathBuf, AppError> {
    let source = fs::read_to_string(&detail.profile.managed_ovpn_path)?;
    let parsed = parse_profile(&source, &detail.profile.managed_dir)?;
    let rewritten_assets = detail
        .assets
        .iter()
        .map(|asset| {
            (
                asset.kind.clone(),
                quote_openvpn_arg(&detail.profile.managed_dir.join(&asset.relative_path)),
            )
        })
        .collect::<HashMap<_, _>>();

    let launch_config_path = runtime_dir.join(LAUNCH_CONFIG_FILE_NAME);
    fs::write(
        &launch_config_path,
        rewrite_profile(&parsed, &rewritten_assets),
    )?;
    Ok(launch_config_path)
}

fn quote_openvpn_arg(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn cleanup_runtime_artifacts(active_session: &ActiveSession) {
    if let Some(auth_file) = &active_session.auth_file {
        let _ = fs::remove_file(auth_file);
    }
    cleanup_runtime_dir(&active_session.runtime_dir);
}

fn cleanup_runtime_dir(runtime_dir: &Path) {
    let _ = fs::remove_dir_all(runtime_dir);
    if let Some(parent) = runtime_dir.parent() {
        if parent
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(parent);
        }
    }
}

#[cfg(unix)]
fn auth_file_options() -> OpenOptions {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(0o600);
    options
}

#[cfg(not(unix))]
fn auth_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    options
}

#[cfg(unix)]
fn tighten_dir_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn tighten_dir_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

fn process_exit_error<'a>(
    code: Option<i32>,
    logs: impl DoubleEndedIterator<Item = &'a LogEntry>,
) -> UserFacingError {
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
        _ => diagnose_exit_error(code, logs).unwrap_or_else(|| match code {
            Some(code) => UserFacingError {
                code: "process_exit".into(),
                title: "Connection failed".into(),
                message: format!("OpenVPN exited with status {code}."),
                suggested_fix: Some(
                    "Inspect the connection log for the underlying failure reason.".into(),
                ),
                details_safe: None,
            },
            None => UserFacingError {
                code: "process_terminated".into(),
                title: "Connection terminated".into(),
                message: "OpenVPN terminated without reporting an exit status.".into(),
                suggested_fix: Some(
                    "Inspect the connection log for the underlying failure reason.".into(),
                ),
                details_safe: None,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use parking_lot::Mutex;
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    use super::{ConnectionManager, CoreEvent};
    use crate::app_state::AppPaths;
    use crate::connection::{ConnectionState, CredentialSubmission, SessionId};
    use crate::errors::AppError;
    use crate::openvpn::{BackendEvent, ConnectRequest, SpawnedSession};
    use crate::profiles::repository::ProfileRepository;
    use crate::profiles::{
        AssetId, AssetKind, AssetOrigin, CredentialMode, ManagedAsset, Profile, ProfileDetail,
        ProfileId, ProfileImportResult, ProfileSummary, ValidationFinding, ValidationStatus,
    };
    use crate::secrets::StoredSecret;
    use crate::{SecretStore, VpnBackend};

    #[derive(Default)]
    struct FakeSecretStore {
        secrets: Mutex<HashMap<ProfileId, StoredSecret>>,
    }

    impl SecretStore for FakeSecretStore {
        fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError> {
            Ok(self.secrets.lock().get(profile_id).cloned())
        }

        fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
            self.secrets
                .lock()
                .insert(secret.profile_id.clone(), secret);
            Ok(())
        }

        fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
            self.secrets.lock().remove(profile_id);
            Ok(())
        }
    }

    struct FakeRepository {
        detail: ProfileDetail,
        settings: crate::openvpn::runtime::Settings,
        last_selected: Mutex<Option<ProfileId>>,
        touch_count: Mutex<u32>,
        saved_credentials: Mutex<bool>,
    }

    impl ProfileRepository for FakeRepository {
        fn save_import(&self, _import: ProfileImportResult) -> Result<ProfileDetail, AppError> {
            unreachable!()
        }

        fn list_profiles(&self) -> Result<Vec<ProfileSummary>, AppError> {
            Ok(vec![])
        }

        fn get_profile(&self, profile_id: &ProfileId) -> Result<ProfileDetail, AppError> {
            if &self.detail.profile.id == profile_id {
                Ok(self.detail.clone())
            } else {
                Err(AppError::ProfileNotFound(profile_id.to_string()))
            }
        }

        fn update_has_saved_credentials(
            &self,
            _profile_id: &ProfileId,
            has_saved_credentials: bool,
        ) -> Result<(), AppError> {
            *self.saved_credentials.lock() = has_saved_credentials;
            Ok(())
        }

        fn touch_last_used(&self, _profile_id: &ProfileId) -> Result<(), AppError> {
            *self.touch_count.lock() += 1;
            Ok(())
        }

        fn get_settings(&self) -> Result<crate::openvpn::runtime::Settings, AppError> {
            Ok(self.settings.clone())
        }

        fn save_settings(
            &self,
            _settings: &crate::openvpn::runtime::Settings,
        ) -> Result<(), AppError> {
            unreachable!()
        }

        fn list_validation_findings(
            &self,
            _profile_id: &ProfileId,
        ) -> Result<Vec<ValidationFinding>, AppError> {
            Ok(vec![])
        }

        fn set_last_selected_profile(
            &self,
            profile_id: Option<&ProfileId>,
        ) -> Result<(), AppError> {
            *self.last_selected.lock() = profile_id.cloned();
            Ok(())
        }

        fn get_last_selected_profile(&self) -> Result<Option<ProfileId>, AppError> {
            Ok(self.last_selected.lock().clone())
        }

        fn delete_profile(&self, _profile_id: &ProfileId) -> Result<(), AppError> {
            Ok(())
        }
    }

    enum QueuedConnect {
        Session {
            pid: Option<u32>,
            event_rx: mpsc::UnboundedReceiver<BackendEvent>,
        },
        Error(AppError),
    }

    #[derive(Default)]
    struct FakeBackendState {
        queue: VecDeque<QueuedConnect>,
        requests: Vec<ConnectRequest>,
        disconnects: Vec<SessionId>,
    }

    #[derive(Clone, Default)]
    struct FakeBackend {
        state: Arc<Mutex<FakeBackendState>>,
    }

    #[derive(Clone)]
    struct ScriptedSession {
        tx: mpsc::UnboundedSender<BackendEvent>,
    }

    impl FakeBackend {
        fn queue_session(&self, pid: Option<u32>) -> ScriptedSession {
            let (tx, rx) = mpsc::unbounded_channel();
            self.state
                .lock()
                .queue
                .push_back(QueuedConnect::Session { pid, event_rx: rx });
            ScriptedSession { tx }
        }

        fn queue_error(&self, error: AppError) {
            self.state
                .lock()
                .queue
                .push_back(QueuedConnect::Error(error));
        }

        fn request_count(&self) -> usize {
            self.state.lock().requests.len()
        }

        fn last_request(&self) -> Option<ConnectRequest> {
            self.state.lock().requests.last().cloned()
        }

        fn disconnect_count(&self) -> usize {
            self.state.lock().disconnects.len()
        }
    }

    impl VpnBackend for FakeBackend {
        fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError> {
            self.state.lock().requests.push(request.clone());
            match self
                .state
                .lock()
                .queue
                .pop_front()
                .expect("expected queued connection")
            {
                QueuedConnect::Session { pid, event_rx } => Ok(SpawnedSession {
                    session_id: request.session_id,
                    pid,
                    event_rx,
                }),
                QueuedConnect::Error(error) => Err(error),
            }
        }

        fn disconnect(&self, session_id: SessionId) -> Result<(), AppError> {
            self.state.lock().disconnects.push(session_id);
            Ok(())
        }
    }

    fn build_manager(
        credential_mode: CredentialMode,
        saved_username: Option<&str>,
    ) -> (
        ConnectionManager,
        FakeBackend,
        ProfileId,
        Arc<FakeSecretStore>,
        Arc<FakeRepository>,
    ) {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().to_path_buf();
        std::mem::forget(temp);

        let paths = AppPaths::new(&base_dir);
        paths.ensure().unwrap();

        let openvpn_path = base_dir.join("openvpn");
        fs::write(&openvpn_path, "#!/bin/sh\n").unwrap();

        let profile_id = ProfileId::new();
        let managed_dir = base_dir.join("profiles").join(profile_id.to_string());
        fs::create_dir_all(&managed_dir).unwrap();
        let asset_path = managed_dir.join("assets").join("tls-auth.key");
        fs::create_dir_all(asset_path.parent().unwrap()).unwrap();
        fs::write(&asset_path, "static-key").unwrap();
        let managed_ovpn_path = managed_dir.join("config.ovpn");
        fs::write(
            &managed_ovpn_path,
            "client\nremote example.com 1194\ntls-auth assets/tls-auth.key 1\n",
        )
        .unwrap();

        let detail = ProfileDetail {
            profile: Profile {
                id: profile_id.clone(),
                name: "Test".into(),
                source_filename: "test.ovpn".into(),
                managed_dir,
                managed_ovpn_path,
                original_import_path: base_dir.join("test.ovpn"),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                dns_intent: vec!["DNS 1.1.1.1".into()],
                credential_mode,
                remote_summary: "example.com:1194".into(),
                has_saved_credentials: false,
                validation_status: ValidationStatus::Ok,
            },
            assets: vec![ManagedAsset {
                id: AssetId::new(),
                profile_id: profile_id.clone(),
                kind: AssetKind::TlsAuth,
                relative_path: "assets/tls-auth.key".into(),
                sha256: "sha".into(),
                origin: AssetOrigin::CopiedFile,
            }],
            findings: vec![],
        };
        let repository = Arc::new(FakeRepository {
            detail,
            settings: crate::openvpn::runtime::Settings {
                openvpn_path_override: Some(openvpn_path),
            },
            last_selected: Mutex::new(None),
            touch_count: Mutex::new(0),
            saved_credentials: Mutex::new(saved_username.is_some()),
        });
        let backend = FakeBackend::default();
        let secret_store = Arc::new(FakeSecretStore::default());
        if let Some(username) = saved_username {
            secret_store
                .set_password(StoredSecret {
                    profile_id: profile_id.clone(),
                    username: username.into(),
                })
                .unwrap();
        }
        let manager = ConnectionManager::new(
            paths,
            repository.clone(),
            secret_store.clone(),
            Arc::new(backend.clone()),
        );

        (manager, backend, profile_id, secret_store, repository)
    }

    #[tokio::test(start_paused = true)]
    async fn retries_after_exit_and_recovers() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let first = backend.queue_session(Some(41));
        let second = backend.queue_session(Some(42));

        manager.connect(profile_id.to_string()).await.unwrap();
        assert_eq!(manager.snapshot().state, ConnectionState::Connecting);

        let first_runtime = backend.last_request().unwrap().runtime_dir;
        assert!(first_runtime.exists());

        first.tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let reconnecting = manager.snapshot();
        assert_eq!(reconnecting.state, ConnectionState::Reconnecting);
        assert_eq!(reconnecting.retry_count, 1);
        assert_eq!(
            reconnecting.substate.as_deref(),
            Some("Retrying in 2 seconds")
        );
        assert!(!first_runtime.exists());

        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;

        assert_eq!(backend.request_count(), 2);
        assert_eq!(manager.snapshot().state, ConnectionState::Connecting);

        second
            .tx
            .send(BackendEvent::Stdout(
                "Initialization Sequence Completed".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;

        let connected = manager.snapshot();
        assert_eq!(connected.state, ConnectionState::Connected);
        assert_eq!(connected.retry_count, 1);
    }

    #[tokio::test]
    async fn writes_runtime_launch_config_with_absolute_asset_paths() {
        let (manager, backend, profile_id, _, repository) =
            build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(43));
        let asset_path = repository
            .detail
            .profile
            .managed_dir
            .join("assets")
            .join("tls-auth.key");

        manager.connect(profile_id.to_string()).await.unwrap();

        let request = backend.last_request().unwrap();
        let launch_config = fs::read_to_string(&request.config_path).unwrap();
        assert!(request.config_path.starts_with(&request.runtime_dir));
        assert!(launch_config.contains(&format!("tls-auth \"{}\" 1", asset_path.display())));

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stops_after_retry_budget_is_exhausted() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let sessions = [
            backend.queue_session(Some(11)),
            backend.queue_session(Some(12)),
            backend.queue_session(Some(13)),
            backend.queue_session(Some(14)),
        ];

        manager.connect(profile_id.to_string()).await.unwrap();
        let delays = [2_u64, 5, 10];

        for (index, delay) in delays.into_iter().enumerate() {
            sessions[index]
                .tx
                .send(BackendEvent::Exited(Some(1)))
                .unwrap();
            tokio::task::yield_now().await;
            assert_eq!(manager.snapshot().state, ConnectionState::Reconnecting);
            tokio::time::advance(Duration::from_secs(delay)).await;
            tokio::task::yield_now().await;
        }

        sessions[3].tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        assert_eq!(backend.request_count(), 4);
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(failed.retry_count, 3);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("process_exit")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn surfaces_last_openvpn_diagnostic_after_retry_budget_is_exhausted() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let sessions = [
            backend.queue_session(Some(21)),
            backend.queue_session(Some(22)),
            backend.queue_session(Some(23)),
            backend.queue_session(Some(24)),
        ];

        manager.connect(profile_id.to_string()).await.unwrap();
        let delays = [2_u64, 5, 10];

        for (index, delay) in delays.into_iter().enumerate() {
            sessions[index]
                .tx
                .send(BackendEvent::Exited(Some(1)))
                .unwrap();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(delay)).await;
            tokio::task::yield_now().await;
        }

        sessions[3]
            .tx
            .send(BackendEvent::Stderr(
                "RESOLVE: Cannot resolve host address: vpn.example.invalid:1194".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        sessions[3].tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        let last_error = failed.last_error.expect("expected terminal error");
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(last_error.code, "openvpn_host_resolution_failed");
        assert!(last_error
            .details_safe
            .as_deref()
            .is_some_and(|detail| detail.contains("Cannot resolve host address")));
    }

    #[tokio::test]
    async fn cleans_runtime_artifacts_for_credentials_and_disconnect() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::UserPass, None);
        let session = backend.queue_session(Some(51));

        manager.connect(profile_id.to_string()).await.unwrap();
        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );

        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "alice".into(),
                password: "secret".into(),
                remember_in_keychain: false,
            })
            .await
            .unwrap();

        let request = backend.last_request().unwrap();
        let auth_file = request.auth_file.clone().unwrap();
        let runtime_dir = request.runtime_dir.clone();
        assert!(auth_file.exists());
        assert!(runtime_dir.exists());

        manager.disconnect().await.unwrap();
        assert_eq!(backend.disconnect_count(), 1);
        assert!(!auth_file.exists());
        assert!(!runtime_dir.exists());

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
        tokio::task::yield_now().await;

        assert_eq!(manager.snapshot().state, ConnectionState::Idle);
    }

    #[tokio::test(start_paused = true)]
    async fn auth_failures_do_not_retry() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(61));

        manager.connect(profile_id.to_string()).await.unwrap();
        session
            .tx
            .send(BackendEvent::Stdout("AUTH_FAILED".into()))
            .unwrap();
        tokio::task::yield_now().await;
        session.tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(20)).await;
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(backend.request_count(), 1);
        assert_eq!(backend.disconnect_count(), 1);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("auth_failed")
        );
    }

    #[tokio::test]
    async fn launch_failures_surface_as_terminal_errors() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        backend.queue_error(AppError::OpenVpnLaunch("permission denied".into()));

        let error = manager.connect(profile_id.to_string()).await.unwrap_err();
        assert!(matches!(error, AppError::OpenVpnLaunch(_)));

        let failed = manager.snapshot();
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("openvpn_launch_failed")
        );
    }

    #[tokio::test]
    async fn prompts_for_credentials_without_saved_username() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::UserPass, None);
        let mut events = manager.subscribe();

        manager.connect(profile_id.to_string()).await.unwrap();

        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );
        assert_eq!(backend.request_count(), 0);

        loop {
            match events.recv().await.unwrap() {
                CoreEvent::CredentialsRequested(prompt) => {
                    assert_eq!(prompt.profile_id, profile_id);
                    assert_eq!(prompt.saved_username, None);
                    assert!(prompt.remember_supported);
                    break;
                }
                CoreEvent::StateChanged(_) | CoreEvent::LogLine(_) | CoreEvent::DnsObserved(_) => {}
            }
        }
    }

    #[tokio::test]
    async fn prompts_with_saved_username_and_does_not_autoconnect() {
        let (manager, backend, profile_id, _, _) =
            build_manager(CredentialMode::UserPass, Some("alice"));
        let mut events = manager.subscribe();

        manager.connect(profile_id.to_string()).await.unwrap();

        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );
        assert_eq!(backend.request_count(), 0);

        loop {
            match events.recv().await.unwrap() {
                CoreEvent::CredentialsRequested(prompt) => {
                    assert_eq!(prompt.profile_id, profile_id);
                    assert_eq!(prompt.saved_username.as_deref(), Some("alice"));
                    assert!(prompt.remember_supported);
                    break;
                }
                CoreEvent::StateChanged(_) | CoreEvent::LogLine(_) | CoreEvent::DnsObserved(_) => {}
            }
        }
    }

    #[tokio::test]
    async fn remember_username_persists_only_the_username() {
        let (manager, backend, profile_id, secret_store, repository) =
            build_manager(CredentialMode::UserPass, None);
        backend.queue_session(Some(71));

        manager.connect(profile_id.to_string()).await.unwrap();
        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "alice".into(),
                password: "secret".into(),
                remember_in_keychain: true,
            })
            .await
            .unwrap();

        let stored = secret_store.get_password(&profile_id).unwrap().unwrap();
        assert_eq!(stored.username, "alice");
        assert!(*repository.saved_credentials.lock());
    }

    #[tokio::test]
    async fn unchecked_remember_removes_saved_username() {
        let (manager, backend, profile_id, secret_store, repository) =
            build_manager(CredentialMode::UserPass, Some("alice"));
        backend.queue_session(Some(72));

        manager.connect(profile_id.to_string()).await.unwrap();
        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "bob".into(),
                password: "secret".into(),
                remember_in_keychain: false,
            })
            .await
            .unwrap();

        assert!(secret_store.get_password(&profile_id).unwrap().is_none());
        assert!(!*repository.saved_credentials.lock());
    }
}
