use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::app_state::AppPaths;
use crate::config::{classify_directive, parse_profile, rewrite_profile, DirectiveClassification};
use crate::errors::AppError;
use crate::profiles::repository::ProfileRepository;
use crate::profiles::{
    AssetId, AssetKind, AssetOrigin, CredentialMode, ImportReport, ImportStatus, ManagedAsset,
    Profile, ProfileId, ProfileImportResult, ValidationAction, ValidationFinding, ValidationSeverity,
    ValidationStatus,
};

#[derive(Debug, Clone)]
pub struct ImportProfileRequest {
    pub source_path: PathBuf,
    pub display_name: Option<String>,
    pub allow_warnings: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportProfileResponse {
    pub profile: Option<crate::profiles::ProfileDetail>,
    pub report: ImportReport,
}

pub struct ProfileImporter {
    paths: AppPaths,
    repository: Arc<dyn ProfileRepository>,
}

impl ProfileImporter {
    pub fn new(paths: AppPaths, repository: Arc<dyn ProfileRepository>) -> Self {
        Self { paths, repository }
    }

    pub fn import_profile(
        &self,
        request: ImportProfileRequest,
    ) -> Result<ImportProfileResponse, AppError> {
        self.paths.ensure()?;
        let source_text = fs::read_to_string(&request.source_path)?;
        let source_dir = request
            .source_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let parsed = parse_profile(&source_text, &source_dir)?;

        let mut report = ImportReport::default();
        let mut findings = Vec::new();
        let canonical_source_dir = canonicalize_existing_dir(&source_dir)?;

        for directive in &parsed.directives {
            if directive.name == "auth-user-pass" && !directive.args.is_empty() {
                findings.push(ValidationFinding {
                    severity: ValidationSeverity::Error,
                    directive: directive.name.clone(),
                    line: directive.line,
                    message: "auth-user-pass file paths are blocked in v1; use Keychain-backed prompts instead.".into(),
                    action: ValidationAction::Block,
                });
                continue;
            }

            let classification = classify_directive(&directive.name, &directive.args);
            match classification {
                DirectiveClassification::Allowed => {}
                DirectiveClassification::Warned => findings.push(ValidationFinding {
                    severity: ValidationSeverity::Warn,
                    directive: directive.name.clone(),
                    line: directive.line,
                    message: format!("'{}' requires explicit approval during import.", directive.name),
                    action: ValidationAction::RequireApproval,
                }),
                DirectiveClassification::Blocked => findings.push(ValidationFinding {
                    severity: ValidationSeverity::Error,
                    directive: directive.name.clone(),
                    line: directive.line,
                    message: format!("'{}' is blocked in v1.", directive.name),
                    action: ValidationAction::Block,
                }),
            }
        }

        let blocked = findings
            .iter()
            .filter(|finding| finding.action == ValidationAction::Block)
            .cloned()
            .collect::<Vec<_>>();
        let warnings = findings
            .iter()
            .filter(|finding| finding.action == ValidationAction::RequireApproval)
            .cloned()
            .collect::<Vec<_>>();
        report.blocked_directives = blocked.clone();
        report.warnings = warnings.clone();
        report.errors.extend(
            blocked
                .iter()
                .map(|finding| format!("Line {}: {}", finding.line, finding.message)),
        );

        if !blocked.is_empty() {
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse { profile: None, report });
        }

        if !warnings.is_empty() && !request.allow_warnings {
            report.status = ImportStatus::NeedsApproval;
            return Ok(ImportProfileResponse { profile: None, report });
        }

        let profile_id = ProfileId::new();
        let managed_dir = self.paths.profiles_dir.join(profile_id.to_string());
        fs::create_dir_all(&managed_dir)?;
        let assets_dir = managed_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;
        let source_filename = request
            .source_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "profile.ovpn".into());
        let original_import_path = managed_dir.join("original.ovpn");
        fs::copy(&request.source_path, &original_import_path)?;

        let mut rewritten_assets = HashMap::new();
        let mut assets = Vec::new();
        let mut seen_assets = HashMap::<AssetKind, (String, usize)>::new();

        for asset in &parsed.referenced_assets {
            let descriptor = asset.source_path.display().to_string();
            if let Some((existing, line)) = seen_assets.get(&asset.kind) {
                if existing != &descriptor {
                    report.errors.push(format!(
                        "Line {} conflicts with line {}: multiple '{}' assets were declared.",
                        asset.line, line, asset.directive
                    ));
                }
                continue;
            } else {
                seen_assets.insert(asset.kind.clone(), (descriptor.clone(), asset.line));
            }

            let resolved = match resolve_asset_path(&canonical_source_dir, &asset.source_path) {
                Ok(path) => path,
                Err(error) => {
                    report.errors.push(import_error_message(asset.line, &error));
                    continue;
                }
            };
            if !resolved.exists() {
                report.missing_files.push(resolved.to_string_lossy().to_string());
                report
                    .errors
                    .push(format!("Line {} references a missing file: {}", asset.line, resolved.display()));
                continue;
            }
            let relative_path = format!("assets/{}", asset.kind.file_name());
            let target = managed_dir.join(&relative_path);
            fs::copy(&resolved, &target)?;
            report.copied_assets.push(relative_path.clone());
            report.rewritten_paths.push(format!("{} -> {}", asset.directive, relative_path));
            rewritten_assets.insert(asset.kind.clone(), relative_path.clone());
            assets.push(build_asset(
                profile_id.clone(),
                asset.kind.clone(),
                &relative_path,
                AssetOrigin::CopiedFile,
                fs::read(&target)?,
            ));
        }

