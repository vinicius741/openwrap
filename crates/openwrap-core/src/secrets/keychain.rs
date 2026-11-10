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

/// Windows Credential Manager (generic credentials), target
/// `app.openwrap.credentials:profile-<id>`. Stores the same JSON payload as the
/// macOS Keychain entry, so it holds the username only, never the password.
#[cfg(windows)]
mod platform {
    use std::io;

    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    use super::{AppError, StoredSecret, SERVICE_NAME};
    use crate::windows::to_wide;

    fn target(account: &str) -> Vec<u16> {
        to_wide(&format!("{SERVICE_NAME}:{account}"))
    }

    fn last_error() -> io::Error {
        io::Error::last_os_error()
    }

    fn is_not_found(error: &io::Error) -> bool {
        error.raw_os_error() == Some(ERROR_NOT_FOUND as i32)
    }

    pub fn get_password(account: &str) -> Result<Option<StoredSecret>, AppError> {
        let target = target(account);
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
            let error = last_error();
            return if is_not_found(&error) {
                Ok(None)
            } else {
                Err(AppError::Keychain(error.to_string()))
            };
        }

        let payload = unsafe {
            let credential = &*credential;
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
            .to_vec()
        };
        unsafe { CredFree(credential.cast()) };

        serde_json::from_slice::<StoredSecret>(&payload)
            .map(Some)
            .map_err(|error| AppError::Keychain(error.to_string()))
    }

    pub fn set_password(account: &str, secret: &StoredSecret) -> Result<(), AppError> {
        let mut payload = serde_json::to_vec(secret)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let mut target = target(account);
        let mut user_name = to_wide(account);
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: u32::try_from(payload.len())
                .map_err(|error| AppError::Keychain(error.to_string()))?,
            CredentialBlob: payload.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user_name.as_mut_ptr(),
            ..Default::default()
        };
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            return Err(AppError::Keychain(last_error().to_string()));
        }
        Ok(())
    }

    pub fn delete_password(account: &str) -> Result<(), AppError> {
        let target = target(account);
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let error = last_error();
            if !is_not_found(&error) {
                return Err(AppError::Keychain(error.to_string()));
            }
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::{AppError, StoredSecret};

    pub fn get_password(_account: &str) -> Result<Option<StoredSecret>, AppError> {
        Err(AppError::Keychain(
            "Credential storage is only available on macOS and Windows.".into(),
        ))
    }

    pub fn set_password(_account: &str, _secret: &StoredSecret) -> Result<(), AppError> {
        Err(AppError::Keychain(
            "Credential storage is only available on macOS and Windows.".into(),
        ))
    }

    pub fn delete_password(_account: &str) -> Result<(), AppError> {
        Err(AppError::Keychain(
            "Credential storage is only available on macOS and Windows.".into(),
        ))
    }
}
