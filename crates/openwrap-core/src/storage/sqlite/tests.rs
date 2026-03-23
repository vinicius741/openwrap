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
