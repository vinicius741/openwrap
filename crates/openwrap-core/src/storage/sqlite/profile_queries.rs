//! Profile-related database queries.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::{ProfileDetail, ProfileId, ProfileImportResult, ProfileSummary, ValidationFinding};

use super::codec::dns_policy_to_string;
use super::mappers::{map_asset, map_finding, map_profile, map_profile_summary};

/// Saves a profile import result to the database.
///
/// This operation is transactional - all profile data, assets, and findings
/// are saved atomically.
pub fn save_import(
    connection: &Connection,
    import: &ProfileImportResult,
) -> Result<ProfileDetail, AppError> {
    let transaction = connection.unchecked_transaction()?;

    connection.execute(
        "INSERT OR REPLACE INTO profiles (
            id, name, source_filename, managed_dir, managed_ovpn_path, original_import_path,
            created_at, updated_at, dns_intent_json, dns_policy, credential_mode, remote_summary,
            has_saved_credentials, validation_status, last_used_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL)",
        params![
            import.profile.id.to_string(),
            import.profile.name,
            import.profile.source_filename,
            import.profile.managed_dir.to_string_lossy(),
            import.profile.managed_ovpn_path.to_string_lossy(),
            import.profile.original_import_path.to_string_lossy(),
            import.profile.created_at.to_rfc3339(),
            import.profile.updated_at.to_rfc3339(),
            serde_json::to_string(&import.profile.dns_intent)
                .map_err(|error| AppError::Serialization(error.to_string()))?,
            dns_policy_to_string(&import.profile.dns_policy),
            format!("{:?}", import.profile.credential_mode),
            import.profile.remote_summary,
            import.profile.has_saved_credentials as i64,
            format!("{:?}", import.profile.validation_status),
        ],
    )?;

    connection.execute(
        "DELETE FROM profile_assets WHERE profile_id = ?1",
        params![import.profile.id.to_string()],
    )?;
    connection.execute(
        "DELETE FROM profile_validation_findings WHERE profile_id = ?1",
        params![import.profile.id.to_string()],
    )?;

    for asset in &import.assets {
        connection.execute(
            "INSERT INTO profile_assets (id, profile_id, kind, relative_path, sha256, origin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                asset.id.to_string(),
                asset.profile_id.to_string(),
                format!("{:?}", asset.kind),
                asset.relative_path,
                asset.sha256,
                format!("{:?}", asset.origin),
            ],
        )?;
    }

    for finding in &import.findings {
        connection.execute(
            "INSERT INTO profile_validation_findings
             (profile_id, directive, line, severity, message, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                import.profile.id.to_string(),
                finding.directive,
                finding.line as i64,
                format!("{:?}", finding.severity),
                finding.message,
                format!("{:?}", finding.action),
            ],
        )?;
    }

    transaction.commit()?;

    get_profile(connection, &import.profile.id)
}

/// Lists all profiles as summaries.
pub fn list_profiles(connection: &Connection) -> Result<Vec<ProfileSummary>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, name, remote_summary, last_used_at, has_saved_credentials, validation_status
         FROM profiles ORDER BY name ASC",
    )?;

    let rows = statement.query_map([], map_profile_summary)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Gets a single profile with all its assets and findings.
pub fn get_profile(
    connection: &Connection,
    profile_id: &ProfileId,
) -> Result<ProfileDetail, AppError> {
    let profile = connection
        .query_row(
            "SELECT id, name, source_filename, managed_dir, managed_ovpn_path, original_import_path,
                    created_at, updated_at, dns_intent_json, dns_policy, credential_mode, remote_summary,
                    has_saved_credentials, validation_status, last_used_at
             FROM profiles WHERE id = ?1",
            params![profile_id.to_string()],
            map_profile,
        )
        .optional()?
        .ok_or_else(|| AppError::ProfileNotFound(profile_id.to_string()))?;

    let mut asset_stmt = connection.prepare(
        "SELECT id, profile_id, kind, relative_path, sha256, origin
         FROM profile_assets WHERE profile_id = ?1 ORDER BY relative_path ASC",
    )?;
    let assets = asset_stmt
        .query_map(params![profile_id.to_string()], map_asset)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut findings_stmt = connection.prepare(
        "SELECT directive, line, severity, message, action
         FROM profile_validation_findings WHERE profile_id = ?1 ORDER BY line ASC",
    )?;
    let findings = findings_stmt
        .query_map(params![profile_id.to_string()], map_finding)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ProfileDetail {
        profile,
        assets,
        findings,
    })
}

/// Updates the `has_saved_credentials` flag for a profile.
pub fn update_has_saved_credentials(
    connection: &Connection,
    profile_id: &ProfileId,
    has_saved_credentials: bool,
) -> Result<(), AppError> {
    connection.execute(
        "UPDATE profiles SET has_saved_credentials = ?1 WHERE id = ?2",
        params![has_saved_credentials as i64, profile_id.to_string()],
    )?;
    Ok(())
}

/// Updates the `last_used_at` timestamp for a profile.
pub fn touch_last_used(connection: &Connection, profile_id: &ProfileId) -> Result<(), AppError> {
    connection.execute(
        "UPDATE profiles SET last_used_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), profile_id.to_string()],
    )?;
    Ok(())
}

/// Updates the DNS policy for a profile.
pub fn update_profile_dns_policy(
    connection: &Connection,
    profile_id: &ProfileId,
    policy: DnsPolicy,
) -> Result<(), AppError> {
    connection.execute(
        "UPDATE profiles SET dns_policy = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            dns_policy_to_string(&policy),
            Utc::now().to_rfc3339(),
            profile_id.to_string()
        ],
    )?;
    Ok(())
}

/// Lists validation findings for a profile.
pub fn list_validation_findings(
    connection: &Connection,
    profile_id: &ProfileId,
) -> Result<Vec<ValidationFinding>, AppError> {
    let mut statement = connection.prepare(
        "SELECT directive, line, severity, message, action
         FROM profile_validation_findings WHERE profile_id = ?1 ORDER BY line ASC",
    )?;
    let rows = statement.query_map(params![profile_id.to_string()], map_finding)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Deletes a profile and all its associated data.
///
/// This operation is transactional - the profile, its assets, findings,
/// and connection history are all deleted atomically.
pub fn delete_profile(connection: &Connection, profile_id: &ProfileId) -> Result<(), AppError> {
    let transaction = connection.unchecked_transaction()?;
    let id_str = profile_id.to_string();

    connection.execute(
        "DELETE FROM profile_assets WHERE profile_id = ?1",
        params![id_str],
    )?;
    connection.execute(
        "DELETE FROM profile_validation_findings WHERE profile_id = ?1",
        params![id_str],
    )?;
    connection.execute(
        "DELETE FROM connection_history WHERE profile_id = ?1",
        params![id_str],
    )?;
    connection.execute("DELETE FROM profiles WHERE id = ?1", params![id_str])?;

    transaction.commit()?;
    Ok(())
}
