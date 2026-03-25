use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;

use crate::app_state::AppPaths;
use crate::config::{parse_profile, rewrite_profile};
use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::repository::ProfileRepository;
use crate::profiles::{
    CredentialMode, CredentialStrategy, ImportReport, ImportStatus, Profile, ProfileId,
    ProfileImportResult, ValidationStatus,
};

use super::validator::{blocked_findings, validate_directives, warning_findings};
use super::{AssetPipeline, ImportProfileRequest, ImportProfileResponse};

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
        let findings = validate_directives(&parsed);
        let blocked = blocked_findings(&findings);
        let warnings = warning_findings(&findings);
        report.blocked_directives = blocked.clone();
        report.warnings = warnings.clone();
        report.errors.extend(
            blocked
                .iter()
                .map(|finding| format!("Line {}: {}", finding.line, finding.message)),
        );

        if !blocked.is_empty() {
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
        }

        if !warnings.is_empty() && !request.allow_warnings {
            report.status = ImportStatus::NeedsApproval;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
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

        let canonical_source_dir = canonicalize_existing_dir(&source_dir)?;
        let mut pipeline = AssetPipeline::new(managed_dir.clone());
        let mut assets = Vec::new();

        for asset in &parsed.referenced_assets {
            if let Some(managed_asset) =
                pipeline.process_file_asset(&profile_id, &canonical_source_dir, asset)
            {
                assets.push(managed_asset);
            }
        }

        for inline in &parsed.inline_assets {
            if let Some(managed_asset) = pipeline.process_inline_asset(&profile_id, inline) {
                assets.push(managed_asset);
            }
        }

        let (rewritten_assets, asset_report) = pipeline.into_inner();
        report.copied_assets = asset_report.copied_assets;
        report.rewritten_paths = asset_report.rewritten_paths;
        report.missing_files = asset_report.missing_files;
        report.errors.extend(asset_report.errors);

        if !report.missing_files.is_empty() || !report.errors.is_empty() {
            let _ = fs::remove_dir_all(&managed_dir);
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
        }

        let managed_ovpn_path = managed_dir.join("profile.ovpn");
        fs::write(
            &managed_ovpn_path,
            rewrite_profile(&parsed, &rewritten_assets),
        )?;

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
            dns_policy: DnsPolicy::SplitDnsPreferred,
            credential_mode: if parsed.requires_auth_user_pass {
                CredentialMode::UserPass
            } else {
                CredentialMode::None
            },
            credential_strategy: CredentialStrategy::Prompt,
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
    use std::fs;
    fs::canonicalize(path).map_err(AppError::from)
}
