pub mod backoff;
pub mod log_parser;
pub mod manager;
pub mod session;
pub mod state_machine;

pub use manager::{ConnectionManager, CoreEvent};
pub use session::{
    ConnectionSnapshot, ConnectionState, CredentialPrompt, CredentialSubmission, LogEntry,
    LogLevel, SessionId,
};

