use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::CommandError;

use super::enrich_with_saved_credentials;
use super::parse::parse_profile_id;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileDnsPolicyDto {
    pub profile_id: String,
    pub dns_policy: openwrap_core::dns::DnsPolicy,
}

#[tauri::command]
pub fn update_profile_dns_policy(
    state: tauri::State<'_, AppState>,
    request: UpdateProfileDnsPolicyDto,
) -> Result<openwrap_core::profiles::ProfileDetail, CommandError> {
    let profile_id = parse_profile_id(&request.profile_id)?;

    let detail = state
        .profile_repository()
        .update_profile_dns_policy(&profile_id, request.dns_policy)
        .map_err(CommandError::from)?;
    Ok(enrich_with_saved_credentials(
        detail,
        state.secret_store(),
    ))
}
