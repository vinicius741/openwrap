use std::path::Path;
use std::str::FromStr;

use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::repository::ProfileRepository;
use crate::profiles::{
    CredentialStrategy, ProfileDetail, ProfileId, ProfileImportResult, ProfileSummary,
    ValidationFinding,
};

use super::codec::dns_policy_to_string;
use super::mappers::{map_asset, map_finding, map_profile, map_profile_summary};
use super::schema::ensure_column;

pub struct SqliteRepository {
    connection: Mutex<Connection>,
}

impl SqliteRepository {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let connection = Connection::open(path)?;
        let repository = Self {
            connection: Mutex::new(connection),
        };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&self) -> Result<(), AppError> {
        let connection = self.connection.lock();
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                source_filename TEXT NOT NULL,
                managed_dir TEXT NOT NULL,
                managed_ovpn_path TEXT NOT NULL,
                original_import_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                dns_intent_json TEXT NOT NULL,
                dns_policy TEXT NOT NULL DEFAULT 'SplitDnsPreferred',
                credential_mode TEXT NOT NULL,
                credential_strategy TEXT NOT NULL DEFAULT 'Prompt',
                remote_summary TEXT NOT NULL,
                has_saved_credentials INTEGER NOT NULL DEFAULT 0,
                validation_status TEXT NOT NULL,
                last_used_at TEXT NULL
            );

            CREATE TABLE IF NOT EXISTS profile_assets (
                id TEXT PRIMARY KEY,
                profile_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                origin TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS profile_validation_findings (
                profile_id TEXT NOT NULL,
                directive TEXT NOT NULL,
                line INTEGER NOT NULL,
                severity TEXT NOT NULL,
                message TEXT NOT NULL,
                action TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS connection_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id TEXT NOT NULL,
                connected_at TEXT NOT NULL,
                state TEXT NOT NULL
            );
            "#,
        )?;
        ensure_column(
            &connection,
            "profiles",
            "dns_policy",
            "TEXT NOT NULL DEFAULT 'SplitDnsPreferred'",
        )?;
        ensure_column(
            &connection,
            "profiles",
            "credential_strategy",
            "TEXT NOT NULL DEFAULT 'Prompt'",
        )?;
        Ok(())
    }

    pub fn connection(&self) -> &Mutex<Connection> {
        &self.connection
    }
}

impl ProfileRepository for SqliteRepository {
    fn save_import(&self, import: ProfileImportResult) -> Result<ProfileDetail, AppError> {
        let connection = self.connection.lock();
        connection.execute(
            "INSERT OR REPLACE INTO profiles (
                id, name, source_filename, managed_dir, managed_ovpn_path, original_import_path,
                created_at, updated_at, dns_intent_json, dns_policy, credential_mode, credential_strategy,
                remote_summary, has_saved_credentials, validation_status, last_used_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, NULL)",
            params![
                import.profile.id.to_string(),
                import.profile.name,
                import.profile.source_filename,
                import.profile.managed_dir.to_string_lossy(),
                import.profile.managed_ovpn_path.to_string_lossy(),
                import.profile.original_import_path.to_string_lossy(),
                import.profile.created_at.to_rfc3339(),
                import.profile.updated_at.to_rfc3339(),
                serde_json::to_string(&import.profile.dns_intent)
                    .map_err(|error| AppError::Serialization(error.to_string()))?,
                dns_policy_to_string(&import.profile.dns_policy),
                format!("{:?}", import.profile.credential_mode),
                format!("{:?}", import.profile.credential_strategy),
                import.profile.remote_summary,
                import.profile.has_saved_credentials as i64,
                format!("{:?}", import.profile.validation_status),
            ],
        )?;

        connection.execute(
            "DELETE FROM profile_assets WHERE profile_id = ?1",
            params![import.profile.id.to_string()],
        )?;
        connection.execute(
            "DELETE FROM profile_validation_findings WHERE profile_id = ?1",
            params![import.profile.id.to_string()],
        )?;

