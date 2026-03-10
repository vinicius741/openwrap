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
