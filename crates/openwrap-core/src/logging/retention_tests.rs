use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Duration, Utc};
use tempfile::TempDir;

use super::model::{SessionMetadata, SessionOutcome};
use super::retention::{cleanup_sessions, directory_bytes};

fn session(base: &Path, days_ago: i64, outcome: SessionOutcome) -> PathBuf {
    let started_at = Utc::now() - Duration::days(days_ago);
    let id = uuid::Uuid::new_v4().to_string();
    let path = base
        .join("sessions")
        .join(started_at.format("%Y-%m-%d").to_string())
        .join(format!("session-{id}"));
    fs::create_dir_all(&path).unwrap();
    let metadata = SessionMetadata {
        session_id: id,
        profile_id: "test".into(),
        profile_name: "Test".into(),
        started_at,
        ended_at: (outcome != SessionOutcome::InProgress).then_some(started_at),
        outcome,
        verbose_mode: false,
    };
    fs::write(
        path.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    fs::write(path.join("openvpn.log"), [b'x'; 64]).unwrap();
    path
}

#[test]
fn retention_keeps_active_session_in_an_expired_date_directory() {
    let temp = TempDir::new().unwrap();
    let active = session(temp.path(), 35, SessionOutcome::InProgress);
    let expired = session(temp.path(), 35, SessionOutcome::Success);
    let recent = session(temp.path(), 1, SessionOutcome::Failed);
    let failure_log = temp.path().join("last-failed-openvpn.log");
    fs::write(&failure_log, "last failure").unwrap();

    assert_eq!(
        cleanup_sessions(temp.path(), 30, None, Some(&active)).unwrap(),
        1
    );
    assert!(active.join("openvpn.log").exists());
    assert!(!expired.exists());
    assert!(recent.exists());
    assert!(failure_log.exists());
}

#[test]
fn size_budget_removes_oldest_completed_sessions_first() {
    let temp = TempDir::new().unwrap();
    let oldest = session(temp.path(), 3, SessionOutcome::Success);
    let middle = session(temp.path(), 2, SessionOutcome::Failed);
    let newest = session(temp.path(), 1, SessionOutcome::Cancelled);
    let unfinished = session(temp.path(), 1, SessionOutcome::InProgress);
    let budget = directory_bytes(&middle).unwrap() + directory_bytes(&newest).unwrap();

    assert_eq!(
        cleanup_sessions(temp.path(), 30, Some(budget), None).unwrap(),
        1
    );
    assert!(!oldest.exists());
    assert!(middle.exists());
    assert!(newest.exists());
    assert!(unfinished.exists());
}

#[test]
fn age_retention_handles_crash_leftovers_and_preserves_the_cutoff_day() {
    let temp = TempDir::new().unwrap();
    let stale = session(temp.path(), 31, SessionOutcome::InProgress);
    let boundary = session(temp.path(), 30, SessionOutcome::Success);
    let today = session(temp.path(), 0, SessionOutcome::Success);
    let unrelated = stale.parent().unwrap().join("keep-me");
    fs::create_dir_all(&unrelated).unwrap();

    assert_eq!(cleanup_sessions(temp.path(), 30, None, None).unwrap(), 1);
    assert!(!stale.exists());
    assert!(unrelated.exists());
    assert!(boundary.exists());
    assert!(today.exists());
    assert_eq!(
        cleanup_sessions(temp.path(), u32::MAX, None, None).unwrap(),
        0
    );
}

#[test]
fn protected_session_survives_even_a_zero_size_budget() {
    let temp = TempDir::new().unwrap();
    let protected = session(temp.path(), 1, SessionOutcome::Success);
    let removable = session(temp.path(), 1, SessionOutcome::Success);
    assert_eq!(
        cleanup_sessions(temp.path(), 30, Some(0), Some(&protected)).unwrap(),
        1
    );
    assert!(protected.exists());
    assert!(!removable.exists());
}

#[cfg(unix)]
#[test]
fn retention_does_not_follow_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let external = session(outside.path(), 35, SessionOutcome::Success);
    let old_date = external.parent().unwrap().file_name().unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir_all(&root).unwrap();
    symlink(external.parent().unwrap(), root.join(old_date)).unwrap();
    assert_eq!(cleanup_sessions(temp.path(), 30, Some(0), None).unwrap(), 0);

    fs::remove_file(root.join(old_date)).unwrap();
    let date_dir = root.join(old_date);
    fs::create_dir(&date_dir).unwrap();
    symlink(&external, date_dir.join("session-linked")).unwrap();
    assert_eq!(cleanup_sessions(temp.path(), 30, Some(0), None).unwrap(), 0);
    assert!(external.join("openvpn.log").exists());

    // Size scans also avoid following nested directory links.
    let local = session(temp.path(), 1, SessionOutcome::Success);
    symlink(&external, local.join("external")).unwrap();
    cleanup_sessions(temp.path(), 30, Some(0), None).unwrap();
    assert!(!local.exists());
    assert!(external.join("openvpn.log").exists());
}

