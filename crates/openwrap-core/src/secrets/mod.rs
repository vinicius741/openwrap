pub mod keychain;

use serde::{Deserialize, Deserializer, Serialize};

use crate::profiles::ProfileId;

#[derive(Debug, Clone, Serialize)]
pub struct StoredSecret {
    pub profile_id: ProfileId,
    pub username: String,
}

#[derive(Debug, Deserialize)]
struct StoredSecretPayload {
    profile_id: ProfileId,
    username: String,
    #[serde(default)]
    _password: Option<String>,
}

impl<'de> Deserialize<'de> for StoredSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let payload = StoredSecretPayload::deserialize(deserializer)?;
        Ok(Self {
            profile_id: payload.profile_id,
            username: payload.username,
        })
    }
}

pub use keychain::KeychainSecretStore;

#[cfg(test)]
mod tests {
    use super::StoredSecret;
    use crate::profiles::ProfileId;

    #[test]
    fn deserializes_legacy_payloads_with_passwords() {
        let profile_id = ProfileId::new();
        let payload =
            format!(r#"{{"profile_id":"{profile_id}","username":"alice","password":"otp"}}"#);

        let stored = serde_json::from_str::<StoredSecret>(&payload).unwrap();

        assert_eq!(stored.profile_id, profile_id);
        assert_eq!(stored.username, "alice");
    }
}
