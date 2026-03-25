use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::Row;

use crate::profiles::{
    AssetKind, AssetOrigin, CredentialStrategy, ManagedAsset, Profile, ProfileId, ProfileSummary,
    ValidationAction, ValidationFinding, ValidationSeverity, ValidationStatus,
};

use super::codec::dns_policy_from_string;

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
        validation_status: match row.get::<_, String>(5)?.as_str() {
            "Warning" => ValidationStatus::Warning,
            "Blocked" => ValidationStatus::Blocked,
            _ => ValidationStatus::Ok,
        },
    })
}

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
        credential_mode: match row.get::<_, String>(10)?.as_str() {
            "UserPass" => crate::profiles::CredentialMode::UserPass,
            _ => crate::profiles::CredentialMode::None,
        },
        credential_strategy: match row.get::<_, String>(11)?.as_str() {
            "PinTotp" => CredentialStrategy::PinTotp,
            _ => CredentialStrategy::Prompt,
        },
        remote_summary: row.get(12)?,
        has_saved_credentials: row.get::<_, i64>(13)? != 0,
        validation_status: match row.get::<_, String>(14)?.as_str() {
            "Warning" => ValidationStatus::Warning,
            "Blocked" => ValidationStatus::Blocked,
            _ => ValidationStatus::Ok,
        },
    })
}

pub fn map_asset(row: &Row<'_>) -> rusqlite::Result<ManagedAsset> {
    Ok(ManagedAsset {
        id: crate::profiles::AssetId::from_str(&row.get::<_, String>(0)?).unwrap(),
        profile_id: ProfileId::from_str(&row.get::<_, String>(1)?).unwrap(),
        kind: match row.get::<_, String>(2)?.as_str() {
            "Ca" => AssetKind::Ca,
            "Cert" => AssetKind::Cert,
            "Key" => AssetKind::Key,
            "Pem" => AssetKind::Pem,
            "Pkcs12" => AssetKind::Pkcs12,
            "TlsAuth" => AssetKind::TlsAuth,
            "TlsCrypt" => AssetKind::TlsCrypt,
            _ => AssetKind::InlineBlob,
        },
        relative_path: row.get(3)?,
        sha256: row.get(4)?,
        origin: match row.get::<_, String>(5)?.as_str() {
            "CopiedFile" => AssetOrigin::CopiedFile,
            _ => AssetOrigin::ExtractedInline,
        },
    })
}

pub fn map_finding(row: &Row<'_>) -> rusqlite::Result<ValidationFinding> {
    Ok(ValidationFinding {
        directive: row.get(0)?,
        line: row.get::<_, i64>(1)? as usize,
        severity: match row.get::<_, String>(2)?.as_str() {
            "Warn" => ValidationSeverity::Warn,
            "Error" => ValidationSeverity::Error,
            _ => ValidationSeverity::Info,
        },
        message: row.get(3)?,
        action: match row.get::<_, String>(4)?.as_str() {
            "RequireApproval" => ValidationAction::RequireApproval,
            "Block" => ValidationAction::Block,
            _ => ValidationAction::Allow,
        },
    })
}
