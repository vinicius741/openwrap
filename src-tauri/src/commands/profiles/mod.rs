pub mod delete;
pub mod dns_policy;
pub mod generated_password;
pub mod import;
pub mod parse;
pub mod selection;

use openwrap_core::profiles::ProfileDetail;
use openwrap_core::SecretStore;
use std::sync::Arc;

pub(crate) fn enrich_with_saved_credentials(
    mut detail: ProfileDetail,
    secret_store: Arc<dyn SecretStore>,
) -> ProfileDetail {
    if detail.profile.credential_mode == openwrap_core::profiles::CredentialMode::UserPass
        && detail.profile.has_saved_credentials
    {
        if let Ok(Some(secret)) = secret_store.get_password(&detail.profile.id) {
            detail.has_saved_pin_totp = secret.is_generated_password();
            detail.saved_username = Some(secret.username);
        } else {
            detail.saved_username = None;
            detail.has_saved_pin_totp = false;
        }
    } else {
        detail.saved_username = None;
        detail.has_saved_pin_totp = false;
    }
    detail
}
