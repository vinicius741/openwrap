pub mod composite;
pub mod keychain;
pub mod local_db;
pub mod totp;

use serde::{Deserialize, Deserializer, Serialize};

use crate::profiles::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum StoredSecretKind {
    #[default]
    UsernameOnly,
    PinTotp,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredSecret {
    pub profile_id: ProfileId,
    pub username: String,
    pub kind: StoredSecretKind,
    pub pin: Option<String>,
    pub totp_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StoredSecretPayload {
    profile_id: ProfileId,
    username: String,
    #[serde(default)]
    kind: StoredSecretKind,
    #[serde(default)]
    pin: Option<String>,
    #[serde(default)]
    totp_secret: Option<String>,
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
            kind: payload.kind,
            pin: payload.pin,
            totp_secret: payload.totp_secret,
        })
    }
}

impl StoredSecret {
    pub fn username_only(profile_id: ProfileId, username: String) -> Self {
        Self {
            profile_id,
            username,
            kind: StoredSecretKind::UsernameOnly,
            pin: None,
            totp_secret: None,
        }
    }

    pub fn pin_totp(
        profile_id: ProfileId,
        username: String,
        pin: String,
        totp_secret: String,
    ) -> Self {
        Self {
            profile_id,
            username,
            kind: StoredSecretKind::PinTotp,
            pin: Some(pin),
            totp_secret: Some(totp_secret),
        }
    }

    pub fn is_generated_password(&self) -> bool {
        self.kind == StoredSecretKind::PinTotp
    }
}

pub use composite::CompositeSecretStore;
pub use keychain::KeychainSecretStore;
pub use local_db::LocalSecretStore;

#[cfg(test)]
mod tests {
    use super::{StoredSecret, StoredSecretKind};
    use crate::profiles::ProfileId;

    #[test]
    fn deserializes_legacy_payloads_with_passwords() {
        let profile_id = ProfileId::new();
        let payload =
            format!(r#"{{"profile_id":"{profile_id}","username":"alice","password":"otp"}}"#);

        let stored = serde_json::from_str::<StoredSecret>(&payload).unwrap();

        assert_eq!(stored.profile_id, profile_id);
        assert_eq!(stored.username, "alice");
        assert_eq!(stored.kind, StoredSecretKind::UsernameOnly);
        assert_eq!(stored.pin, None);
        assert_eq!(stored.totp_secret, None);
    }

    #[test]
    fn serializes_generated_password_payloads() {
        let profile_id = ProfileId::new();
        let secret = StoredSecret::pin_totp(
            profile_id.clone(),
            "alice".into(),
            "1234".into(),
            "JBSWY3DPEHPK3PXP".into(),
        );

        let roundtrip: StoredSecret =
            serde_json::from_str(&serde_json::to_string(&secret).unwrap()).unwrap();

        assert_eq!(roundtrip.profile_id, profile_id);
        assert_eq!(roundtrip.username, "alice");
        assert_eq!(roundtrip.kind, StoredSecretKind::PinTotp);
        assert_eq!(roundtrip.pin.as_deref(), Some("1234"));
        assert_eq!(roundtrip.totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
    }
}
