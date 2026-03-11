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
