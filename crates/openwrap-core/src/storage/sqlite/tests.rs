use std::fs;

use crate::profiles::ProfileId;
use crate::profiles::ProfileRepository;

use super::SqliteRepository;

#[test]
fn clearing_last_selected_profile_removes_the_setting_row() {
    let db_path =
        std::env::temp_dir().join(format!("openwrap-sqlite-test-{}.db", uuid::Uuid::new_v4()));
    let repository = SqliteRepository::new(&db_path).unwrap();
    let profile_id = ProfileId::new();

    repository
        .set_last_selected_profile(Some(&profile_id))
        .unwrap();
    assert_eq!(
        repository.get_last_selected_profile().unwrap(),
        Some(profile_id.clone())
    );

    repository.set_last_selected_profile(None).unwrap();
    assert_eq!(repository.get_last_selected_profile().unwrap(), None);

    fs::remove_file(db_path).unwrap();
}

fn temp_db_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("openwrap-sqlite-test-{}.db", uuid::Uuid::new_v4()))
}

/// Inserts a minimal profile row, leaving every column added by later
/// migrations at its default.
fn insert_profile_row(connection: &rusqlite::Connection, profile_id: &ProfileId, dns_policy: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO profiles (
                id, name, source_filename, managed_dir, managed_ovpn_path, original_import_path,
                created_at, updated_at, dns_intent_json, dns_policy, credential_mode,
                remote_summary, validation_status
            ) VALUES (?1, 'Test', 'test.ovpn', '/tmp/p', '/tmp/p/profile.ovpn', '/tmp/test.ovpn',
                      ?2, ?2, '[]', ?3, 'None', 'example.com 1194', 'Ok')",
            rusqlite::params![profile_id.to_string(), now, dns_policy],
        )
        .unwrap();
}

#[test]
fn listed_dns_domains_round_trip_and_reject_invalid_entries() {
    let db_path = temp_db_path();
    let repository = SqliteRepository::new(&db_path).unwrap();
    let profile_id = ProfileId::new();
    insert_profile_row(&repository.connection().lock(), &profile_id, "FullOverride");

    assert!(repository
        .get_profile(&profile_id)
        .unwrap()
        .profile
        .dns_match_domains
        .is_empty());

    let updated = repository
        .update_profile_dns_match_domains(
            &profile_id,
            vec![
                "BMWGroup.net".into(),
                "bmw.com".into(),
                "bmwgroup.net".into(),
            ],
        )
        .unwrap();
    assert_eq!(
        updated.profile.dns_match_domains,
        vec!["bmwgroup.net", "bmw.com"]
    );

    let rejected = repository.update_profile_dns_match_domains(
        &profile_id,
        vec!["bmw.com".into(), "bmw.com; rm -rf /".into()],
    );
    assert!(matches!(
        rejected,
        Err(crate::errors::AppError::Validation { .. })
    ));
    assert_eq!(
        repository
            .get_profile(&profile_id)
            .unwrap()
            .profile
            .dns_match_domains,
        vec!["bmwgroup.net", "bmw.com"]
    );

    fs::remove_file(db_path).unwrap();
}

#[test]
fn upgrading_moves_split_dns_profiles_to_full_override_only_once() {
    let db_path = temp_db_path();
    let split_id = ProfileId::new();
    let observe_id = ProfileId::new();

    // A database written by a version without listed domains.
    {
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE profiles (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    source_filename TEXT NOT NULL,
                    managed_dir TEXT NOT NULL,
                    managed_ovpn_path TEXT NOT NULL,
                    original_import_path TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    dns_intent_json TEXT NOT NULL,
                    dns_policy TEXT NOT NULL DEFAULT 'SplitDnsPreferred',
                    credential_mode TEXT NOT NULL,
                    credential_strategy TEXT NOT NULL DEFAULT 'Prompt',
                    remote_summary TEXT NOT NULL,
                    has_saved_credentials INTEGER NOT NULL DEFAULT 0,
                    validation_status TEXT NOT NULL,
                    last_used_at TEXT NULL
                );",
            )
            .unwrap();
        insert_profile_row(&connection, &split_id, "SplitDnsPreferred");
        insert_profile_row(&connection, &observe_id, "ObserveOnly");
    }

    let repository = SqliteRepository::new(&db_path).unwrap();
    let split = repository.get_profile(&split_id).unwrap().profile;
    assert_eq!(split.dns_policy, crate::dns::DnsPolicy::FullOverride);
    assert!(split.dns_match_domains.is_empty());
    assert_eq!(
        repository
            .get_profile(&observe_id)
            .unwrap()
            .profile
            .dns_policy,
        crate::dns::DnsPolicy::ObserveOnly
    );

    // Choosing listed domains afterwards must survive the next launch.
    repository
        .update_profile_dns_policy(&split_id, crate::dns::DnsPolicy::SplitDnsPreferred)
        .unwrap();
    drop(repository);
    let repository = SqliteRepository::new(&db_path).unwrap();
    assert_eq!(
        repository
            .get_profile(&split_id)
            .unwrap()
            .profile
            .dns_policy,
        crate::dns::DnsPolicy::SplitDnsPreferred
    );

    fs::remove_file(db_path).unwrap();
}
