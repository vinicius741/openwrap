mod writer;

mod catalog;
mod model;
mod retention;
#[cfg(test)]
mod retention_tests;
mod session_manager;
mod shared;
#[cfg(test)]
mod tests;

pub use model::{SessionMetadata, SessionOutcome, SessionSummary};
pub use retention::{DEFAULT_MAX_AGE_DAYS, DEFAULT_MAX_COMPLETED_BYTES};
pub use session_manager::SessionLogManager;
pub use shared::SharedSessionLogManager;

use crate::errors::AppError;

impl SessionLogManager {
    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>, AppError> {
        catalog::get_recent_sessions(self.get_base_logs_dir(), limit)
    }

    pub fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u64, AppError> {
        retention::cleanup_sessions(
            self.get_base_logs_dir(),
            max_age_days,
            None,
            self.current_session_dir().map(|path| path.as_path()),
        )
    }
}
