//! Profile importer coordinator.
//!
//! This module provides the main `ProfileImporter` struct that orchestrates
//! the entire profile import process by delegating to specialized submodules.

use std::collections::HashMap as StdHashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use crate::app_state::AppPaths;
use crate::config::parse_profile;
use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::import::assets::{canonicalize_existing_dir, process_assets, SeenAssets};
use crate::profiles::import::report::ReportBuilder;
use crate::profiles::import::validator::validate_directives;
use crate::profiles::repository::ProfileRepository;
use crate::profiles::{
    CredentialMode, ImportReport, ImportStatus, Profile, ProfileId, ProfileImportResult,
    ValidationStatus,
};

/// Request to import a profile from a file.
#[derive(Debug, Clone)]
pub struct ImportProfileRequest {
    pub source_path: PathBuf,
    pub display_name: Option<String>,
    pub allow_warnings: bool,
}

/// Response from a profile import operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportProfileResponse {
    pub profile: Option<crate::profiles::ProfileDetail>,
    pub report: ImportReport,
}

/// Coordinates the profile import process.
///
/// This struct is the main entry point for importing OpenVPN profiles.
/// It delegates validation, asset processing, and report generation to
/// specialized submodules.
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
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let parsed = parse_profile(&source_text, &source_dir)?;

        // Validate directives
        let validation_result = validate_directives(&parsed.directives);
        let findings = validation_result
            .blocked
            .iter()
            .chain(validation_result.warnings.iter())
            .cloned()
            .collect::<Vec<_>>();

        // Build initial report from findings
        let mut report = ReportBuilder::new()
            .with_findings(&findings)
            .build();

        // Check for blocking errors
        if !validation_result.blocked.is_empty() {
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
        }

        // Check if warnings need approval
        if !validation_result.warnings.is_empty() && !request.allow_warnings {
            report.status = ImportStatus::NeedsApproval;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
        }

        // Set up managed directory
        let profile_id = ProfileId::new();
        let managed_dir = self.paths.profiles_dir.join(profile_id.to_string());
        fs::create_dir_all(&managed_dir)?;

        // Copy original file
        let source_filename = request
            .source_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "profile.ovpn".into());
        let original_import_path = managed_dir.join("original.ovpn");
        fs::copy(&request.source_path, &original_import_path)?;

        // Process assets
        let canonical_source_dir = canonicalize_existing_dir(&source_dir)?;
        let mut seen_assets = SeenAssets::new();
        let asset_result = process_assets(
            &profile_id,
            &parsed,
            &managed_dir,
            &canonical_source_dir,
            &mut seen_assets,
        );

        // Check for asset processing errors before updating report
        let has_asset_errors = !asset_result.missing_files.is_empty() || !asset_result.errors.is_empty();

        // Update report with asset processing results
        report.copied_assets = asset_result.copied_paths;
        report.rewritten_paths = asset_result.rewritten_descriptions;
        report.missing_files = asset_result.missing_files;
        report.errors.extend(asset_result.errors);

        // Check for asset processing errors
        if has_asset_errors {
            let _ = fs::remove_dir_all(&managed_dir);
            report.status = ImportStatus::Blocked;
            return Ok(ImportProfileResponse {
                profile: None,
                report,
            });
        }

        // Write rewritten profile
        let managed_ovpn_path = managed_dir.join("profile.ovpn");
        let rewritten_assets: StdHashMap<_, _> = asset_result.rewritten_paths.into_iter().collect();
        fs::write(
            &managed_ovpn_path,
            crate::config::rewrite_profile(&parsed, &rewritten_assets),
        )?;

        // Build profile record
        let validation_status = if validation_result.warnings.is_empty() {
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
            remote_summary: parsed.remotes.join(", "),
            has_saved_credentials: false,
            validation_status,
        };

        // Persist to repository
        let detail = self.repository.save_import(ProfileImportResult {
            profile,
            assets: asset_result.assets,
            findings,
        })?;

        report.status = ImportStatus::Imported;
        Ok(ImportProfileResponse {
            profile: Some(detail),
            report,
        })
    }
}
