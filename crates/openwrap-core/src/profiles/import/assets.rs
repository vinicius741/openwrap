//! Asset copying, resolution, and hashing.
//!
//! This module handles the filesystem operations for resolving asset paths,
//! copying asset files, extracting inline assets, and computing hashes.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::errors::AppError;
use crate::profiles::{AssetId, AssetKind, AssetOrigin, ManagedAsset, ParsedProfile, ProfileId};

/// Tracks seen assets to detect conflicts between inline and file-based assets.
pub type SeenAssets = HashMap<AssetKind, (String, usize)>;

/// Result of processing all assets (both file-based and inline).
pub struct AssetProcessingResult {
    /// Managed asset records ready for persistence.
    pub assets: Vec<ManagedAsset>,
    /// Mapping from asset kind to the relative path in the managed directory.
    pub rewritten_paths: HashMap<AssetKind, String>,
    /// Paths that were successfully copied (for report).
    pub copied_paths: Vec<String>,
    /// Rewritten path descriptions (for report).
    pub rewritten_descriptions: Vec<String>,
    /// Missing file paths (for report).
    pub missing_files: Vec<String>,
    /// Errors encountered during processing.
    pub errors: Vec<String>,
}

/// Resolves and validates an asset path relative to the source directory.
///
/// This function:
/// - Rejects path traversal attempts (.. components)
/// - Rejects absolute paths outside the source directory
/// - Canonicalizes the resolved path for comparison
pub fn resolve_asset_path(source_dir: &Path, candidate: &Path) -> Result<PathBuf, AppError> {
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

/// Builds a managed asset record with SHA256 hash.
pub fn build_asset(
    profile_id: ProfileId,
    kind: AssetKind,
    relative_path: &str,
    origin: AssetOrigin,
    bytes: Vec<u8>,
) -> ManagedAsset {
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
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

/// Formats an import error for display in the report.
pub fn format_import_error(line: usize, error: &AppError) -> String {
    match error {
        AppError::UnsupportedAbsolutePath(path) => format!(
            "Line {line} references an absolute path outside the imported profile directory: {}",
            path.display()
        ),
        AppError::Validation { title, message, .. } => format!("Line {line} ({title}): {message}"),
        other => format!("Line {line}: {other}"),
    }
}

/// Processes all assets from a parsed profile.
///
/// This function handles both file-based assets and inline assets,
/// copying them to the managed directory and building asset records.
pub fn process_assets(
    profile_id: &ProfileId,
    parsed: &ParsedProfile,
    managed_dir: &Path,
    canonical_source_dir: &Path,
    seen_assets: &mut SeenAssets,
) -> AssetProcessingResult {
    let mut result = AssetProcessingResult {
        assets: Vec::new(),
        rewritten_paths: HashMap::new(),
        copied_paths: Vec::new(),
        rewritten_descriptions: Vec::new(),
        missing_files: Vec::new(),
        errors: Vec::new(),
    };

    let assets_dir = managed_dir.join("assets");
    if let Err(e) = fs::create_dir_all(&assets_dir) {
        result
            .errors
            .push(format!("Failed to create assets directory: {}", e));
        return result;
    }

    // Process file-based assets
    for asset in &parsed.referenced_assets {
        let descriptor = asset.source_path.display().to_string();
        if let Some((existing, line)) = seen_assets.get(&asset.kind) {
            if existing != &descriptor {
                result.errors.push(format!(
                    "Line {} conflicts with line {}: multiple '{}' assets were declared.",
                    asset.line, line, asset.directive
                ));
            }
            continue;
        } else {
            seen_assets.insert(asset.kind.clone(), (descriptor.clone(), asset.line));
        }

        let resolved = match resolve_asset_path(canonical_source_dir, &asset.source_path) {
            Ok(path) => path,
            Err(error) => {
                result.errors.push(format_import_error(asset.line, &error));
                continue;
            }
        };

        if !resolved.exists() {
            result
                .missing_files
                .push(resolved.to_string_lossy().to_string());
            result.errors.push(format!(
                "Line {} references a missing file: {}",
                asset.line,
                resolved.display()
            ));
            continue;
        }

        let relative_path = format!("assets/{}", asset.kind.file_name());
        let target = managed_dir.join(&relative_path);

        if let Err(e) = fs::copy(&resolved, &target) {
            result.errors.push(format!(
                "Line {}: Failed to copy asset '{}': {}",
                asset.line,
                asset.source_path.display(),
                e
            ));
            continue;
        }

        result.copied_paths.push(relative_path.clone());
        result
            .rewritten_descriptions
            .push(format!("{} -> {}", asset.directive, relative_path));
        result
            .rewritten_paths
            .insert(asset.kind.clone(), relative_path.clone());

        let bytes = match fs::read(&target) {
            Ok(b) => b,
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to read copied asset: {}", e));
                continue;
            }
        };

        result.assets.push(build_asset(
            profile_id.clone(),
            asset.kind.clone(),
            &relative_path,
            AssetOrigin::CopiedFile,
            bytes,
        ));
    }

    // Process inline assets
    for inline in &parsed.inline_assets {
        let descriptor = "<inline>".to_string();
        if let Some((existing, line)) = seen_assets.get(&inline.kind) {
            if existing != &descriptor {
                result.errors.push(format!(
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

        if let Err(e) = fs::write(&target, &inline.content) {
            result.errors.push(format!(
                "Line {}: Failed to write inline asset '{}': {}",
                inline.line, inline.directive, e
            ));
            continue;
        }

        result.copied_paths.push(relative_path.clone());
        result
            .rewritten_descriptions
            .push(format!("inline {} -> {}", inline.directive, relative_path));
        result
            .rewritten_paths
            .insert(inline.kind.clone(), relative_path.clone());

        result.assets.push(build_asset(
            profile_id.clone(),
            inline.kind.clone(),
            &relative_path,
            AssetOrigin::ExtractedInline,
            inline.content.as_bytes().to_vec(),
        ));
    }

    result
}

/// Canonicalizes an existing directory path.
pub fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, AppError> {
    fs::canonicalize(path).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{AssetReference, InlineAsset};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn resolves_relative_asset_path() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path();
        fs::write(source_dir.join("ca.crt"), "certificate").unwrap();

        // Canonicalize the source dir first, since resolve_asset_path checks
        // that the resolved path's parent starts with the canonical source dir
        let canonical_source = fs::canonicalize(source_dir).unwrap();
        let resolved = resolve_asset_path(&canonical_source, Path::new("ca.crt")).unwrap();
        assert!(resolved.exists());
        assert!(resolved.ends_with("ca.crt"));
    }

    #[test]
    fn blocks_path_traversal() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();

        let result = resolve_asset_path(&source_dir, Path::new("../escape.key"));
        assert!(result.is_err());
        if let Err(AppError::Validation { title, .. }) = result {
            assert!(title.contains("Path traversal"));
        } else {
            panic!("Expected Validation error");
        }
    }

    #[test]
    fn builds_asset_with_correct_hash() {
        let profile_id = ProfileId::new();
        let bytes = b"test content".to_vec();

        let asset = build_asset(
            profile_id,
            AssetKind::Ca,
            "assets/ca.crt",
            AssetOrigin::CopiedFile,
            bytes,
        );

        assert_eq!(asset.kind, AssetKind::Ca);
        assert_eq!(asset.relative_path, "assets/ca.crt");
        assert_eq!(asset.origin, AssetOrigin::CopiedFile);
        // SHA256 of "test content"
        assert_eq!(
            asset.sha256,
            "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72"
        );
    }

    #[test]
    fn detects_conflicting_assets() {
        let temp = TempDir::new().unwrap();
        let managed_dir = temp.path().join("managed");
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&managed_dir).unwrap();
        fs::write(source_dir.join("ca.crt"), "cert").unwrap();

        let profile_id = ProfileId::new();

        // Create a parsed profile with both file and inline CA
        let parsed = ParsedProfile {
            directives: vec![],
            referenced_assets: vec![AssetReference {
                kind: AssetKind::Ca,
                source_path: PathBuf::from("ca.crt"),
                directive: "ca".to_string(),
                line: 1,
            }],
            inline_assets: vec![InlineAsset {
                kind: AssetKind::Ca,
                content: "inline cert".to_string(),
                directive: "ca".to_string(),
                line: 2,
            }],
            remotes: vec![],
            dns_directives: vec![],
            requires_auth_user_pass: false,
        };

        let canonical_source = canonicalize_existing_dir(&source_dir).unwrap();
        let mut seen = SeenAssets::new();
        let result = process_assets(
            &profile_id,
            &parsed,
            &managed_dir,
            &canonical_source,
            &mut seen,
        );

        assert!(result
            .errors
            .iter()
            .any(|e| e.contains("defined as both inline content and a file asset")));
    }
}
