//! Session-based logging system for debugging connection issues.
//!
//! This module provides structured logging that persists to disk, organized by
//! session and date. It's designed to help debug DNS and connection issues
//! by capturing detailed logs that survive app restarts.
//!
//! # Module Structure
//!
//! - `model` - Domain models (SessionId, SessionMetadata, etc.)
//! - `writer` - Low-level buffered file sink
//! - `session_manager` - Active session lifecycle management
//! - `catalog` - Session discovery and retention cleanup
//! - `shared` - Thread-safe wrapper (SharedSessionLogManager)

mod catalog;
mod model;
mod session_manager;
mod shared;
mod writer;

// #[cfg(test)]
// mod tests;

// Re-export public API
pub use catalog::{cleanup_old_sessions, get_recent_sessions};
pub use model::{SessionMetadata, SessionOutcome, SessionSummary};
pub use session_manager::SessionLogManager;
pub use shared::SharedSessionLogManager;
