use std::fs;
use tempfile::TempDir;

use chrono::Utc;

use crate::connection::SessionId;
use crate::profiles::ProfileId;

use super::model::{SessionMetadata, SessionOutcome};
use super::session_manager::SessionLogManager;
use super::SharedSessionLogManager;

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

    let log_dir = manager
        .start_session(&session_id, &profile_id, "Test Profile")
        .unwrap();

    assert!(log_dir.exists());
    assert!(log_dir.join("metadata.json").exists());

    let metadata_content = fs::read_to_string(log_dir.join("metadata.json")).unwrap();
    let metadata: SessionMetadata = serde_json::from_str(&metadata_content).unwrap();
    assert_eq!(metadata.profile_name, "Test Profile");
    assert_eq!(metadata.outcome, SessionOutcome::InProgress);
    assert!(metadata.ended_at.is_none());

    manager.end_session(SessionOutcome::Success);

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

    manager
        .start_session(&session_id, &profile_id, "Test Profile")
        .unwrap();

    manager.log_openvpn("OpenVPN output line");
    manager.log_dns("DNS debug output");
    manager.log_core("Core event: state changed");

    manager.end_session(SessionOutcome::Success);

    let log_dir = temp_dir
        .path()
        .join("sessions")
        .join(Utc::now().format("%Y-%m-%d").to_string())
        .join(format!("session-{}", session_id));

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

    manager
        .start_session(&session_id, &profile_id, "Test Profile")
        .unwrap();
    manager2.log_core("Logged from clone");
    manager2.end_session(SessionOutcome::Success);
}

#[test]
fn get_recent_sessions_returns_multiple_sessions_sorted() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

    let session_id1 = SessionId::new();
    let profile_id1 = ProfileId::new();
    manager
        .start_session(&session_id1, &profile_id1, "Profile 1")
        .unwrap();
    manager.end_session(SessionOutcome::Success);

    std::thread::sleep(std::time::Duration::from_millis(10));

    let session_id2 = SessionId::new();
    let profile_id2 = ProfileId::new();
    manager
        .start_session(&session_id2, &profile_id2, "Profile 2")
        .unwrap();
    manager.end_session(SessionOutcome::Failed);

    std::thread::sleep(std::time::Duration::from_millis(10));

    let session_id3 = SessionId::new();
    let profile_id3 = ProfileId::new();
    manager
        .start_session(&session_id3, &profile_id3, "Profile 3")
        .unwrap();
    manager.end_session(SessionOutcome::Success);

    let sessions = manager.get_recent_sessions(10).unwrap();
    assert_eq!(sessions.len(), 3);

    assert_eq!(sessions[0].profile_name, "Profile 3");
    assert_eq!(sessions[1].profile_name, "Profile 2");
    assert_eq!(sessions[2].profile_name, "Profile 1");

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

    let old_date = Utc::now() - chrono::Duration::days(35);
    let old_date_str = old_date.format("%Y-%m-%d").to_string();
    let old_session_dir = temp_dir
        .path()
        .join("sessions")
        .join(&old_date_str)
        .join("session-old");
    fs::create_dir_all(&old_session_dir).unwrap();

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
    )
    .unwrap();

    let recent_session_id = SessionId::new();
    let recent_profile_id = ProfileId::new();
    manager
        .start_session(&recent_session_id, &recent_profile_id, "Recent Profile")
        .unwrap();
    manager.end_session(SessionOutcome::Success);

    let removed = manager.cleanup_old_sessions(30).unwrap();
    assert_eq!(removed, 1);

    assert!(!old_session_dir.exists());

    let sessions = manager.get_recent_sessions(10).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].profile_name, "Recent Profile");
}

#[test]
fn cleanup_old_sessions_handles_malformed_directory_names() {
    let temp_dir = TempDir::new().unwrap();
    let manager = SessionLogManager::new(temp_dir.path().to_path_buf(), false);

    let malformed_dir = temp_dir
        .path()
        .join("sessions")
        .join("not-a-date")
        .join("session-test");
    fs::create_dir_all(&malformed_dir).unwrap();

    let removed = manager.cleanup_old_sessions(30).unwrap();
    assert_eq!(removed, 0);

    assert!(malformed_dir.exists());
}
