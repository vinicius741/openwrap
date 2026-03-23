//! Enum serialization helpers for SQLite storage.

use crate::dns::DnsPolicy;
use crate::profiles::{AssetKind, AssetOrigin, CredentialMode, ValidationAction, ValidationSeverity, ValidationStatus};

/// Converts a DNS policy to its string representation for storage.
pub fn dns_policy_to_string(policy: &DnsPolicy) -> &'static str {
    match policy {
        DnsPolicy::SplitDnsPreferred => "SplitDnsPreferred",
        DnsPolicy::FullOverride => "FullOverride",
        DnsPolicy::ObserveOnly => "ObserveOnly",
    }
}

/// Parses a DNS policy from its string representation.
pub fn dns_policy_from_string(value: &str) -> DnsPolicy {
    match value {
        "FullOverride" => DnsPolicy::FullOverride,
        "ObserveOnly" => DnsPolicy::ObserveOnly,
        _ => DnsPolicy::SplitDnsPreferred,
    }
}

/// Parses a validation status from its string representation.
pub fn validation_status_from_string(value: &str) -> ValidationStatus {
    match value {
        "Warning" => ValidationStatus::Warning,
        "Blocked" => ValidationStatus::Blocked,
        _ => ValidationStatus::Ok,
    }
}

/// Parses a credential mode from its string representation.
pub fn credential_mode_from_string(value: &str) -> CredentialMode {
    match value {
        "UserPass" => CredentialMode::UserPass,
        _ => CredentialMode::None,
    }
}

/// Parses an asset kind from its string representation.
pub fn asset_kind_from_string(value: &str) -> AssetKind {
    match value {
        "Ca" => AssetKind::Ca,
        "Cert" => AssetKind::Cert,
        "Key" => AssetKind::Key,
        "Pem" => AssetKind::Pem,
        "Pkcs12" => AssetKind::Pkcs12,
        "TlsAuth" => AssetKind::TlsAuth,
        "TlsCrypt" => AssetKind::TlsCrypt,
        _ => AssetKind::InlineBlob,
    }
}

/// Parses an asset origin from its string representation.
pub fn asset_origin_from_string(value: &str) -> AssetOrigin {
    match value {
        "CopiedFile" => AssetOrigin::CopiedFile,
        _ => AssetOrigin::ExtractedInline,
    }
}

/// Parses a validation severity from its string representation.
pub fn validation_severity_from_string(value: &str) -> ValidationSeverity {
    match value {
        "Warn" => ValidationSeverity::Warn,
        "Error" => ValidationSeverity::Error,
        _ => ValidationSeverity::Info,
    }
}

/// Parses a validation action from its string representation.
pub fn validation_action_from_string(value: &str) -> ValidationAction {
    match value {
        "RequireApproval" => ValidationAction::RequireApproval,
        "Block" => ValidationAction::Block,
        _ => ValidationAction::Allow,
    }
}
