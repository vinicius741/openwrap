use chrono::Utc;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::{ConnectionSnapshot, ConnectionState, LogEntry, SessionId};
use crate::logging::SharedSessionLogManager;
use crate::profiles::{ProfileDetail, ProfileId};
use crate::secrets::StoredSecretKind;
use crate::{ProfileRepository, SecretStore, VpnBackend};

use super::credentials::{generate_password, PlanCredentials};

pub const MAX_LOG_ENTRIES: usize = 500;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    StateChanged(ConnectionSnapshot),
    LogLine(LogEntry),
    CredentialsRequested(crate::connection::CredentialPrompt),
    DnsObserved(crate::dns::DnsObservation),
}

#[derive(Clone)]
pub struct PendingCredentials {
    pub profile: ProfileDetail,
}

#[derive(Clone)]
pub struct ActiveSession {
    pub session_id: SessionId,
    pub generation: u64,
    pub runtime_dir: std::path::PathBuf,
    pub auth_file: Option<std::path::PathBuf>,
    pub extra_cleanup_paths: Vec<std::path::PathBuf>,
}

pub struct ManagerState {
    pub snapshot: ConnectionSnapshot,
    pub logs: VecDeque<LogEntry>,
    pub pending_credentials: Option<PendingCredentials>,
    pub active_session: Option<ActiveSession>,
    pub reconnect_plan: Option<ConnectionPlan>,
    pub next_generation: u64,
    /// Generation of the attempt that is preparing or spawning OpenVPN and has
    /// no `active_session` yet. Disconnect clears it, which tells the attempt
    /// to stop the process it just started instead of adopting it.
    pub pending_attempt: Option<u64>,
    /// TOTP step of the last generated password sent to the server.
    pub last_otp_step: Option<u64>,
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
            pending_attempt: None,
            last_otp_step: None,
        }
    }
}

#[derive(Clone)]
pub struct ConnectionPlan {
    pub detail: ProfileDetail,
    pub credentials: PlanCredentials,
}

/// What `connect` needs before it can start an attempt.
enum ConnectCredentials {
    Ready(PlanCredentials),
    Prompt { saved_username: Option<String> },
}

pub struct ConnectionManager {
    pub(super) paths: AppPaths,
    pub(super) repository: Arc<dyn ProfileRepository>,
    pub(super) secret_store: Arc<dyn SecretStore>,
    pub(super) backend: Arc<dyn VpnBackend>,
    pub(super) dns_observer: Arc<dyn crate::dns::DnsObserver>,
    pub(super) events: broadcast::Sender<CoreEvent>,
    pub(super) state: Arc<Mutex<ManagerState>>,
    pub(super) session_log: SharedSessionLogManager,
}

