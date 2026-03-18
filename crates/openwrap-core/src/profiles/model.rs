use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dns::{DnsObservation, DnsPolicy};
use crate::errors::UserFacingError;

macro_rules! uuid_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(value)?))
            }
        }
    };
}

uuid_newtype!(ProfileId);
uuid_newtype!(AssetId);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationStatus {
    Ok,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Ca,
    Cert,
    Key,
    Pem,
    Pkcs12,
    TlsAuth,
    TlsCrypt,
    InlineBlob,
}

impl AssetKind {
    pub fn file_name(&self) -> &'static str {
        match self {
            Self::Ca => "ca.crt",
            Self::Cert => "cert.crt",
            Self::Key => "key.key",
            Self::Pem => "bundle.pem",
            Self::Pkcs12 => "identity.p12",
            Self::TlsAuth => "tls-auth.key",
            Self::TlsCrypt => "tls-crypt.key",
            Self::InlineBlob => "inline.blob",
        }
    }

    pub fn from_directive(value: &str) -> Option<Self> {
        match value {
            "ca" => Some(Self::Ca),
            "cert" => Some(Self::Cert),
            "key" => Some(Self::Key),
            "pkcs12" => Some(Self::Pkcs12),
            "tls-auth" => Some(Self::TlsAuth),
            "tls-crypt" => Some(Self::TlsCrypt),
            "pem" => Some(Self::Pem),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetOrigin {
    CopiedFile,
    ExtractedInline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CredentialMode {
    None,
    UserPass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationAction {
    Allow,
    RequireApproval,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationFinding {
    pub severity: ValidationSeverity,
    pub directive: String,
    pub line: usize,
    pub message: String,
    pub action: ValidationAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedAsset {
    pub id: AssetId,
    pub profile_id: ProfileId,
    pub kind: AssetKind,
    pub relative_path: String,
    pub sha256: String,
    pub origin: AssetOrigin,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDirective {
    pub name: String,
    pub args: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReference {
    pub directive: String,
    pub kind: AssetKind,
    pub source_path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineAsset {
    pub directive: String,
    pub kind: AssetKind,
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedProfile {
    pub directives: Vec<ParsedDirective>,
    pub referenced_assets: Vec<AssetReference>,
    pub inline_assets: Vec<InlineAsset>,
    pub remotes: Vec<String>,
    pub dns_directives: Vec<String>,
    pub requires_auth_user_pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileImportResult {
    pub profile: Profile,
    pub assets: Vec<ManagedAsset>,
    pub findings: Vec<ValidationFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRuntimeView {
    pub summary: ProfileSummary,
    pub dns_observation: DnsObservation,
    pub last_error: Option<UserFacingError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_id_new_generates_unique_ids() {
        let id1 = ProfileId::new();
        let id2 = ProfileId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn profile_id_default() {
        let id = ProfileId::default();
        assert_ne!(id.0, uuid::Uuid::nil());
    }

    #[test]
    fn profile_id_display() {
        let id = ProfileId::new();
        assert_eq!(id.to_string(), id.0.to_string());
    }

    #[test]
    fn profile_id_from_str_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id: ProfileId = uuid_str.parse().unwrap();
        assert_eq!(id.0.to_string(), uuid_str);
    }

    #[test]
    fn profile_id_from_str_invalid() {
        let result: Result<ProfileId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn asset_id_new_generates_unique_ids() {
        let id1 = AssetId::new();
        let id2 = AssetId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn validation_status_serialization() {
        let statuses = vec![
            ValidationStatus::Ok,
            ValidationStatus::Warning,
            ValidationStatus::Blocked,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let roundtrip: ValidationStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, roundtrip);
        }
    }

    #[test]
    fn asset_kind_file_name() {
        assert_eq!(AssetKind::Ca.file_name(), "ca.crt");
        assert_eq!(AssetKind::Cert.file_name(), "cert.crt");
        assert_eq!(AssetKind::Key.file_name(), "key.key");
        assert_eq!(AssetKind::Pem.file_name(), "bundle.pem");
        assert_eq!(AssetKind::Pkcs12.file_name(), "identity.p12");
        assert_eq!(AssetKind::TlsAuth.file_name(), "tls-auth.key");
        assert_eq!(AssetKind::TlsCrypt.file_name(), "tls-crypt.key");
        assert_eq!(AssetKind::InlineBlob.file_name(), "inline.blob");
    }

    #[test]
    fn asset_kind_from_directive() {
        assert_eq!(AssetKind::from_directive("ca"), Some(AssetKind::Ca));
        assert_eq!(AssetKind::from_directive("cert"), Some(AssetKind::Cert));
        assert_eq!(AssetKind::from_directive("key"), Some(AssetKind::Key));
        assert_eq!(AssetKind::from_directive("pkcs12"), Some(AssetKind::Pkcs12));
        assert_eq!(
            AssetKind::from_directive("tls-auth"),
            Some(AssetKind::TlsAuth)
        );
        assert_eq!(
            AssetKind::from_directive("tls-crypt"),
            Some(AssetKind::TlsCrypt)
        );
        assert_eq!(AssetKind::from_directive("pem"), Some(AssetKind::Pem));
        assert_eq!(AssetKind::from_directive("unknown"), None);
    }

    #[test]
    fn asset_origin_serialization() {
        assert_eq!(
            serde_json::to_string(&AssetOrigin::CopiedFile).unwrap(),
            "\"CopiedFile\""
        );
        assert_eq!(
            serde_json::to_string(&AssetOrigin::ExtractedInline).unwrap(),
            "\"ExtractedInline\""
        );
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
    fn validation_severity_order() {
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Info).unwrap(),
            "\"Info\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Warn).unwrap(),
            "\"Warn\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn validation_action_serialization() {
        let actions = vec![
            ValidationAction::Allow,
            ValidationAction::RequireApproval,
            ValidationAction::Block,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let roundtrip: ValidationAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, roundtrip);
        }
    }

    #[test]
    fn validation_finding_structure() {
        let finding = ValidationFinding {
            severity: ValidationSeverity::Error,
            directive: "script-security".to_string(),
            line: 42,
            message: "Blocked directive".to_string(),
            action: ValidationAction::Block,
        };
        assert_eq!(finding.severity, ValidationSeverity::Error);
        assert_eq!(finding.directive, "script-security");
        assert_eq!(finding.line, 42);
        assert_eq!(finding.action, ValidationAction::Block);
    }

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
    fn managed_asset_structure() {
        let asset = ManagedAsset {
            id: AssetId::new(),
            profile_id: ProfileId::new(),
            kind: AssetKind::Ca,
            relative_path: "ca.crt".to_string(),
            sha256: "abc123".to_string(),
            origin: AssetOrigin::CopiedFile,
        };
        assert_eq!(asset.kind, AssetKind::Ca);
        assert_eq!(asset.origin, AssetOrigin::CopiedFile);
    }

    #[test]
    fn parsed_directive_structure() {
        let directive = ParsedDirective {
            name: "remote".to_string(),
            args: vec!["vpn.example.com".to_string(), "1194".to_string()],
            line: 10,
        };
        assert_eq!(directive.name, "remote");
        assert_eq!(directive.args.len(), 2);
        assert_eq!(directive.line, 10);
    }

    #[test]
    fn inline_asset_structure() {
        let asset = InlineAsset {
            directive: "ca".to_string(),
            kind: AssetKind::Ca,
            content: "CERTIFICATE".to_string(),
            line: 5,
        };
        assert_eq!(asset.directive, "ca");
        assert_eq!(asset.kind, AssetKind::Ca);
        assert_eq!(asset.content, "CERTIFICATE");
        assert_eq!(asset.line, 5);
    }

    #[test]
    fn parsed_profile_empty() {
        let profile = ParsedProfile {
            directives: Vec::new(),
            referenced_assets: Vec::new(),
            inline_assets: Vec::new(),
            remotes: Vec::new(),
            dns_directives: Vec::new(),
            requires_auth_user_pass: false,
        };
        assert!(profile.directives.is_empty());
        assert!(profile.remotes.is_empty());
        assert!(!profile.requires_auth_user_pass);
    }

    #[test]
    fn parsed_profile_with_remotes() {
        let mut profile = ParsedProfile {
            directives: Vec::new(),
            referenced_assets: Vec::new(),
            inline_assets: Vec::new(),
            remotes: Vec::new(),
            dns_directives: Vec::new(),
            requires_auth_user_pass: false,
        };
        profile.remotes.push("vpn.example.com 1194 udp".to_string());
        profile.requires_auth_user_pass = true;
        assert_eq!(profile.remotes.len(), 1);
        assert!(profile.requires_auth_user_pass);
    }
}
