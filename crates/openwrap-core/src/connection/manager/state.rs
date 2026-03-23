use std::collections::VecDeque;

use chrono::Utc;

use crate::connection::{ConnectionSnapshot, LogEntry, SessionId};
use crate::profiles::ProfileDetail;

const MAX_LOG_ENTRIES: usize = 500;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    StateChanged(ConnectionSnapshot),
    LogLine(LogEntry),
    CredentialsRequested(crate::connection::CredentialPrompt),
    DnsObserved(crate::dns::DnsObservation),
}

#[derive(Clone)]
pub(crate) struct PendingCredentials {
    pub(crate) profile: ProfileDetail,
}

#[derive(Clone)]
pub(crate) struct ConnectionPlan {
    pub(crate) detail: ProfileDetail,
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ActiveSession {
    pub(crate) session_id: SessionId,
    pub(crate) generation: u64,
    pub(crate) runtime_dir: std::path::PathBuf,
    pub(crate) auth_file: Option<std::path::PathBuf>,
    pub(crate) extra_cleanup_paths: Vec<std::path::PathBuf>,
}

pub(crate) struct ManagerState {
    pub(crate) snapshot: ConnectionSnapshot,
    pub(crate) logs: VecDeque<LogEntry>,
    pub(crate) pending_credentials: Option<PendingCredentials>,
    pub(crate) active_session: Option<ActiveSession>,
    pub(crate) reconnect_plan: Option<ConnectionPlan>,
    pub(crate) next_generation: u64,
    pub(crate) auto_promoted_policy_persisted: bool,
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
            auto_promoted_policy_persisted: false,
        }
    }
}

pub(crate) fn session_is_current(state: &ManagerState, active_session: &ActiveSession) -> bool {
    state.active_session.as_ref().is_some_and(|current| {
        current.session_id == active_session.session_id
            && current.generation == active_session.generation
    })
}

pub(crate) fn apply_terminal_error(
    snapshot: &mut ConnectionSnapshot,
    log_file_path: Option<String>,
    profile_id: &crate::profiles::ProfileId,
    error: crate::errors::UserFacingError,
) {
    use crate::connection::ConnectionState;
    snapshot.state = ConnectionState::Error;
    snapshot.profile_id = Some(profile_id.clone());
    snapshot.pid = None;
    snapshot.substate = None;
    snapshot.log_file_path = log_file_path;
    snapshot.last_error = Some(error);
}

pub(crate) fn clear_logs(state: &mut ManagerState) {
    state.logs.clear();
}

pub(crate) fn push_log(state: &mut ManagerState, entry: LogEntry) {
    state.logs.push_back(entry);
    while state.logs.len() > MAX_LOG_ENTRIES {
        state.logs.pop_front();
    }
}

pub(crate) fn with_logs<T, F>(state: &mut ManagerState, f: F) -> T
where
    F: FnOnce(&VecDeque<LogEntry>) -> T,
{
    f(&state.logs)
}

pub(crate) fn transition_snapshot_in_place(
    state: &mut ManagerState,
    intent: crate::connection::state_machine::ConnectionIntent,
    profile_id: Option<crate::profiles::ProfileId>,
) -> Result<(), crate::errors::AppError> {
    use crate::connection::state_machine::transition;
    state.snapshot.state = transition(state.snapshot.state.clone(), intent)?;
    state.snapshot.profile_id = profile_id;
    state.snapshot.last_error = None;
    state.snapshot.log_file_path = None;
    state.snapshot.substate = None;
    state.snapshot.pid = None;
    state.snapshot.started_at.get_or_insert_with(Utc::now);
    Ok(())
}
