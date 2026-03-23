use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::profiles::ProfileId;

use super::model::{SessionOutcome, SessionSummary};
use super::session_manager::SessionLogManager;

#[derive(Clone)]
pub struct SharedSessionLogManager {
    inner: Arc<Mutex<SessionLogManager>>,
}

impl SharedSessionLogManager {
    pub fn new(base_logs_dir: PathBuf, verbose: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SessionLogManager::new(base_logs_dir, verbose))),
        }
    }

    pub fn start_session(
        &self,
        session_id: &SessionId,
        profile_id: &ProfileId,
        profile_name: &str,
    ) -> Result<PathBuf, AppError> {
        self.inner
            .lock()
            .start_session(session_id, profile_id, profile_name)
    }

    pub fn end_session(&self, outcome: SessionOutcome) {
        self.inner.lock().end_session(outcome);
    }

    pub fn log_openvpn(&self, line: &str) {
        self.inner.lock().log_openvpn(line);
    }

    pub fn log_dns(&self, line: &str) {
        self.inner.lock().log_dns(line);
    }

    pub fn log_core(&self, event: &str) {
        self.inner.lock().log_core(event);
    }

    pub fn flush(&self) {
        self.inner.lock().flush();
    }

    pub fn current_session_dir(&self) -> Option<PathBuf> {
        self.inner.lock().current_session_dir().cloned()
    }

    pub fn set_verbose(&self, verbose: bool) {
        self.inner.lock().set_verbose(verbose);
    }

    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>, AppError> {
        let base_logs_dir = self.inner.lock().get_base_logs_dir().clone();
        crate::logging::catalog::get_recent_sessions(&base_logs_dir, limit)
    }

    pub fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u64, AppError> {
        let base_logs_dir = self.inner.lock().get_base_logs_dir().clone();
        crate::logging::catalog::cleanup_old_sessions(&base_logs_dir, max_age_days)
    }
}
