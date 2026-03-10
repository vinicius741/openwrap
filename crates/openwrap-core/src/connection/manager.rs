use std::collections::VecDeque;
use std::fs;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::backoff::retry_delay_seconds;
use crate::connection::log_parser::{classify_signal, sanitize_log, ParsedLogSignal};
use crate::connection::state_machine::{transition, ConnectionIntent};
use crate::connection::{
    ConnectionSnapshot, ConnectionState, CredentialPrompt, CredentialSubmission, LogEntry, SessionId,
};
use crate::dns::{DnsObserver, PassiveDnsObserver};
use crate::errors::{AppError, UserFacingError};
use crate::openvpn::{BackendEvent, ConnectRequest};
use crate::profiles::{CredentialMode, ProfileDetail, ProfileId};
use crate::{ProfileRepository, SecretStore, VpnBackend};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    StateChanged(ConnectionSnapshot),
    LogLine(LogEntry),
    CredentialsRequested(CredentialPrompt),
    DnsObserved(crate::dns::DnsObservation),
}

struct PendingCredentials {
    profile: ProfileDetail,
}

struct ActiveSession {
    session_id: SessionId,
}

struct ManagerState {
    snapshot: ConnectionSnapshot,
    logs: VecDeque<LogEntry>,
    pending_credentials: Option<PendingCredentials>,
    active_session: Option<ActiveSession>,
}

impl Default for ManagerState {
    fn default() -> Self {
        Self {
            snapshot: ConnectionSnapshot::default(),
            logs: VecDeque::with_capacity(500),
            pending_credentials: None,
            active_session: None,
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
        self.repository.set_last_selected_profile(Some(&profile_id))?;
        self.set_state(transition(
            self.snapshot().state,
            ConnectionIntent::BeginConnect,
        )?, Some(profile_id.clone()), None, None);

        if detail.profile.credential_mode == CredentialMode::UserPass {
            if let Some(secret) = self.secret_store.get_password(&profile_id)? {
                return self.start_connect(detail, Some(secret.username), Some(secret.password)).await;
            }

            self.state.lock().pending_credentials = Some(PendingCredentials { profile: detail.clone() });
            self.set_state(
                transition(self.snapshot().state, ConnectionIntent::NeedCredentials)?,
                Some(profile_id.clone()),
                None,
                None,
            );
            let prompt = CredentialPrompt {
                profile_id,
                remember_supported: true,
            };
            let _ = self.events.send(CoreEvent::CredentialsRequested(prompt));
            return Ok(self.snapshot());
        }

        self.start_connect(detail, None, None).await
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
            self.secret_store.set_password(crate::secrets::StoredSecret {
                profile_id: submission.profile_id.clone(),
                username: submission.username.clone(),
                password: submission.password.clone(),
            })?;
            self.repository
                .update_has_saved_credentials(&submission.profile_id, true)?;
        }

        self.set_state(
            transition(self.snapshot().state, ConnectionIntent::CredentialsReady)?,
            Some(submission.profile_id),
            None,
            None,
        );

        self.start_connect(profile, Some(submission.username), Some(submission.password))
            .await
    }

    pub async fn disconnect(&self) -> Result<ConnectionSnapshot, AppError> {
        let session_id = self
            .state
            .lock()
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone());

        if let Some(session_id) = session_id {
            self.set_state(
                transition(self.snapshot().state, ConnectionIntent::BeginDisconnect)?,
                self.snapshot().profile_id.clone(),
                None,
                None,
            );
            self.backend.disconnect(session_id)?;
            self.finish_disconnect();
        }

