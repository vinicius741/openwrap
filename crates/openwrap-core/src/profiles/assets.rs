use serde::{Deserialize, Serialize};

use super::ids::{AssetId, ProfileId};

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
pub struct ManagedAsset {
    pub id: AssetId,
    pub profile_id: ProfileId,
    pub kind: AssetKind,
    pub relative_path: String,
    pub sha256: String,
    pub origin: AssetOrigin,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
