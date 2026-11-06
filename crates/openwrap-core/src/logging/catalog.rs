use std::fs;
use std::path::PathBuf;

use crate::errors::AppError;

use super::model::{SessionMetadata, SessionSummary};

pub fn get_recent_sessions(
    base_logs_dir: &PathBuf,
    limit: usize,
) -> Result<Vec<SessionSummary>, AppError> {
    let sessions_dir = base_logs_dir.join("sessions");
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for date_entry in fs::read_dir(&sessions_dir)? {
        let date_dir = date_entry?;
        if !date_dir.file_type()?.is_dir() {
            continue;
        }

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

    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    sessions.truncate(limit);
    Ok(sessions)
}
