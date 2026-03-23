use openwrap_core::AppError;

pub fn parse_profile_id(id: &str) -> Result<openwrap_core::profiles::ProfileId, AppError> {
    id.parse::<openwrap_core::profiles::ProfileId>()
        .map_err(|error: uuid::Error| AppError::ConnectionState(error.to_string()))
}