impl ConnectionManager {
    pub fn new(
        paths: AppPaths,
        repository: Arc<dyn ProfileRepository>,
        secret_store: Arc<dyn SecretStore>,
        backend: Arc<dyn VpnBackend>,
    ) -> Self {
        let (events, _) = broadcast::channel(256);
        let settings = repository.get_settings().ok();
        let verbose = settings
            .as_ref()
            .map(|s| s.verbose_logging)
            .unwrap_or(false);
        let session_log = SharedSessionLogManager::new(paths.logs_dir.clone(), verbose);
        Self {
            paths,
            repository,
            secret_store,
            backend,
            dns_observer: Arc::new(crate::dns::PassiveDnsObserver),
            events,
            state: Arc::new(Mutex::new(ManagerState::default())),
            session_log,
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

    pub fn session_log(&self) -> &SharedSessionLogManager {
        &self.session_log
    }

    pub fn set_verbose_logging(&self, verbose: bool) {
        self.session_log.set_verbose(verbose);
    }

    pub async fn connect(
        &self,
        profile_id: String,
    ) -> Result<ConnectionSnapshot, crate::errors::AppError> {
        let profile_id = profile_id
            .parse::<ProfileId>()
            .map_err(|error| crate::errors::AppError::ConnectionState(error.to_string()))?;
        let detail = self.repository.get_profile(&profile_id)?;
        self.repository
            .set_last_selected_profile(Some(&profile_id))?;

        self.transition_snapshot(
            crate::connection::state_machine::ConnectionIntent::BeginConnect,
            Some(profile_id.clone()),
        )?;

        // Any failure from here on must leave a terminal state; otherwise the
        // snapshot stays in ValidatingProfile, where Connect is an invalid
        // transition.
        let credentials = match self.resolve_connect_credentials(&detail, &profile_id) {
            Ok(credentials) => credentials,
            Err(error) => {
                self.fail_if_in(&profile_id, ConnectionState::ValidatingProfile, &error);
                return Err(error);
            }
        };

        match credentials {
            ConnectCredentials::Ready(credentials) => {
                self.start_connect(detail, credentials, false).await
            }
            ConnectCredentials::Prompt { saved_username } => {
                {
                    let mut state = self.state.lock();
                    state.pending_credentials = Some(PendingCredentials {
                        profile: detail.clone(),
                    });
                }
                self.transition_snapshot(
                    crate::connection::state_machine::ConnectionIntent::NeedCredentials,
                    Some(profile_id.clone()),
                )?;
                let prompt = crate::connection::CredentialPrompt {
                    profile_id,
                    remember_supported: true,
                    saved_username,
                };
                let _ = self.events.send(CoreEvent::CredentialsRequested(prompt));
                Ok(self.snapshot())
            }
        }
    }

    fn resolve_connect_credentials(
        &self,
        detail: &ProfileDetail,
        profile_id: &ProfileId,
    ) -> Result<ConnectCredentials, crate::errors::AppError> {
        if detail.profile.credential_mode != crate::profiles::CredentialMode::UserPass {
            return Ok(ConnectCredentials::Ready(PlanCredentials::None));
        }

        let stored_secret = self.secret_store.get_password(profile_id)?;

        if detail.profile.credential_strategy == crate::profiles::CredentialStrategy::PinTotp {
            let configured = stored_secret.ok_or_else(|| {
                crate::errors::AppError::Settings(
                    "This profile is set to generate its VPN password automatically, but no local generated-password configuration is saved yet.".into(),
                )
            })?;
            // Validate the saved PIN and TOTP secret now so a broken setup
            // fails before OpenVPN starts. Each attempt generates its own code.
            generate_password(&configured, Utc::now())?;
            return Ok(ConnectCredentials::Ready(PlanCredentials::Generated(
                configured,
            )));
        }

        let saved_username = stored_secret.and_then(|secret| {
            (secret.kind == StoredSecretKind::UsernameOnly).then_some(secret.username)
        });
        Ok(ConnectCredentials::Prompt { saved_username })
    }

    /// Puts the snapshot into `Error`, but only while it is still `expected`,
    /// so a failure never overwrites a disconnect that happened meanwhile.
    fn fail_if_in(
        &self,
        profile_id: &ProfileId,
        expected: ConnectionState,
        error: &crate::errors::AppError,
    ) {
        let mut state = self.state.lock();
        if state.snapshot.state == expected {
            super::connect::terminal_error_locked(
                &self.paths,
                &mut state,
                &self.events,
                profile_id,
                error,
            );
        }
    }

    pub async fn submit_credentials(
        &self,
        submission: crate::connection::CredentialSubmission,
    ) -> Result<ConnectionSnapshot, crate::errors::AppError> {
        let profile = {
            let mut state = self.state.lock();
            let pending = state.pending_credentials.take().ok_or_else(|| {
                crate::errors::AppError::ConnectionState("no pending credential prompt".into())
            })?;
            pending.profile
        };

        if let Err(error) = self.remember_username(&submission) {
            self.fail_if_in(
                &submission.profile_id,
                ConnectionState::AwaitingCredentials,
                &error,
            );
            return Err(error);
        }

        self.start_connect(
            profile,
            PlanCredentials::Static {
                username: submission.username,
                password: submission.password,
            },
            false,
        )
        .await
    }

    fn remember_username(
        &self,
        submission: &crate::connection::CredentialSubmission,
    ) -> Result<(), crate::errors::AppError> {
        if submission.remember_in_keychain {
            self.secret_store
                .set_password(crate::secrets::StoredSecret::username_only(
                    submission.profile_id.clone(),
                    submission.username.clone(),
                ))?;
            self.repository
                .update_has_saved_credentials(&submission.profile_id, true)?;
        } else {
            self.secret_store.delete_password(&submission.profile_id)?;
            self.repository
                .update_has_saved_credentials(&submission.profile_id, false)?;
        }
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<ConnectionSnapshot, crate::errors::AppError> {
        let session = {
            let state = self.state.lock();
            state.active_session.clone()
        };

        if let Some(session) = session {
            self.transition_snapshot(
                crate::connection::state_machine::ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            self.clear_pending_connect_state();
            crate::connection::manager::cleanup_auth_file(&session);
            self.backend.disconnect(session.session_id.clone())?;
            return Ok(self.snapshot());
        }

        if self.snapshot().state != ConnectionState::Idle {
            self.transition_snapshot(
                crate::connection::state_machine::ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            self.finish_disconnect();
        }

        Ok(self.snapshot())
    }

    pub fn shutdown(&self) -> Result<(), crate::errors::AppError> {
        let session = {
            let mut state = self.state.lock();
            state.pending_credentials = None;
            state.reconnect_plan = None;
            state.pending_attempt = None;
            state.active_session.clone()
        };

        let should_reconcile =
            session.is_some() || self.paths.runtime_dir.join("dns-state").exists();

        if let Some(session) = session {
            crate::connection::manager::cleanup_auth_file(&session);
            let _ = self
                .backend
                .disconnect_on_shutdown(session.session_id.clone());
            crate::connection::manager::cleanup_runtime_artifacts(&session);
        }

        let reconcile_result = if should_reconcile {
            let result = self
                .backend
                .reconcile_dns(crate::openvpn::ReconcileDnsRequest {
                    runtime_root: self.paths.runtime_dir.clone(),
                });
            if let Err(error) = &result {
                self.session_log
                    .log_dns(&format!("DNS shutdown reconciliation failed: {error}"));
            } else {
                self.session_log
                    .log_dns("DNS shutdown reconciliation succeeded");
            }
            result
        } else {
            Ok(())
        };

        self.finish_disconnect();
        reconcile_result
    }

    pub async fn disconnect_if_connected(
        &self,
        profile_id: &ProfileId,
    ) -> Result<(), crate::errors::AppError> {
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

    pub(super) async fn start_connect(
        &self,
        detail: ProfileDetail,
        credentials: PlanCredentials,
        is_retry: bool,
    ) -> Result<ConnectionSnapshot, crate::errors::AppError> {
        let plan = ConnectionPlan {
            detail,
            credentials,
        };
        crate::connection::manager::connect::start_connect_attempt(
            self.paths.clone(),
            self.repository.clone(),
            self.backend.clone(),
            self.dns_observer.clone(),
            self.events.clone(),
            self.state.clone(),
            self.session_log.clone(),
            plan,
            is_retry,
        )
    }

    pub(super) fn transition_snapshot(
        &self,
        intent: crate::connection::state_machine::ConnectionIntent,
        profile_id: Option<ProfileId>,
    ) -> Result<(), crate::errors::AppError> {
        let mut state = self.state.lock();
        state.snapshot.state =
            crate::connection::state_machine::transition(state.snapshot.state.clone(), intent)?;
        state.snapshot.profile_id = profile_id;
        state.snapshot.last_error = None;
        state.snapshot.log_file_path = None;
        state.snapshot.substate = None;
        state.snapshot.pid = None;
        state.snapshot.started_at.get_or_insert_with(Utc::now);
        let _ = self
            .events
            .send(CoreEvent::StateChanged(state.snapshot.clone()));
        Ok(())
    }

    pub(super) fn clear_pending_connect_state(&self) {
        let mut state = self.state.lock();
        state.pending_credentials = None;
        state.reconnect_plan = None;
        state.pending_attempt = None;
    }

    pub(super) fn finish_disconnect(&self) {
        let mut state = self.state.lock();
        state.pending_credentials = None;
        state.reconnect_plan = None;
        state.pending_attempt = None;
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
        // Drop may run on the main thread during app termination (no Tokio
        // runtime entered) and inside `extern "C"`/nounwind boundaries, where
        // a panic would abort the process. Contain any failure so teardown
        // always completes.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.shutdown()));
    }
}

pub(super) fn session_is_current(state: &ManagerState, active_session: &ActiveSession) -> bool {
    state.active_session.as_ref().is_some_and(|current| {
        current.session_id == active_session.session_id
            && current.generation == active_session.generation
    })
}