        for inline in &parsed.inline_assets {
            let descriptor = "<inline>".to_string();
            if let Some((existing, line)) = seen_assets.get(&inline.kind) {
                if existing != &descriptor {
                    report.errors.push(format!(
                        "Line {} conflicts with line {}: '{}' is defined as both inline content and a file asset.",
                        inline.line, line, inline.directive
                    ));
                }
                continue;
            } else {
                seen_assets.insert(inline.kind.clone(), (descriptor, inline.line));
            }

            let relative_path = format!("assets/{}", inline.kind.file_name());
            let target = managed_dir.join(&relative_path);
            fs::write(&target, &inline.content)?;
            report.copied_assets.push(relative_path.clone());
            report.rewritten_paths.push(format!("inline {} -> {}", inline.directive, relative_path));
            rewritten_assets.insert(inline.kind.clone(), relative_path.clone());
            assets.push(build_asset(
                profile_id.clone(),
                inline.kind.clone(),
                &relative_path,
                AssetOrigin::ExtractedInline,
                inline.content.as_bytes().to_vec(),
            ));
        }

        if !report.missing_files.is_empty() || !report.errors.is_empty() {
            let _ = fs::remove_dir_all(&managed_dir);
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse { profile: None, report });
        }

        let managed_ovpn_path = managed_dir.join("profile.ovpn");
        fs::write(&managed_ovpn_path, rewrite_profile(&parsed, &rewritten_assets))?;

        let validation_status = if warnings.is_empty() {
            ValidationStatus::Ok
        } else {
            ValidationStatus::Warning
        };
        let now = Utc::now();
        let profile = Profile {
            id: profile_id.clone(),
            name: request.display_name.unwrap_or_else(|| {
                request
                    .source_path
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Imported profile".into())
            }),
            source_filename,
            managed_dir: managed_dir.clone(),
            managed_ovpn_path,
            original_import_path,
            created_at: now,
            updated_at: now,
            dns_intent: parsed.dns_directives.clone(),
            credential_mode: if parsed.requires_auth_user_pass {
                CredentialMode::UserPass
            } else {
                CredentialMode::None
            },
            remote_summary: parsed.remotes.join(", "),
            has_saved_credentials: false,
            validation_status,
        };

        let detail = self.repository.save_import(ProfileImportResult {
            profile,
            assets,
            findings,
        })?;
        report.status = ImportStatus::Imported;
        Ok(ImportProfileResponse {
            profile: Some(detail),
            report,
        })
    }
}

fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, AppError> {
    fs::canonicalize(path).map_err(AppError::from)
}

fn resolve_asset_path(source_dir: &Path, candidate: &Path) -> Result<PathBuf, AppError> {
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AppError::Validation {
            title: "Path traversal detected".into(),
            message: "Referenced asset escapes the import directory.".into(),
            directive: None,
            line: None,
        });
    }

    let candidate_path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        source_dir.join(candidate)
    };

    let parent = candidate_path.parent().ok_or_else(|| AppError::Validation {
        title: "Invalid asset path".into(),
        message: "Referenced asset path has no parent directory.".into(),
        directive: None,
        line: None,
    })?;
    let canonical_parent = fs::canonicalize(parent)?;

    if !canonical_parent.starts_with(source_dir) {
        return if candidate.is_absolute() {
            Err(AppError::UnsupportedAbsolutePath(candidate.to_path_buf()))
        } else {
            Err(AppError::Validation {
                title: "Path traversal detected".into(),
                message: "Referenced asset escapes the import directory.".into(),
                directive: None,
                line: None,
            })
        };
    }

    if candidate_path.exists() {
        fs::canonicalize(&candidate_path).map_err(AppError::from)
    } else {
        Ok(candidate_path)
    }
}

fn import_error_message(line: usize, error: &AppError) -> String {
    match error {
        AppError::UnsupportedAbsolutePath(path) => format!(
            "Line {line} references an absolute path outside the imported profile directory: {}",
            path.display()
        ),
        AppError::Validation { title, message, .. } => format!("Line {line} ({title}): {message}"),
        other => format!("Line {line}: {other}"),
    }
}