#[cfg(unix)]
struct UnreadableDirectory {
    path: PathBuf,
    permissions: fs::Permissions,
}

#[cfg(unix)]
impl UnreadableDirectory {
    fn new(path: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0)).unwrap();
        Self { path, permissions }
    }
}

#[cfg(unix)]
impl Drop for UnreadableDirectory {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.permissions.clone());
    }
}

#[cfg(unix)]
#[test]
fn unreadable_date_does_not_block_age_or_size_cleanup() {
    let temp = TempDir::new().unwrap();
    let blocked = session(temp.path(), 2, SessionOutcome::Success);
    let expired = session(temp.path(), 35, SessionOutcome::Success);
    let completed = session(temp.path(), 1, SessionOutcome::Success);
    let guard = UnreadableDirectory::new(blocked.parent().unwrap().to_path_buf());
    if fs::read_dir(&guard.path).is_ok() {
        // Processes with elevated privileges may bypass mode bits.
        return;
    }

    assert_eq!(cleanup_sessions(temp.path(), 30, Some(0), None).unwrap(), 2);
    assert!(!expired.exists());
    assert!(!completed.exists());
    drop(guard);
    assert!(blocked.exists());
}

#[cfg(unix)]
#[test]
fn unreadable_session_contents_do_not_block_other_budget_candidates() {
    let temp = TempDir::new().unwrap();
    let blocked = session(temp.path(), 2, SessionOutcome::Success);
    let nested = blocked.join("unreadable");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("log"), "keep").unwrap();
    let completed = session(temp.path(), 1, SessionOutcome::Success);
    let guard = UnreadableDirectory::new(nested);
    if fs::read_dir(&guard.path).is_ok() {
        return;
    }

    assert_eq!(cleanup_sessions(temp.path(), 30, Some(0), None).unwrap(), 1);
    assert!(blocked.exists());
    assert!(!completed.exists());
}

#[cfg(unix)]
#[test]
fn symlinked_sessions_root_supports_cleanup_and_active_protection() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let relocated = TempDir::new().unwrap();
    let protected = session(relocated.path(), 35, SessionOutcome::Success);
    let expired = session(relocated.path(), 35, SessionOutcome::Success);
    let completed = session(relocated.path(), 1, SessionOutcome::Success);
    let relocated_root = relocated.path().join("sessions");
    symlink(&relocated_root, temp.path().join("sessions")).unwrap();
    let active_path = temp
        .path()
        .join("sessions")
        .join(protected.strip_prefix(&relocated_root).unwrap());

    assert_eq!(
        cleanup_sessions(temp.path(), 30, Some(0), Some(&active_path)).unwrap(),
        2
    );
    assert!(protected.exists());
    assert!(!expired.exists());
    assert!(!completed.exists());
    assert!(fs::symlink_metadata(temp.path().join("sessions"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn concurrent_cleanup_passes_tolerate_removed_entries() {
    let temp = TempDir::new().unwrap();
    for index in 0..32 {
        session(temp.path(), index % 7, SessionOutcome::Success);
    }
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let run = || {
            barrier.wait();
            cleanup_sessions(temp.path(), 30, Some(0), None).unwrap()
        };
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        first.join().unwrap();
        second.join().unwrap();
    });
    assert_eq!(cleanup_sessions(temp.path(), 30, Some(0), None).unwrap(), 0);
    assert!(fs::read_dir(temp.path().join("sessions"))
        .unwrap()
        .next()
        .is_none());
}
