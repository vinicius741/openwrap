mod writer;

mod catalog;
mod model;
mod session_manager;
mod shared;
#[cfg(test)]
mod tests;

pub use model::{SessionMetadata, SessionOutcome, SessionSummary};
pub use session_manager::SessionLogManager;
pub use shared::SharedSessionLogManager;

use crate::errors::AppError;

impl SessionLogManager {
    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>, AppError> {
        catalog::get_recent_sessions(self.get_base_logs_dir(), limit)
    }

    pub fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u64, AppError> {
        catalog::cleanup_old_sessions(self.get_base_logs_dir(), max_age_days)
    }
}
