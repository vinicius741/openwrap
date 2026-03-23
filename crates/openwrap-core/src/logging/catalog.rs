use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

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

pub fn cleanup_old_sessions(base_logs_dir: &PathBuf, max_age_days: u32) -> Result<u64, AppError> {
    let sessions_dir = base_logs_dir.join("sessions");
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
