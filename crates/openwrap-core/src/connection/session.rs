use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dns::DnsObservation;
use crate::errors::UserFacingError;
use crate::profiles::ProfileId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Idle,
    ValidatingProfile,
    AwaitingCredentials,
    PreparingRuntime,
    StartingProcess,
    Connecting,
    Connected,
    Reconnecting,
    Disconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSnapshot {
    pub profile_id: Option<ProfileId>,
    pub state: ConnectionState,
    pub substate: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub pid: Option<u32>,
    pub retry_count: u8,
    pub dns_observation: DnsObservation,
    pub log_file_path: Option<String>,
    pub last_error: Option<UserFacingError>,
}

impl Default for ConnectionSnapshot {
    fn default() -> Self {
        Self {
            profile_id: None,
            state: ConnectionState::Idle,
            substate: None,
            started_at: None,
            pid: None,
            retry_count: 0,
            dns_observation: DnsObservation::default(),
            log_file_path: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialPrompt {
    pub profile_id: ProfileId,
    pub remember_supported: bool,
    pub saved_username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSubmission {
    pub profile_id: ProfileId,
    pub username: String,
    pub password: String,
    pub remember_in_keychain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: DateTime<Utc>,
    pub stream: String,
    pub level: LogLevel,
    pub message: String,
    pub sanitized: bool,
    pub classification: String,
}

pub type LogBuffer = VecDeque<LogEntry>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::DnsEffectiveMode;

    #[test]
    fn session_id_generates_unique_ids() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn session_id_display_format() {
        let id = SessionId::new();
        let display = id.to_string();
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn session_id_default_is_non_nil() {
        // Default() should produce a valid (non-nil) UUID, not the nil UUID
        let id = SessionId::default();
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn session_id_new_is_non_nil() {
        let id = SessionId::new();
        assert_ne!(id.0, Uuid::nil());
    }

    #[test]
    fn connection_state_serialization() {
        let states = vec![
            ConnectionState::Idle,
            ConnectionState::ValidatingProfile,
            ConnectionState::AwaitingCredentials,
            ConnectionState::PreparingRuntime,
            ConnectionState::StartingProcess,
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Reconnecting,
            ConnectionState::Disconnecting,
            ConnectionState::Error,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let roundtrip: ConnectionState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, roundtrip);
        }
    }

    #[test]
    fn connection_snapshot_default() {
        let snapshot = ConnectionSnapshot::default();
        assert!(snapshot.profile_id.is_none());
        assert_eq!(snapshot.state, ConnectionState::Idle);
        assert!(snapshot.substate.is_none());
        assert!(snapshot.started_at.is_none());
        assert!(snapshot.pid.is_none());
        assert_eq!(snapshot.retry_count, 0);
        assert!(snapshot.dns_observation.effective_mode == DnsEffectiveMode::ObserveOnly);
        assert!(snapshot.log_file_path.is_none());
        assert!(snapshot.last_error.is_none());
    }

    #[test]
    fn credential_prompt_structure() {
        let profile_id = ProfileId::new();
        let prompt = CredentialPrompt {
            profile_id: profile_id.clone(),
            remember_supported: true,
            saved_username: Some("testuser".to_string()),
        };
        assert_eq!(prompt.profile_id, profile_id);
        assert!(prompt.remember_supported);
        assert_eq!(prompt.saved_username.as_deref(), Some("testuser"));
    }

    #[test]
    fn credential_submission_structure() {
        let profile_id = ProfileId::new();
        let submission = CredentialSubmission {
            profile_id: profile_id.clone(),
            username: "testuser".to_string(),
            password: "testpass".to_string(),
            remember_in_keychain: true,
        };
        assert_eq!(submission.profile_id, profile_id);
        assert_eq!(submission.username, "testuser");
        assert_eq!(submission.password, "testpass");
        assert!(submission.remember_in_keychain);
    }

    #[test]
    fn log_level_serialization() {
        let levels = vec![
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let roundtrip: LogLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, roundtrip);
        }
    }

    #[test]
    fn log_entry_structure() {
        let entry = LogEntry {
            ts: chrono::Utc::now(),
            stream: "stdout".to_string(),
            level: LogLevel::Info,
            message: "Test message".to_string(),
            sanitized: true,
            classification: "info".to_string(),
        };
        assert_eq!(entry.stream, "stdout");
        assert_eq!(entry.level, LogLevel::Info);
        assert!(entry.sanitized);
    }

    #[test]
    fn log_buffer_enqueue_dequeue() {
        let mut buffer: LogBuffer = LogBuffer::new();
        buffer.push_back(LogEntry {
            ts: chrono::Utc::now(),
            stream: "stdout".to_string(),
            level: LogLevel::Info,
            message: "Message 1".to_string(),
            sanitized: true,
            classification: "info".to_string(),
        });
        buffer.push_back(LogEntry {
            ts: chrono::Utc::now(),
            stream: "stderr".to_string(),
            level: LogLevel::Error,
            message: "Message 2".to_string(),
            sanitized: true,
            classification: "error".to_string(),
        });
        assert_eq!(buffer.len(), 2);
        let first = buffer.pop_front().unwrap();
        assert_eq!(first.message, "Message 1");
        assert_eq!(first.level, LogLevel::Info);
        let second = buffer.pop_front().unwrap();
        assert_eq!(second.message, "Message 2");
        assert_eq!(second.level, LogLevel::Error);
    }
}
