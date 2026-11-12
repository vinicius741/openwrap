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
    Ok(enrich_with_saved_credentials(detail, state.secret_store()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileDnsMatchDomainsDto {
    pub profile_id: String,
    pub domains: Vec<String>,
}

/// Replaces the domains resolved through the VPN when the profile uses
/// `DnsPolicy::SplitDnsPreferred`. Takes effect on the next connect.
#[tauri::command]
pub fn update_profile_dns_match_domains(
    state: tauri::State<'_, AppState>,
    request: UpdateProfileDnsMatchDomainsDto,
) -> Result<openwrap_core::profiles::ProfileDetail, CommandError> {
    let profile_id = parse_profile_id(&request.profile_id)?;

    let detail = state
        .profile_repository()
        .update_profile_dns_match_domains(&profile_id, request.domains)
        .map_err(CommandError::from)?;
    Ok(enrich_with_saved_credentials(detail, state.secret_store()))
}