        Ok(self.snapshot())
    }

    async fn start_connect(
        &self,
        detail: ProfileDetail,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<ConnectionSnapshot, AppError> {
        self.set_state(
            transition(self.snapshot().state, ConnectionIntent::PrepareRuntime)?,
            Some(detail.profile.id.clone()),
            None,
            None,
        );

        let settings = self.repository.get_settings()?;
        let detection = crate::detect_openvpn_binaries(settings.openvpn_path_override.clone());
        let binary = detection
            .selected_path
            .ok_or(AppError::OpenVpnBinaryNotFound)?;

        let runtime_dir = self.paths.runtime_dir.join(detail.profile.id.to_string());
        fs::create_dir_all(&runtime_dir)?;
        let auth_file = if let (Some(username), Some(password)) = (username, password) {
            let auth_path = runtime_dir.join("auth.txt");
            fs::write(&auth_path, format!("{username}\n{password}\n"))?;
            Some(auth_path)
        } else {
            None
        };

        let session_id = SessionId::new();
        let request = ConnectRequest {
            session_id: session_id.clone(),
            profile_id: detail.profile.id.clone(),
            openvpn_binary: binary,
            config_path: detail.profile.managed_ovpn_path.clone(),
            auth_file,
            runtime_dir,
        };

        let spawned = self.backend.connect(request)?;
        self.repository.touch_last_used(&detail.profile.id)?;
        self.set_state(
            transition(self.snapshot().state, ConnectionIntent::Spawned)?,
            Some(detail.profile.id.clone()),
            spawned.pid,
            None,
        );
        self.set_state(
            transition(self.snapshot().state, ConnectionIntent::PrepareRuntime)?,
            Some(detail.profile.id.clone()),
            spawned.pid,
            None,
        );

        let mut observation = self.dns_observer.from_profile(&detail.profile.dns_intent);
        let state = self.state.clone();
        let events = self.events.clone();
        let backend = self.backend.clone();
        let profile_id = detail.profile.id.clone();
        let task_session_id = session_id.clone();
        tokio::spawn(async move {
            let mut event_rx = spawned.event_rx;
            while let Some(event) = event_rx.recv().await {
                match event {
                    BackendEvent::Stdout(line) => handle_log(
                        &state,
                        &events,
                        &profile_id,
                        &backend,
                        &task_session_id,
                        &mut observation,
                        "stdout",
                        &line,
                    ),
                    BackendEvent::Stderr(line) => handle_log(
                        &state,
                        &events,
                        &profile_id,
                        &backend,
                        &task_session_id,
                        &mut observation,
                        "stderr",
                        &line,
                    ),
                    BackendEvent::Exited(code) => {
                        let mut state = state.lock();
                        if state.snapshot.state == ConnectionState::Disconnecting {
                            state.snapshot = ConnectionSnapshot::default();
                            state.active_session = None;
                            let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
                            break;
                        }

                        match retry_delay_seconds(state.snapshot.retry_count) {
                            Some(_) => {
                                state.snapshot.state = ConnectionState::Reconnecting;
                                state.snapshot.retry_count += 1;
                                let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
                            }
                            None => {
                                state.snapshot.state = ConnectionState::Error;
                                state.snapshot.last_error = Some(UserFacingError {
                                    code: "process_exit".into(),
                                    title: "Connection failed".into(),
                                    message: format!("OpenVPN exited with status {:?}.", code),
                                    suggested_fix: Some("Inspect the log output for the failure reason.".into()),
                                    details_safe: None,
                                });
                                state.active_session = None;
                                let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
                            }
                        }
                    }
                }
            }
        });

        self.state.lock().active_session = Some(ActiveSession { session_id });
        Ok(self.snapshot())
    }

    fn set_state(
        &self,
        state_value: ConnectionState,
        profile_id: Option<ProfileId>,
        pid: Option<u32>,
        last_error: Option<UserFacingError>,
    ) {
        let mut state = self.state.lock();
        state.snapshot.state = state_value;
        state.snapshot.profile_id = profile_id;
        state.snapshot.pid = pid;
        state.snapshot.started_at.get_or_insert_with(Utc::now);
        state.snapshot.last_error = last_error;
        let _ = self.events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    }

    fn finish_disconnect(&self) {
        let mut state = self.state.lock();
        state.snapshot = ConnectionSnapshot::default();
        state.active_session = None;
        let _ = self.events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    }
}

fn handle_log(
    state: &Arc<Mutex<ManagerState>>,
    events: &broadcast::Sender<CoreEvent>,
    profile_id: &ProfileId,
    backend: &Arc<dyn VpnBackend>,
    session_id: &SessionId,
    observation: &mut crate::dns::DnsObservation,
    stream: &str,
    line: &str,
) {
    let entry = sanitize_log(stream, line);
    let signal = classify_signal(line);
    {
        let mut state = state.lock();
        state.logs.push_back(entry.clone());
        while state.logs.len() > 500 {
            state.logs.pop_front();
        }

        match signal {
            ParsedLogSignal::Connected => state.snapshot.state = ConnectionState::Connected,
            ParsedLogSignal::AuthFailed => {
                state.snapshot.state = ConnectionState::Error;
                state.snapshot.last_error = Some(UserFacingError {
                    code: "auth_failed".into(),
                    title: "Authentication failed".into(),
                    message: "OpenVPN reported an authentication failure.".into(),
                    suggested_fix: Some("Re-enter your username and password.".into()),
                    details_safe: None,
                });
                let _ = backend.disconnect(session_id.clone());
            }
            ParsedLogSignal::RetryableFailure => {
                state.snapshot.state = ConnectionState::Reconnecting;
            }
            ParsedLogSignal::DnsHint => {
                observation.runtime_pushed.push(line.to_string());
                state.snapshot.dns_observation = observation.clone();
                let _ = events.send(CoreEvent::DnsObserved(observation.clone()));
            }
            ParsedLogSignal::None => {}
        }

        state.snapshot.profile_id = Some(profile_id.clone());
        let _ = events.send(CoreEvent::StateChanged(state.snapshot.clone()));
    }
    let _ = events.send(CoreEvent::LogLine(entry));
}
