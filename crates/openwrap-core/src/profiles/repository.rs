use crate::errors::AppError;
use crate::profiles::model::{
    ProfileDetail, ProfileId, ProfileImportResult, ProfileSummary, ValidationFinding,
};

pub trait ProfileRepository: Send + Sync {
    fn save_import(&self, import: ProfileImportResult) -> Result<ProfileDetail, AppError>;
    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, AppError>;
    fn get_profile(&self, profile_id: &ProfileId) -> Result<ProfileDetail, AppError>;
    fn update_has_saved_credentials(
        &self,
        profile_id: &ProfileId,
        has_saved_credentials: bool,
    ) -> Result<(), AppError>;
    fn touch_last_used(&self, profile_id: &ProfileId) -> Result<(), AppError>;
    fn get_settings(&self) -> Result<crate::openvpn::runtime::Settings, AppError>;
    fn save_settings(&self, settings: &crate::openvpn::runtime::Settings) -> Result<(), AppError>;
    fn list_validation_findings(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ValidationFinding>, AppError>;
    fn set_last_selected_profile(
        &self,
        profile_id: Option<&ProfileId>,
    ) -> Result<(), AppError>;
    fn get_last_selected_profile(&self) -> Result<Option<ProfileId>, AppError>;
}
