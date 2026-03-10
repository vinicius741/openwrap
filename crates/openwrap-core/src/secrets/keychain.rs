use crate::errors::AppError;
use crate::profiles::ProfileId;
use crate::secrets::StoredSecret;
use crate::SecretStore;

const SERVICE_NAME: &str = "app.openwrap.credentials";

#[derive(Debug, Default)]
pub struct KeychainSecretStore;

impl KeychainSecretStore {
    pub fn new() -> Self {
        Self
    }

    fn account(profile_id: &ProfileId) -> String {
        format!("profile-{profile_id}")
    }
}

impl SecretStore for KeychainSecretStore {
    fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError> {
        platform::get_password(&Self::account(profile_id))
    }

    fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
        platform::set_password(&Self::account(&secret.profile_id), &secret)
    }

    fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        platform::delete_password(&Self::account(profile_id))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use security_framework::base::Error as SecurityError;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    use super::{AppError, StoredSecret, SERVICE_NAME};

    pub fn get_password(account: &str) -> Result<Option<StoredSecret>, AppError> {
        match get_generic_password(SERVICE_NAME, account) {
            Ok(bytes) => serde_json::from_slice::<StoredSecret>(&bytes)
                .map(Some)
                .map_err(|error| AppError::Keychain(error.to_string())),
            Err(error) if is_not_found(&error) => Ok(None),
            Err(error) => Err(AppError::Keychain(error.to_string())),
        }
    }

    pub fn set_password(account: &str, secret: &StoredSecret) -> Result<(), AppError> {
        let payload = serde_json::to_vec(secret)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        set_generic_password(SERVICE_NAME, account, &payload)
            .map_err(|error| AppError::Keychain(error.to_string()))
    }

    pub fn delete_password(account: &str) -> Result<(), AppError> {
        match delete_generic_password(SERVICE_NAME, account) {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Ok(()),
            Err(error) => Err(AppError::Keychain(error.to_string())),
        }
    }

    fn is_not_found(error: &SecurityError) -> bool {
        error.code() == -25300
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{AppError, StoredSecret};

    pub fn get_password(_account: &str) -> Result<Option<StoredSecret>, AppError> {
        Err(AppError::Keychain(
            "Keychain storage is only available on macOS.".into(),
        ))
    }

    pub fn set_password(_account: &str, _secret: &StoredSecret) -> Result<(), AppError> {
        Err(AppError::Keychain(
            "Keychain storage is only available on macOS.".into(),
        ))
    }

    pub fn delete_password(_account: &str) -> Result<(), AppError> {
        Err(AppError::Keychain(
            "Keychain storage is only available on macOS.".into(),
        ))
    }
}
