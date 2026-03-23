use serde::{Deserialize, Serialize};

use super::assets::{AssetKind, ManagedAsset};
use super::validation::ValidationFinding;

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
    pub source_path: std::path::PathBuf,
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
    pub profile: super::Profile,
    pub assets: Vec<ManagedAsset>,
    pub findings: Vec<ValidationFinding>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
