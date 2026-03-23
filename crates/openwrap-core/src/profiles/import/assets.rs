use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::errors::AppError;
use crate::profiles::{
    AssetId, AssetKind, AssetOrigin, AssetReference, ImportReport, InlineAsset, ManagedAsset,
    ProfileId,
};

pub struct AssetPipeline {
    seen_assets: HashMap<AssetKind, (String, usize)>,
    managed_dir: PathBuf,
    rewritten_assets: HashMap<AssetKind, String>,
    report: ImportReport,
}

impl AssetPipeline {
    pub fn new(managed_dir: PathBuf) -> Self {
        Self {
            seen_assets: HashMap::new(),
            managed_dir,
            rewritten_assets: HashMap::new(),
            report: ImportReport::default(),
        }
    }

    pub fn report_mut(&mut self) -> &mut ImportReport {
        &mut self.report
    }

    pub fn into_inner(self) -> (HashMap<AssetKind, String>, ImportReport) {
        (self.rewritten_assets, self.report)
    }

    pub fn process_file_asset(
        &mut self,
        profile_id: &ProfileId,
        canonical_source_dir: &Path,
        asset: &AssetReference,
    ) -> Option<ManagedAsset> {
        let descriptor = asset.source_path.display().to_string();
        if let Some((existing, line)) = self.seen_assets.get(&asset.kind) {
            if existing != &descriptor {
                self.report.errors.push(format!(
                    "Line {} conflicts with line {}: multiple '{}' assets were declared.",
                    asset.line, line, asset.directive
                ));
            }
            return None;
        }
        self.seen_assets
            .insert(asset.kind.clone(), (descriptor, asset.line));

        let resolved = match resolve_asset_path(canonical_source_dir, &asset.source_path) {
            Ok(path) => path,
            Err(error) => {
                self.report
                    .errors
                    .push(import_error_message(asset.line, &error));
                return None;
            }
        };

        if !resolved.exists() {
            self.report
                .missing_files
                .push(resolved.to_string_lossy().to_string());
            self.report.errors.push(format!(
                "Line {} references a missing file: {}",
                asset.line,
                resolved.display()
            ));
            return None;
        }

        let relative_path = format!("assets/{}", asset.kind.file_name());
        let target = self.managed_dir.join(&relative_path);
        fs::copy(&resolved, &target).ok()?;
        self.report.copied_assets.push(relative_path.clone());
        self.report
            .rewritten_paths
            .push(format!("{} -> {}", asset.directive, relative_path));
        self.rewritten_assets
            .insert(asset.kind.clone(), relative_path.clone());

        let bytes = fs::read(&target).ok()?;
        Some(build_asset(
            profile_id.clone(),
            asset.kind.clone(),
            &relative_path,
            AssetOrigin::CopiedFile,
            bytes,
        ))
    }

    pub fn process_inline_asset(
        &mut self,
        profile_id: &ProfileId,
        inline: &InlineAsset,
    ) -> Option<ManagedAsset> {
        let descriptor = "<inline>".to_string();
        if let Some((existing, line)) = self.seen_assets.get(&inline.kind) {
            if existing != &descriptor {
                self.report.errors.push(format!(
                    "Line {} conflicts with line {}: '{}' is defined as both inline content and a file asset.",
                    inline.line, line, inline.directive
                ));
            }
            return None;
        }
        self.seen_assets
            .insert(inline.kind.clone(), (descriptor, inline.line));

        let relative_path = format!("assets/{}", inline.kind.file_name());
        let target = self.managed_dir.join(&relative_path);
        fs::write(&target, &inline.content).ok()?;
        self.report.copied_assets.push(relative_path.clone());
        self.report
            .rewritten_paths
            .push(format!("inline {} -> {}", inline.directive, relative_path));
        self.rewritten_assets
            .insert(inline.kind.clone(), relative_path.clone());

        Some(build_asset(
            profile_id.clone(),
            inline.kind.clone(),
            &relative_path,
            AssetOrigin::ExtractedInline,
            inline.content.as_bytes().to_vec(),
        ))
    }
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

    let parent = candidate_path
        .parent()
        .ok_or_else(|| AppError::Validation {
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

pub fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, AppError> {
    fs::canonicalize(path).map_err(AppError::from)
}