        for asset in &import.assets {
            connection.execute(
                "INSERT INTO profile_assets (id, profile_id, kind, relative_path, sha256, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    asset.id.to_string(),
                    asset.profile_id.to_string(),
                    format!("{:?}", asset.kind),
                    asset.relative_path,
                    asset.sha256,
                    format!("{:?}", asset.origin),
                ],
            )?;
        }

        for finding in &import.findings {
            connection.execute(
                "INSERT INTO profile_validation_findings
                 (profile_id, directive, line, severity, message, action)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    import.profile.id.to_string(),
                    finding.directive,
                    finding.line as i64,
                    format!("{:?}", finding.severity),
                    finding.message,
                    format!("{:?}", finding.action),
                ],
            )?;
        }

        drop(connection);
        self.get_profile(&import.profile.id)
    }

    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT id, name, remote_summary, last_used_at, has_saved_credentials, validation_status
             FROM profiles ORDER BY name ASC",
        )?;

        let rows = statement.query_map([], map_profile_summary)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn get_profile(&self, profile_id: &ProfileId) -> Result<ProfileDetail, AppError> {
        let connection = self.connection.lock();
        let profile = connection
            .query_row(
                "SELECT id, name, source_filename, managed_dir, managed_ovpn_path, original_import_path,
                        created_at, updated_at, dns_intent_json, dns_policy, credential_mode, credential_strategy,
                        remote_summary, has_saved_credentials, validation_status, last_used_at
                 FROM profiles WHERE id = ?1",
                params![profile_id.to_string()],
                map_profile,
            )
            .optional()?
            .ok_or_else(|| AppError::ProfileNotFound(profile_id.to_string()))?;

        let mut asset_stmt = connection.prepare(
            "SELECT id, profile_id, kind, relative_path, sha256, origin
             FROM profile_assets WHERE profile_id = ?1 ORDER BY relative_path ASC",
        )?;
        let assets = asset_stmt
            .query_map(params![profile_id.to_string()], map_asset)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut findings_stmt = connection.prepare(
            "SELECT directive, line, severity, message, action
             FROM profile_validation_findings WHERE profile_id = ?1 ORDER BY line ASC",
        )?;
        let findings = findings_stmt
            .query_map(params![profile_id.to_string()], map_finding)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ProfileDetail {
            profile,
            assets,
            findings,
            saved_username: None,
            has_saved_pin_totp: false,
        })
    }

    fn update_has_saved_credentials(
        &self,
        profile_id: &ProfileId,
        has_saved_credentials: bool,
    ) -> Result<(), AppError> {
        self.connection.lock().execute(
            "UPDATE profiles SET has_saved_credentials = ?1 WHERE id = ?2",
            params![has_saved_credentials as i64, profile_id.to_string()],
        )?;
        Ok(())
    }

    fn touch_last_used(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        self.connection.lock().execute(
            "UPDATE profiles SET last_used_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), profile_id.to_string()],
        )?;
        Ok(())
    }

    fn update_profile_credential_strategy(
        &self,
        profile_id: &ProfileId,
        strategy: CredentialStrategy,
    ) -> Result<ProfileDetail, AppError> {
        self.connection.lock().execute(
            "UPDATE profiles SET credential_strategy = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                format!("{strategy:?}"),
                Utc::now().to_rfc3339(),
                profile_id.to_string()
            ],
        )?;
        self.get_profile(profile_id)
    }

    fn list_validation_findings(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ValidationFinding>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "SELECT directive, line, severity, message, action
             FROM profile_validation_findings WHERE profile_id = ?1 ORDER BY line ASC",
        )?;
        let rows = statement.query_map(params![profile_id.to_string()], map_finding)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn update_profile_dns_policy(
        &self,
        profile_id: &ProfileId,
        policy: DnsPolicy,
    ) -> Result<ProfileDetail, AppError> {
        self.connection.lock().execute(
            "UPDATE profiles SET dns_policy = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                dns_policy_to_string(&policy),
                Utc::now().to_rfc3339(),
                profile_id.to_string()
            ],
        )?;
        self.get_profile(profile_id)
    }

    fn delete_profile(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        let connection = self.connection.lock();
        let id_str = profile_id.to_string();
        connection.execute(
            "DELETE FROM profile_assets WHERE profile_id = ?1",
            params![id_str],
        )?;
        connection.execute(
            "DELETE FROM profile_validation_findings WHERE profile_id = ?1",
            params![id_str],
        )?;
        connection.execute(
            "DELETE FROM connection_history WHERE profile_id = ?1",
            params![id_str],
        )?;
        connection.execute("DELETE FROM profiles WHERE id = ?1", params![id_str])?;
        Ok(())
    }

    fn get_settings(&self) -> Result<crate::openvpn::runtime::Settings, AppError> {
        let connection = self.connection.lock();
        let value = connection
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;

        match value {
            Some(value) => serde_json::from_str(&value)
                .map_err(|error| AppError::Serialization(error.to_string())),
            None => Ok(crate::openvpn::runtime::Settings::default()),
        }
    }

    fn save_settings(&self, settings: &crate::openvpn::runtime::Settings) -> Result<(), AppError> {
        let value = serde_json::to_string(settings)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        self.connection.lock().execute(
            "INSERT INTO settings (key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![value],
        )?;
        Ok(())
    }

    fn set_last_selected_profile(&self, profile_id: Option<&ProfileId>) -> Result<(), AppError> {
        let connection = self.connection.lock();

        if let Some(profile_id) = profile_id {
            connection.execute(
                "INSERT INTO settings (key, value) VALUES ('last_selected_profile', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![profile_id.to_string()],
            )?;
        } else {
            connection.execute(
                "DELETE FROM settings WHERE key = 'last_selected_profile'",
                [],
            )?;
        }
        Ok(())
    }

    fn get_last_selected_profile(&self) -> Result<Option<ProfileId>, AppError> {
        let connection = self.connection.lock();
        let value = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'last_selected_profile'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();

        value
            .map(|value| {
                ProfileId::from_str(&value).map_err(|error| AppError::Settings(error.to_string()))
            })
            .transpose()
    }
}
