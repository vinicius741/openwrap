use std::process::Command;

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
        let output = Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                SERVICE_NAME,
                "-a",
                &Self::account(profile_id),
                "-w",
            ])
            .output()
            .map_err(|error| AppError::Keychain(error.to_string()))?;

        if !output.status.success() {
            return Ok(None);
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let secret = serde_json::from_str::<StoredSecret>(raw.trim())
            .map_err(|error| AppError::Keychain(error.to_string()))?;
        Ok(Some(secret))
    }

    fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
        let payload = serde_json::to_string(&secret)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let output = Command::new("/usr/bin/security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                SERVICE_NAME,
                "-a",
                &Self::account(&secret.profile_id),
                "-w",
                &payload,
            ])
            .output()
            .map_err(|error| AppError::Keychain(error.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(AppError::Keychain(String::from_utf8_lossy(&output.stderr).trim().to_string()))
        }
    }

    fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        let output = Command::new("/usr/bin/security")
            .args([
                "delete-generic-password",
                "-s",
                SERVICE_NAME,
                "-a",
                &Self::account(profile_id),
            ])
            .output()
            .map_err(|error| AppError::Keychain(error.to_string()))?;

        if output.status.success() || String::from_utf8_lossy(&output.stderr).contains("could not be found") {
            Ok(())
        } else {
            Err(AppError::Keychain(String::from_utf8_lossy(&output.stderr).trim().to_string()))
        }
    }
}
