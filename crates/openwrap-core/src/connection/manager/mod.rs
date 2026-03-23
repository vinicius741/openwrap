mod connect;
mod errors;
mod events;
mod runtime;
mod state;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::app_state::AppPaths;
use crate::connection::state_machine::ConnectionIntent;
use crate::connection::{ConnectionSnapshot, ConnectionState, CredentialPrompt, CredentialSubmission, LogEntry};
use crate::dns::{DnsObserver, PassiveDnsObserver};
use crate::errors::AppError;
use crate::logging::SharedSessionLogManager;
use crate::profiles::{CredentialMode, ProfileId};
use crate::{ProfileRepository, SecretStore, VpnBackend};

pub use state::CoreEvent;

use connect::start_connect_attempt;
use runtime::cleanup_auth_file;
use state::{transition_snapshot_in_place, ConnectionPlan, ManagerState, PendingCredentials};

pub struct ConnectionManager {
    pub(crate) paths: AppPaths,
    pub(crate) repository: Arc<dyn ProfileRepository>,
    pub(crate) secret_store: Arc<dyn SecretStore>,
    pub(crate) backend: Arc<dyn VpnBackend>,
    pub(crate) dns_observer: Arc<dyn DnsObserver>,
    pub(crate) events: broadcast::Sender<CoreEvent>,
    pub(crate) state: Arc<Mutex<ManagerState>>,
    pub(crate) session_log: SharedSessionLogManager,
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
            dns_observer: Arc::new(PassiveDnsObserver),
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

    pub async fn connect(&self, profile_id: String) -> Result<ConnectionSnapshot, AppError> {
        let profile_id = profile_id
            .parse::<ProfileId>()
            .map_err(|error| AppError::ConnectionState(error.to_string()))?;
        let detail = self.repository.get_profile(&profile_id)?;
        self.repository
            .set_last_selected_profile(Some(&profile_id))?;

        transition_snapshot_in_place(
            &mut self.state.lock(),
            ConnectionIntent::BeginConnect,
            Some(profile_id.clone()),
        )?;
        let _ = self.events.send(CoreEvent::StateChanged(self.snapshot()));

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
            transition_snapshot_in_place(
                &mut self.state.lock(),
                ConnectionIntent::NeedCredentials,
                Some(profile_id.clone()),
            )?;
            let _ = self.events.send(CoreEvent::StateChanged(self.snapshot()));
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
            transition_snapshot_in_place(
                &mut self.state.lock(),
                ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            let _ = self.events.send(CoreEvent::StateChanged(self.snapshot()));
            self.clear_pending_connect_state();
            cleanup_auth_file(&session);
            self.backend.disconnect(session.session_id.clone())?;
            return Ok(self.snapshot());
        }

        if self.snapshot().state != ConnectionState::Idle {
            transition_snapshot_in_place(
                &mut self.state.lock(),
                ConnectionIntent::BeginDisconnect,
                self.snapshot().profile_id.clone(),
            )?;
            let _ = self.events.send(CoreEvent::StateChanged(self.snapshot()));
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
            cleanup_auth_file(&session);
            let _ = self.backend.disconnect(session.session_id.clone());
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
        detail: crate::profiles::ProfileDetail,
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
            self.session_log.clone(),
            plan,
            is_retry,
        )
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
        state.auto_promoted_policy_persisted = false;
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
