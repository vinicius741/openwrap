use crate::app_state::AppState;
use crate::error::CommandError;
use chrono::Utc;
use openwrap_core::profiles::{CredentialStrategy, ProfileDetail, ProfileId, ProfileRepository};
use openwrap_core::secrets::StoredSecret;
use openwrap_core::SecretStore;

use super::parse::parse_profile_id;

#[tauri::command]
pub fn configure_generated_password_profile(
    state: tauri::State<'_, AppState>,
    profile_id: String,
    username: String,
    pin: String,
    totp_secret: String,
) -> Result<openwrap_core::profiles::ProfileDetail, CommandError> {
    let profile_id = parse_profile_id(&profile_id)?;
    let detail = state.profile_repository().get_profile(&profile_id)?;
    if detail.profile.credential_mode != openwrap_core::profiles::CredentialMode::UserPass {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            "Only username/password VPN profiles can use generated passwords.".into(),
        )));
    }

    let username = username.trim();
    if username.is_empty() {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            "Username cannot be empty.".into(),
        )));
    }

    let pin = pin.trim();
    if pin.len() != 4 || !pin.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            "PIN must contain exactly 4 digits.".into(),
        )));
    }

    let totp_secret = totp_secret.trim();
    if totp_secret.is_empty() {
        return Err(CommandError::from(openwrap_core::AppError::Settings(
            "TOTP secret cannot be empty.".into(),
        )));
    }
    openwrap_core::secrets::totp::generate_totp(totp_secret, Utc::now())?;

    let previous_secret = state.secret_store().get_password(&profile_id)?;
    let next_secret = openwrap_core::secrets::StoredSecret::pin_totp(
        profile_id.clone(),
        username.into(),
        pin.into(),
        totp_secret.into(),
    );

    state
        .secret_store()
        .set_password(next_secret)
        .map_err(Into::into)
        .and_then(|_| {
            state
                .profile_repository()
                .update_has_saved_credentials(&profile_id, true)
                .map_err(CommandError::from)
        })
        .and_then(|_| {
            state
                .profile_repository()
                .update_profile_credential_strategy(&profile_id, CredentialStrategy::PinTotp)
                .map_err(CommandError::from)
        })
        .or_else(|error| {
            rollback_generated_password_change(
                state.secret_store(),
                state.profile_repository(),
                &profile_id,
                &detail,
                previous_secret,
            );
            Err(error)
        })
}

#[tauri::command]
pub fn clear_generated_password_profile(
    state: tauri::State<'_, AppState>,
    profile_id: String,
) -> Result<openwrap_core::profiles::ProfileDetail, CommandError> {
    let profile_id = parse_profile_id(&profile_id)?;
    let detail = state.profile_repository().get_profile(&profile_id)?;
    let previous_secret = state.secret_store().get_password(&profile_id)?;

    state
        .secret_store()
        .delete_password(&profile_id)
        .map_err(Into::into)
        .and_then(|_| {
            state
                .profile_repository()
                .update_has_saved_credentials(&profile_id, false)
                .map_err(CommandError::from)
        })
        .and_then(|_| {
            state
                .profile_repository()
                .update_profile_credential_strategy(&profile_id, CredentialStrategy::Prompt)
                .map_err(CommandError::from)
        })
        .or_else(|error| {
            rollback_generated_password_change(
                state.secret_store(),
                state.profile_repository(),
                &profile_id,
                &detail,
                previous_secret,
            );
            Err(error)
        })
}

fn rollback_generated_password_change(
    secret_store: std::sync::Arc<dyn SecretStore>,
    repository: std::sync::Arc<dyn ProfileRepository>,
    profile_id: &ProfileId,
    detail: &ProfileDetail,
    previous_secret: Option<StoredSecret>,
) {
    let _ = restore_secret(secret_store, profile_id, previous_secret);
    let _ =
        repository.update_has_saved_credentials(profile_id, detail.profile.has_saved_credentials);
    let _ = repository
        .update_profile_credential_strategy(profile_id, detail.profile.credential_strategy.clone());
}

fn restore_secret(
    secret_store: std::sync::Arc<dyn SecretStore>,
    profile_id: &ProfileId,
    previous_secret: Option<StoredSecret>,
) -> Result<(), openwrap_core::AppError> {
    match previous_secret {
        Some(secret) => secret_store.set_password(secret),
        None => secret_store.delete_password(profile_id),
    }
}