fn build_asset(
    profile_id: ProfileId,
    kind: AssetKind,
    relative_path: &str,
    origin: AssetOrigin,
    bytes: Vec<u8>,
) -> ManagedAsset {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sha256 = format!("{:x}", hasher.finalize());

    ManagedAsset {
        id: AssetId::new(),
        profile_id,
        kind,
        relative_path: relative_path.to_string(),
        sha256,
        origin,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use tempfile::TempDir;

    use crate::app_state::AppPaths;
    use crate::profiles::import::{ImportProfileRequest, ProfileImporter};
    use crate::storage::sqlite::SqliteRepository;

    #[test]
    fn imports_profile_and_rewrites_assets() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("ca.crt"), "certificate").unwrap();
        fs::write(
            source_dir.join("sample.ovpn"),
            "client\nremote vpn.example.com 1194\nca ca.crt\n",
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository.clone());

        let response = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("sample.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(response.profile.is_some());
        let profile = response.profile.unwrap();
        assert!(profile.profile.managed_ovpn_path.exists());
        let stored = fs::read_to_string(&profile.profile.managed_ovpn_path).unwrap();
        assert!(stored.contains("ca assets/ca.crt"));
    }

    #[test]
    fn blocks_missing_assets() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("sample.ovpn"),
            "client\nremote vpn.example.com 1194\nca missing.crt\n",
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository);

        let response = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("sample.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(response.profile.is_none());
        assert_eq!(response.report.status, crate::profiles::ImportStatus::Blocked);
        assert_eq!(response.report.missing_files.len(), 1);
        assert_eq!(response.report.errors.len(), 1);
    }

    #[test]
    fn blocks_asset_path_traversal() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(source_dir.join("nested")).unwrap();
        fs::write(temp.path().join("escape.key"), "secret").unwrap();
        fs::write(
            source_dir.join("nested").join("sample.ovpn"),
            "client\nremote vpn.example.com 1194\nkey ../escape.key\n",
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository);

        let response = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("nested").join("sample.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(response.profile.is_none());
        assert_eq!(response.report.status, crate::profiles::ImportStatus::Blocked);
        assert!(response
            .report
            .errors
            .iter()
            .any(|error| error.contains("Path traversal detected")));
    }

    #[test]
    fn blocks_duplicate_inline_and_file_assets() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("ca.crt"), "certificate").unwrap();
        fs::write(
            source_dir.join("sample.ovpn"),
            "client\nremote vpn.example.com 1194\nca ca.crt\n<ca>\ninline\n</ca>\n",
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository);

        let response = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("sample.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(response.profile.is_none());
        assert!(response
            .report
            .errors
            .iter()
            .any(|error| error.contains("defined as both inline content and a file asset")));
    }

    #[test]
    fn blocks_non_dns_dhcp_options() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("sample.ovpn"),
            "client\nremote vpn.example.com 1194\ndhcp-option DOMAIN corp.example\n",
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository);

        let response = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("sample.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(response.profile.is_none());
        assert_eq!(response.report.status, crate::profiles::ImportStatus::Blocked);
        assert!(response
            .report
            .errors
            .iter()
            .any(|error| error.contains("dhcp-option")));
    }

    #[test]
    fn bmw_profile_requires_approval_but_is_importable() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("bmw.ovpn"),
            r#"dev tun
persist-tun
persist-key
data-ciphers AES-256-CBC:AES-256-GCM:AES-256-CFB
data-ciphers-fallback AES-256-CFB
auth SHA256
tls-client
client
resolv-retry infinite
remote aws-b.ilia.digital 1197 udp4
nobind
auth-user-pass
remote-cert-tls server
explicit-exit-notify
reneg-sec 0
pull-filter ignore redirect-gateway
dhcp-option DNS 10.0.1.50
route 160.52.107.21 255.255.255.255
setenv CLIENT_CERT 0
key-direction 1
<ca>
certificate
</ca>
<tls-auth>
static-key
</tls-auth>
"#,
        )
        .unwrap();

        let paths = AppPaths::new(temp.path().join("app"));
        paths.ensure().unwrap();
        let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
        let importer = ProfileImporter::new(paths, repository);

        let preview = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("bmw.ovpn"),
                display_name: None,
                allow_warnings: false,
            })
            .unwrap();

        assert!(preview.profile.is_none());
        assert_eq!(preview.report.status, crate::profiles::ImportStatus::NeedsApproval);
        assert!(preview.report.errors.is_empty());
        assert_eq!(preview.report.warnings.len(), 3);

        let imported = importer
            .import_profile(ImportProfileRequest {
                source_path: source_dir.join("bmw.ovpn"),
                display_name: None,
                allow_warnings: true,
            })
            .unwrap();

        assert!(imported.profile.is_some());
        assert_eq!(imported.report.status, crate::profiles::ImportStatus::Imported);
        assert!(imported.report.errors.is_empty());

        let profile = imported.profile.unwrap();
        let stored = fs::read_to_string(&profile.profile.managed_ovpn_path).unwrap();
        assert!(stored.contains("ca assets/ca.crt"));
        assert!(stored.contains("tls-auth assets/tls-auth.key"));
        assert!(stored.contains("auth-nocache"));
    }
}
