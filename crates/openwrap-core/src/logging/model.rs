//! Domain models for session logging.
//!
//! This module contains the data structures used across the logging system.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Outcome of a connection session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    /// Session completed successfully (connected then disconnected cleanly).
    Success,
    /// Session failed with an error.
    Failed,
    /// Session was cancelled by user.
    Cancelled,
    /// Session is still in progress.
    InProgress,
}

/// Metadata about a session, stored in metadata.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub outcome: SessionOutcome,
    pub verbose_mode: bool,
}

/// Summary of a session for listing purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub profile_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub outcome: SessionOutcome,
    pub log_dir: PathBuf,
}
