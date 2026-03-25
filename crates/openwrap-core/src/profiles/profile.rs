use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::dns::DnsPolicy;

use super::assets::ManagedAsset;
use super::ids::ProfileId;
use super::validation::{ValidationFinding, ValidationStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CredentialMode {
    None,
    UserPass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CredentialStrategy {
    Prompt,
    PinTotp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportStatus {
    Imported,
    NeedsApproval,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    pub status: ImportStatus,
    pub copied_assets: Vec<String>,
    pub rewritten_paths: Vec<String>,
    pub warnings: Vec<ValidationFinding>,
    pub blocked_directives: Vec<ValidationFinding>,
    pub missing_files: Vec<String>,
    pub errors: Vec<String>,
}

impl Default for ImportReport {
    fn default() -> Self {
        Self {
            status: ImportStatus::Imported,
            copied_assets: Vec::new(),
            rewritten_paths: Vec::new(),
            warnings: Vec::new(),
            blocked_directives: Vec::new(),
            missing_files: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: ProfileId,
    pub name: String,
    pub remote_summary: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub has_saved_credentials: bool,
    pub validation_status: ValidationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub source_filename: String,
    pub managed_dir: PathBuf,
    pub managed_ovpn_path: PathBuf,
    pub original_import_path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub dns_intent: Vec<String>,
    pub dns_policy: DnsPolicy,
    pub credential_mode: CredentialMode,
    pub credential_strategy: CredentialStrategy,
    pub remote_summary: String,
    pub has_saved_credentials: bool,
    pub validation_status: ValidationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDetail {
    pub profile: Profile,
    pub assets: Vec<ManagedAsset>,
    pub findings: Vec<ValidationFinding>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_report_default() {
        let report = ImportReport::default();
        assert_eq!(report.status, ImportStatus::Imported);
        assert!(report.copied_assets.is_empty());
        assert!(report.rewritten_paths.is_empty());
        assert!(report.warnings.is_empty());
        assert!(report.blocked_directives.is_empty());
        assert!(report.missing_files.is_empty());
        assert!(report.errors.is_empty());
    }

    #[test]
    fn credential_mode_serialization() {
        let modes = vec![CredentialMode::None, CredentialMode::UserPass];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let roundtrip: CredentialMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, roundtrip);
        }
    }

    #[test]
    fn credential_strategy_serialization() {
        let strategies = vec![CredentialStrategy::Prompt, CredentialStrategy::PinTotp];
        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let roundtrip: CredentialStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, roundtrip);
        }
    }
}
