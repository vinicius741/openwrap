use openwrap_core::profiles::ProfileId;

use crate::app_state::AppState;

pub fn resolve_connect_target(state: &tauri::State<'_, AppState>) -> Option<(ProfileId, String)> {
    let repository = state.profile_repository();
    repository
        .get_last_selected_profile()
        .ok()
        .flatten()
        .and_then(|profile_id| {
            repository
                .get_profile(&profile_id)
                .ok()
                .map(|profile| (profile_id, profile.profile.name))
        })
        .or_else(|| {
            repository
                .list_profiles()
                .ok()?
                .into_iter()
                .next()
                .map(|profile| (profile.id, profile.name))
        })
}
