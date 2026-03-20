//! Session-based logging system for debugging connection issues.
//!
//! This module provides structured logging that persists to disk, organized by
//! session and date. It's designed to help debug DNS and connection issues
//! by capturing detailed logs that survive app restarts.

mod writer;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::profiles::ProfileId;

use writer::BufferedWriter;

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

/// Active session log with buffered writers for each log type.
struct ActiveSessionLog {
    log_dir: PathBuf,
    openvpn_writer: BufferedWriter,
    dns_writer: BufferedWriter,
    core_writer: BufferedWriter,
    verbose: bool,
}

impl ActiveSessionLog {
    fn new(log_dir: PathBuf, verbose: bool) -> Result<Self, AppError> {
        fs::create_dir_all(&log_dir)?;

        let openvpn_writer = BufferedWriter::new(log_dir.join("openvpn.log"), verbose)?;
        let dns_writer = BufferedWriter::new(log_dir.join("dns.log"), verbose)?;
        let core_writer = BufferedWriter::new(log_dir.join("core.log"), verbose)?;

        Ok(Self {
            log_dir,
            openvpn_writer,
            dns_writer,
            core_writer,
            verbose,
        })
    }

    fn log_openvpn(&mut self, line: &str) {
        if let Err(e) = self.openvpn_writer.write_line(line) {
            eprintln!("[logging] Failed to write openvpn log: {}", e);
        }
    }

    fn log_dns(&mut self, line: &str) {
        if let Err(e) = self.dns_writer.write_line(line) {
            eprintln!("[logging] Failed to write dns log: {}", e);
        }
    }

    fn log_core(&mut self, line: &str) {
        if let Err(e) = self.core_writer.write_line(line) {
            eprintln!("[logging] Failed to write core log: {}", e);
        }
    }

    fn flush(&mut self) {
        let _ = self.openvpn_writer.flush();
        let _ = self.dns_writer.flush();
        let _ = self.core_writer.flush();
    }

    fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        self.openvpn_writer.set_immediate_flush(verbose);
        self.dns_writer.set_immediate_flush(verbose);
        self.core_writer.set_immediate_flush(verbose);
    }
}

/// Manages session-based logging to disk.
pub struct SessionLogManager {
    base_logs_dir: PathBuf,
    current_session: Option<ActiveSessionLog>,
    verbose: bool,
}

impl SessionLogManager {
    /// Create a new SessionLogManager.
    pub fn new(base_logs_dir: PathBuf, verbose: bool) -> Self {
        Self {
            base_logs_dir,
            current_session: None,
            verbose,
        }
    }

    /// Start a new session log.
    pub fn start_session(
        &mut self,
        session_id: &SessionId,
        profile_id: &ProfileId,
        profile_name: &str,
    ) -> Result<PathBuf, AppError> {
        // End any existing session first
        self.end_session(SessionOutcome::Cancelled);

        // Capture start time once for consistency between path and metadata
        let started_at = Utc::now();
        let date = started_at.format("%Y-%m-%d").to_string();
        let log_dir = self
            .base_logs_dir
            .join("sessions")
            .join(&date)
            .join(format!("session-{}", session_id));

        // Create the session log
        let mut session_log = ActiveSessionLog::new(log_dir.clone(), self.verbose)?;

        // Write metadata
        let metadata = SessionMetadata {
            session_id: session_id.to_string(),
            profile_id: profile_id.to_string(),
            profile_name: profile_name.to_string(),
            started_at,
            ended_at: None,
            outcome: SessionOutcome::InProgress,
            verbose_mode: self.verbose,
        };

        let metadata_path = log_dir.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        if let Err(e) = fs::write(&metadata_path, &metadata_json) {
            eprintln!("[logging] Failed to write metadata: {}", e);
        }

        // Log session start
        session_log.log_core(&format!(
            "[{}] Session started for profile '{}' (verbose={})",
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            profile_name,
            self.verbose
        ));

        self.current_session = Some(session_log);
        Ok(log_dir)
    }

