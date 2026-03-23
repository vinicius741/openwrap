mod assets;
mod importer;
#[cfg(test)]
mod tests;
mod validator;

pub use assets::{canonicalize_existing_dir, AssetPipeline};
pub use importer::ProfileImporter;
pub use validator::{blocked_findings, validate_directives, warning_findings};

use crate::profiles::{ImportReport, ProfileDetail};

#[derive(Debug, Clone)]
pub struct ImportProfileRequest {
    pub source_path: std::path::PathBuf,
    pub display_name: Option<String>,
    pub allow_warnings: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportProfileResponse {
    pub profile: Option<ProfileDetail>,
    pub report: ImportReport,
}
