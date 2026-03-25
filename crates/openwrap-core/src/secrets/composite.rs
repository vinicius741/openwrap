use crate::errors::AppError;
use crate::profiles::ProfileId;
use crate::secrets::{KeychainSecretStore, LocalSecretStore, StoredSecret, StoredSecretKind};
use crate::SecretStore;

#[derive(Debug)]
pub struct CompositeSecretStore {
    keychain: KeychainSecretStore,
    local: LocalSecretStore,
}

impl CompositeSecretStore {
    pub fn new(keychain: KeychainSecretStore, local: LocalSecretStore) -> Self {
        Self { keychain, local }
    }
}

impl SecretStore for CompositeSecretStore {
    fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError> {
        if let Some(secret) = self.local.get_password(profile_id)? {
            return Ok(Some(secret));
        }
        self.keychain.get_password(profile_id)
    }

    fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
        match secret.kind {
            StoredSecretKind::UsernameOnly => {
                self.local.delete_password(&secret.profile_id)?;
                self.keychain.set_password(secret)
            }
            StoredSecretKind::PinTotp => {
                self.keychain.delete_password(&secret.profile_id)?;
                self.local.set_password(secret)
            }
        }
    }

    fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        self.local.delete_password(profile_id)?;
        self.keychain.delete_password(profile_id)
    }
}
