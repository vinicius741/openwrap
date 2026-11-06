use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, Utc};

use crate::errors::AppError;

use super::model::{SessionMetadata, SessionOutcome};

pub const DEFAULT_MAX_AGE_DAYS: u32 = 30;
pub const DEFAULT_MAX_COMPLETED_BYTES: u64 = 100 * 1024 * 1024;

struct CompletedSession {
    path: PathBuf,
    started_at: chrono::DateTime<Utc>,
    bytes: u64,
}

/// Remove expired sessions, then the oldest completed sessions over the budget.
/// The active directory is always protected. Today's sessions are never age-expired.
/// The sessions root may be a symlink; links inside it are left untouched.
/// Unreadable entries are skipped so other sessions can still be pruned.
pub(super) fn cleanup_sessions(
    base_logs_dir: &Path,
    max_age_days: u32,
    max_completed_bytes: Option<u64>,
    active_dir: Option<&Path>,
) -> Result<u64, AppError> {
    let sessions_dir = base_logs_dir.join("sessions");
    match fs::metadata(&sessions_dir) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
        Ok(metadata) if !metadata.file_type().is_dir() => return Ok(0),
        Ok(_) => {}
    }

    let cutoff = Utc::now()
        .date_naive()
        .checked_sub_signed(chrono::Duration::days(i64::from(max_age_days)))
        .unwrap_or(NaiveDate::MIN);
    let mut removed = 0;
    let mut completed = Vec::new();
    let mut total_bytes: u64 = 0;

    let date_entries = match fs::read_dir(&sessions_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    for date_entry in date_entries {
        let Some(date_entry) = directory_entry(date_entry, &sessions_dir) else {
            continue;
        };
        let Ok(date) =
            NaiveDate::parse_from_str(&date_entry.file_name().to_string_lossy(), "%Y-%m-%d")
        else {
            continue;
        };
        let date_dir = date_entry.path();
        let session_entries = match fs::read_dir(&date_dir) {
            Ok(entries) => entries,
            Err(error) => {
                report_scan_error(&date_dir, &error);
                continue;
            }
        };
        for session_entry in session_entries {
            let Some(session_entry) = directory_entry(session_entry, &date_dir) else {
                continue;
            };
            if !session_entry
                .file_name()
                .to_string_lossy()
                .starts_with("session-")
            {
                continue;
            }
            let path = session_entry.path();
            if active_dir == Some(path.as_path()) {
                continue;
            }
            if date < cutoff {
                removed += u64::from(remove_session(&path));
                continue;
            }
            if max_completed_bytes.is_none() {
                continue;
            }
            // New sessions created during this scan cannot become size-pruning
            // candidates while they are still being written.
            let metadata = fs::read_to_string(path.join("metadata.json"))
                .ok()
                .and_then(|json| serde_json::from_str::<SessionMetadata>(&json).ok());
            if let Some(metadata) = metadata.filter(|metadata| {
                metadata.ended_at.is_some() && metadata.outcome != SessionOutcome::InProgress
            }) {
                let bytes = match directory_bytes(&path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        report_scan_error(&path, &error);
                        continue;
                    }
                };
                total_bytes = total_bytes.saturating_add(bytes);
                completed.push(CompletedSession {
                    path,
                    started_at: metadata.started_at,
                    bytes,
                });
            }
        }
        // This fails harmlessly if active sessions or unrelated entries remain.
        let _ = fs::remove_dir(&date_dir);
    }

    if let Some(max_bytes) = max_completed_bytes {
        completed.sort_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| a.path.cmp(&b.path))
        });
        for session in completed {
            if total_bytes <= max_bytes {
                break;
            }
            if remove_session(&session.path) {
                total_bytes = total_bytes.saturating_sub(session.bytes);
                removed += 1;
                if let Some(date_dir) = session.path.parent() {
                    let _ = fs::remove_dir(date_dir);
                }
            }
        }
    }
    Ok(removed)
}

fn directory_entry(entry: io::Result<fs::DirEntry>, parent: &Path) -> Option<fs::DirEntry> {
    let entry = match entry {
        Ok(entry) => entry,
        Err(error) => {
            report_scan_error(parent, &error);
            return None;
        }
    };
    match entry.file_type() {
        Ok(kind) if kind.is_dir() => Some(entry),
        Ok(_) => None,
        Err(error) => {
            report_scan_error(&entry.path(), &error);
            None
        }
    }
}

fn report_scan_error(path: &Path, error: &io::Error) {
    // Another cleanup pass may have removed this entry already.
    if error.kind() != io::ErrorKind::NotFound {
        eprintln!(
            "[logging] Skipping unreadable logs at {}: {error}",
            path.display()
        );
    }
}

fn remove_session(path: &Path) -> bool {
    match fs::remove_dir_all(path) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "[logging] Failed to remove session {}: {error}",
                path.display()
            );
            false
        }
    }
}

pub(super) fn directory_bytes(path: &Path) -> io::Result<u64> {
    let mut bytes: u64 = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        let size = if metadata.is_dir() {
            directory_bytes(&entry.path())?
        } else {
            metadata.len()
        };
        bytes = bytes.saturating_add(size);
    }
    Ok(bytes)
}
