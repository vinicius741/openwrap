pub mod keychain;

use serde::{Deserialize, Serialize};

use crate::profiles::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSecret {
    pub profile_id: ProfileId,
    pub username: String,
    pub password: String,
}

pub use keychain::KeychainSecretStore;