    /// End the current session with the given outcome.
    pub fn end_session(&mut self, outcome: SessionOutcome) {
        if let Some(mut session) = self.current_session.take() {
            // Log session end BEFORE flushing to ensure it's captured
            session.log_core(&format!(
                "Session ended with outcome: {:?}",
                outcome
            ));

            // Update metadata with outcome
            let metadata_path = session.log_dir.join("metadata.json");
            if let Ok(content) = fs::read_to_string(&metadata_path) {
                if let Ok(mut metadata) = serde_json::from_str::<SessionMetadata>(&content) {
                    metadata.ended_at = Some(Utc::now());
                    metadata.outcome = outcome;

                    if let Ok(json) = serde_json::to_string_pretty(&metadata) {
                        if let Err(e) = fs::write(&metadata_path, &json) {
                            eprintln!("[logging] Failed to update metadata: {}", e);
                        }
                    }
                }
            }

            // Final flush to ensure all logs are persisted
            session.flush();
        }
    }

    /// Log an OpenVPN output line.
    pub fn log_openvpn(&mut self, line: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_openvpn(line);
        }
    }

    /// Log a DNS-specific output line.
    pub fn log_dns(&mut self, line: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_dns(line);
        }
    }

    /// Log a core event (state transition, etc.).
    pub fn log_core(&mut self, event: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_core(&format!(
                "[{}] {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                event
            ));
        }
    }

    /// Flush all pending writes.
    pub fn flush(&mut self) {
        if let Some(session) = &mut self.current_session {
            session.flush();
        }
    }

    /// Get the current session's log directory, if any.
    pub fn current_session_dir(&self) -> Option<&PathBuf> {
        self.current_session.as_ref().map(|s| &s.log_dir)
    }

    /// Update verbose mode setting.
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        if let Some(session) = &mut self.current_session {
            session.set_verbose(verbose);
        }
    }

    /// Get recent sessions, sorted by start time (most recent first).
    pub fn get_recent_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>, AppError> {
        let sessions_dir = self.base_logs_dir.join("sessions");
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        // Iterate through date directories
        for date_entry in fs::read_dir(&sessions_dir)? {
            let date_dir = date_entry?;
            if !date_dir.file_type()?.is_dir() {
                continue;
            }

            // Iterate through session directories
            for session_entry in fs::read_dir(date_dir.path())? {
                let session_dir = session_entry?;
                if !session_dir.file_type()?.is_dir() {
                    continue;
                }

                let metadata_path = session_dir.path().join("metadata.json");
                if let Ok(content) = fs::read_to_string(&metadata_path) {
                    if let Ok(metadata) = serde_json::from_str::<SessionMetadata>(&content) {
                        sessions.push(SessionSummary {
                            session_id: metadata.session_id,
                            profile_name: metadata.profile_name,
                            started_at: metadata.started_at,
                            ended_at: metadata.ended_at,
                            outcome: metadata.outcome,
                            log_dir: session_dir.path(),
                        });
                    }
                }
            }
        }

        // Sort by start time, most recent first
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        // Limit results
        sessions.truncate(limit);
        Ok(sessions)
    }

    /// Clean up sessions older than the specified number of days.
    pub fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u64, AppError> {
        let sessions_dir = self.base_logs_dir.join("sessions");
        if !sessions_dir.exists() {
            return Ok(0);
        }

        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let mut removed_count = 0;

        for date_entry in fs::read_dir(&sessions_dir)? {
            let date_dir = date_entry?;
            if !date_dir.file_type()?.is_dir() {
                continue;
            }

            // Check if the directory name is a date older than cutoff
            if let Some(dir_name) = date_dir.file_name().to_str() {
                if let Ok(dir_date) = chrono::NaiveDate::parse_from_str(dir_name, "%Y-%m-%d") {
                    let dir_datetime = dir_date.and_hms_opt(0, 0, 0).unwrap();
                    let dir_utc: DateTime<Utc> = DateTime::from_naive_utc_and_offset(dir_datetime, Utc);

                    if dir_utc < cutoff {
                        if let Err(e) = fs::remove_dir_all(date_dir.path()) {
                            eprintln!("[logging] Failed to remove old session dir: {}", e);
                        } else {
                            removed_count += 1;
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}

/// Thread-safe wrapper for SessionLogManager.
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
        self.inner.lock().start_session(session_id, profile_id, profile_name)
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
        self.inner.lock().get_recent_sessions(limit)
    }

    pub fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u64, AppError> {
        self.inner.lock().cleanup_old_sessions(max_age_days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn session_outcome_serialization() {
        let outcomes = vec![
            SessionOutcome::Success,
            SessionOutcome::Failed,
            SessionOutcome::Cancelled,
            SessionOutcome::InProgress,
        ];
        for outcome in outcomes {
            let json = serde_json::to_string(&outcome).unwrap();
            let roundtrip: SessionOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(outcome, roundtrip);
        }
    }

    #[test]
    fn session_metadata_serialization() {
        let metadata = SessionMetadata {
            session_id: "test-session-id".to_string(),
            profile_id: "test-profile-id".to_string(),
            profile_name: "Test Profile".to_string(),
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            outcome: SessionOutcome::Success,
            verbose_mode: true,
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let roundtrip: SessionMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata.session_id, roundtrip.session_id);
        assert_eq!(metadata.profile_name, roundtrip.profile_name);
        assert_eq!(metadata.outcome, roundtrip.outcome);
        assert!(roundtrip.verbose_mode);
    }

    #[test]
    fn session_log_manager_start_and_end_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        let session_id = SessionId::new();
        let profile_id = ProfileId::new();

        let log_dir = manager.start_session(&session_id, &profile_id, "Test Profile").unwrap();

        // Check that directory was created
        assert!(log_dir.exists());
        assert!(log_dir.join("metadata.json").exists());

        // Check metadata
        let metadata_content = fs::read_to_string(log_dir.join("metadata.json")).unwrap();
        let metadata: SessionMetadata = serde_json::from_str(&metadata_content).unwrap();
        assert_eq!(metadata.profile_name, "Test Profile");
        assert_eq!(metadata.outcome, SessionOutcome::InProgress);
        assert!(metadata.ended_at.is_none());

        // End session
        manager.end_session(SessionOutcome::Success);

        // Check updated metadata
        let metadata_content = fs::read_to_string(log_dir.join("metadata.json")).unwrap();
        let metadata: SessionMetadata = serde_json::from_str(&metadata_content).unwrap();
        assert_eq!(metadata.outcome, SessionOutcome::Success);
        assert!(metadata.ended_at.is_some());
    }

    #[test]
    fn session_log_manager_logs_to_files() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        let session_id = SessionId::new();
        let profile_id = ProfileId::new();

        manager.start_session(&session_id, &profile_id, "Test Profile").unwrap();

        manager.log_openvpn("OpenVPN output line");
        manager.log_dns("DNS debug output");
        manager.log_core("Core event: state changed");

        manager.end_session(SessionOutcome::Success);

        let log_dir = temp_dir.path()
            .join("sessions")
            .join(Utc::now().format("%Y-%m-%d").to_string())
            .join(format!("session-{}", session_id));

        // Check log files were created and contain content
        let openvpn_log = fs::read_to_string(log_dir.join("openvpn.log")).unwrap();
        assert!(openvpn_log.contains("OpenVPN output line"));

        let dns_log = fs::read_to_string(log_dir.join("dns.log")).unwrap();
        assert!(dns_log.contains("DNS debug output"));

        let core_log = fs::read_to_string(log_dir.join("core.log")).unwrap();
        assert!(core_log.contains("Core event: state changed"));
    }

    #[test]
    fn shared_session_log_manager_is_cloneable() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SharedSessionLogManager::new(temp_dir.path().to_path_buf(), false);
        let manager2 = manager.clone();

        let session_id = SessionId::new();
        let profile_id = ProfileId::new();

        manager.start_session(&session_id, &profile_id, "Test Profile").unwrap();
        manager2.log_core("Logged from clone");
        manager2.end_session(SessionOutcome::Success);
    }

    #[test]
    fn get_recent_sessions_returns_multiple_sessions_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        // Create multiple sessions with a small delay to ensure different timestamps
        let session_id1 = SessionId::new();
        let profile_id1 = ProfileId::new();
        manager.start_session(&session_id1, &profile_id1, "Profile 1").unwrap();
        manager.end_session(SessionOutcome::Success);

        std::thread::sleep(std::time::Duration::from_millis(10));

        let session_id2 = SessionId::new();
        let profile_id2 = ProfileId::new();
        manager.start_session(&session_id2, &profile_id2, "Profile 2").unwrap();
        manager.end_session(SessionOutcome::Failed);

        std::thread::sleep(std::time::Duration::from_millis(10));

        let session_id3 = SessionId::new();
        let profile_id3 = ProfileId::new();
        manager.start_session(&session_id3, &profile_id3, "Profile 3").unwrap();
        manager.end_session(SessionOutcome::Success);

        // Get recent sessions
        let sessions = manager.get_recent_sessions(10).unwrap();
        assert_eq!(sessions.len(), 3);

        // Should be sorted most recent first
        assert_eq!(sessions[0].profile_name, "Profile 3");
        assert_eq!(sessions[1].profile_name, "Profile 2");
        assert_eq!(sessions[2].profile_name, "Profile 1");

        // Test limit
        let limited = manager.get_recent_sessions(2).unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].profile_name, "Profile 3");
        assert_eq!(limited[1].profile_name, "Profile 2");
    }

    #[test]
    fn get_recent_sessions_returns_empty_when_no_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        let sessions = manager.get_recent_sessions(10).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn cleanup_old_sessions_removes_old_directories() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        // Create a session directory with an old date
        let old_date = Utc::now() - chrono::Duration::days(35);
        let old_date_str = old_date.format("%Y-%m-%d").to_string();
        let old_session_dir = temp_dir.path()
            .join("sessions")
            .join(&old_date_str)
            .join("session-old");
        fs::create_dir_all(&old_session_dir).unwrap();

        // Create metadata for the old session
        let old_metadata = SessionMetadata {
            session_id: "old-session".to_string(),
            profile_id: "old-profile".to_string(),
            profile_name: "Old Profile".to_string(),
            started_at: old_date,
            ended_at: Some(old_date),
            outcome: SessionOutcome::Success,
            verbose_mode: false,
        };
        fs::write(
            old_session_dir.join("metadata.json"),
            serde_json::to_string_pretty(&old_metadata).unwrap(),
        ).unwrap();

        // Create a recent session through normal means
        let recent_session_id = SessionId::new();
        let recent_profile_id = ProfileId::new();
        manager.start_session(&recent_session_id, &recent_profile_id, "Recent Profile").unwrap();
        manager.end_session(SessionOutcome::Success);

        // Cleanup sessions older than 30 days
        let removed = manager.cleanup_old_sessions(30).unwrap();
        assert_eq!(removed, 1);

        // Verify old session is gone
        assert!(!old_session_dir.exists());

        // Verify recent session still exists
        let sessions = manager.get_recent_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].profile_name, "Recent Profile");
    }

    #[test]
    fn cleanup_old_sessions_handles_malformed_directory_names() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

        // Create a directory with an invalid date format (should be skipped)
        let malformed_dir = temp_dir.path()
            .join("sessions")
            .join("not-a-date")
            .join("session-test");
        fs::create_dir_all(&malformed_dir).unwrap();

        // Cleanup should not panic and should not remove malformed directories
        let removed = manager.cleanup_old_sessions(30).unwrap();
        assert_eq!(removed, 0);

        // Malformed directory should still exist (we don't delete what we can't parse)
        assert!(malformed_dir.exists());
    }
}
