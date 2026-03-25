use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::AppError;
use crate::profiles::ProfileId;
use crate::secrets::{StoredSecret, StoredSecretKind};
use crate::SecretStore;

#[derive(Debug)]
pub struct LocalSecretStore {
    connection: Mutex<Connection>,
}

impl LocalSecretStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open(&path)?;
        #[cfg(unix)]
        {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(&path, permissions)?;
        }

        let store = Self {
            connection: Mutex::new(connection),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), AppError> {
        self.connection.lock().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS profile_secrets (
                profile_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                username TEXT NOT NULL,
                pin TEXT NULL,
                totp_secret TEXT NULL
            );
            "#,
        )?;
        Ok(())
    }
}

impl SecretStore for LocalSecretStore {
    fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError> {
        self.connection
            .lock()
            .query_row(
                "SELECT kind, username, pin, totp_secret FROM profile_secrets WHERE profile_id = ?1",
                params![profile_id.to_string()],
                |row| {
                    let kind = match row.get::<_, String>(0)?.as_str() {
                        "PinTotp" => StoredSecretKind::PinTotp,
                        _ => StoredSecretKind::UsernameOnly,
                    };
                    Ok(StoredSecret {
                        profile_id: profile_id.clone(),
                        username: row.get(1)?,
                        kind,
                        pin: row.get(2)?,
                        totp_secret: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
        self.connection.lock().execute(
            "INSERT INTO profile_secrets (profile_id, kind, username, pin, totp_secret)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(profile_id) DO UPDATE SET
                 kind = excluded.kind,
                 username = excluded.username,
                 pin = excluded.pin,
                 totp_secret = excluded.totp_secret",
            params![
                secret.profile_id.to_string(),
                format!("{:?}", secret.kind),
                secret.username,
                secret.pin,
                secret.totp_secret,
            ],
        )?;
        Ok(())
    }

    fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        self.connection.lock().execute(
            "DELETE FROM profile_secrets WHERE profile_id = ?1",
            params![profile_id.to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::LocalSecretStore;
    use crate::profiles::ProfileId;
    use crate::secrets::{StoredSecret, StoredSecretKind};
    use crate::SecretStore;

    #[test]
    fn stores_generated_password_profiles() {
        let temp = tempdir().unwrap();
        let store = LocalSecretStore::new(temp.path().join("secrets.sqlite3")).unwrap();
        let profile_id = ProfileId::new();

        store
            .set_password(StoredSecret::pin_totp(
                profile_id.clone(),
                "alice".into(),
                "1234".into(),
                "JBSWY3DPEHPK3PXP".into(),
            ))
            .unwrap();

        let secret = store.get_password(&profile_id).unwrap().unwrap();
        assert_eq!(secret.kind, StoredSecretKind::PinTotp);
        assert_eq!(secret.username, "alice");
        assert_eq!(secret.pin.as_deref(), Some("1234"));
    }

    #[test]
    fn deletes_stored_secrets() {
        let temp = tempdir().unwrap();
        let store = LocalSecretStore::new(temp.path().join("secrets.sqlite3")).unwrap();
        let profile_id = ProfileId::new();

        store
            .set_password(StoredSecret::username_only(
                profile_id.clone(),
                "alice".into(),
            ))
            .unwrap();
        store.delete_password(&profile_id).unwrap();

        assert!(store.get_password(&profile_id).unwrap().is_none());
    }
}
