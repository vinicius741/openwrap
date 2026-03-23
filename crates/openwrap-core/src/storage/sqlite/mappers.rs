//! Row mapping functions for SQLite queries.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::Row;

use crate::profiles::{
    AssetId, ManagedAsset, Profile, ProfileId, ProfileSummary, ValidationFinding,
};

use super::codec::{
    asset_kind_from_string, asset_origin_from_string, credential_mode_from_string,
    dns_policy_from_string, validation_action_from_string, validation_severity_from_string,
    validation_status_from_string,
};

/// Maps a database row to a [`ProfileSummary`].
pub fn map_profile_summary(row: &Row<'_>) -> rusqlite::Result<ProfileSummary> {
    Ok(ProfileSummary {
        id: ProfileId::from_str(&row.get::<_, String>(0)?).unwrap(),
        name: row.get(1)?,
        remote_summary: row.get(2)?,
        last_used_at: row
            .get::<_, Option<String>>(3)?
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc)),
        has_saved_credentials: row.get::<_, i64>(4)? != 0,
        validation_status: validation_status_from_string(&row.get::<_, String>(5)?),
    })
}

/// Maps a database row to a [`Profile`].
pub fn map_profile(row: &Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: ProfileId::from_str(&row.get::<_, String>(0)?).unwrap(),
        name: row.get(1)?,
        source_filename: row.get(2)?,
        managed_dir: row.get::<_, String>(3)?.into(),
        managed_ovpn_path: row.get::<_, String>(4)?.into(),
        original_import_path: row.get::<_, String>(5)?.into(),
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
            .unwrap()
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
            .unwrap()
            .with_timezone(&Utc),
        dns_intent: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
        dns_policy: dns_policy_from_string(&row.get::<_, String>(9)?),
        credential_mode: credential_mode_from_string(&row.get::<_, String>(10)?),
        remote_summary: row.get(11)?,
        has_saved_credentials: row.get::<_, i64>(12)? != 0,
        validation_status: validation_status_from_string(&row.get::<_, String>(13)?),
    })
}

/// Maps a database row to a [`ManagedAsset`].
pub fn map_asset(row: &Row<'_>) -> rusqlite::Result<ManagedAsset> {
    Ok(ManagedAsset {
        id: AssetId::from_str(&row.get::<_, String>(0)?).unwrap(),
        profile_id: ProfileId::from_str(&row.get::<_, String>(1)?).unwrap(),
        kind: asset_kind_from_string(&row.get::<_, String>(2)?),
        relative_path: row.get(3)?,
        sha256: row.get(4)?,
        origin: asset_origin_from_string(&row.get::<_, String>(5)?),
    })
}

/// Maps a database row to a [`ValidationFinding`].
pub fn map_finding(row: &Row<'_>) -> rusqlite::Result<ValidationFinding> {
    Ok(ValidationFinding {
        directive: row.get(0)?,
        line: row.get::<_, i64>(1)? as usize,
        severity: validation_severity_from_string(&row.get::<_, String>(2)?),
        message: row.get(3)?,
        action: validation_action_from_string(&row.get::<_, String>(4)?),
    })
}
