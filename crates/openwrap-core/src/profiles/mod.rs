pub mod assets;
pub mod ids;
pub mod import;
pub mod model;
pub mod parsed;
pub mod profile;
pub mod runtime;
pub mod validation;

pub mod repository;

pub use assets::{AssetKind, AssetOrigin, ManagedAsset};
pub use ids::{AssetId, ProfileId};
pub use import::{ImportProfileRequest, ImportProfileResponse, ProfileImporter};
pub use parsed::{
    AssetReference, InlineAsset, ParsedDirective, ParsedProfile, ProfileImportResult,
};
pub use profile::{
    CredentialMode, CredentialStrategy, ImportReport, ImportStatus, Profile, ProfileDetail,
    ProfileSummary,
};
pub use repository::ProfileRepository;
pub use runtime::ProfileRuntimeView;
pub use validation::{ValidationAction, ValidationFinding, ValidationSeverity, ValidationStatus};
