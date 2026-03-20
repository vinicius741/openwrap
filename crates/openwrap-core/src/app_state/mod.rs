use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::connection::SessionId;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub base_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub database_path: PathBuf,
}

impl AppPaths {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        Self {
            profiles_dir: base_dir.join("profiles"),
            runtime_dir: base_dir.join("runtime"),
            logs_dir: base_dir.join("logs"),
            database_path: base_dir.join("openwrap.sqlite3"),
            base_dir,
        }
    }

    pub fn failed_connection_log_path(&self) -> PathBuf {
        self.logs_dir.join("last-failed-openvpn.log")
    }

    pub fn sessions_logs_dir(&self) -> PathBuf {
        self.logs_dir.join("sessions")
    }

    /// Get the session log directory for a specific session.
    /// Uses the provided datetime for the date folder to ensure consistency.
    pub fn session_log_dir_at(&self, session_id: &SessionId, at: &chrono::DateTime<chrono::Utc>) -> PathBuf {
        let date = at.format("%Y-%m-%d").to_string();
        self.sessions_logs_dir()
            .join(&date)
            .join(format!("session-{}", session_id))
    }

    pub fn session_log_dir(&self, session_id: &SessionId) -> PathBuf {
        self.session_log_dir_at(session_id, &Utc::now())
    }

    pub fn ensure(&self) -> Result<(), AppError> {
        fs::create_dir_all(&self.base_dir)?;
        fs::create_dir_all(&self.profiles_dir)?;
        fs::create_dir_all(&self.runtime_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
        fs::create_dir_all(&self.sessions_logs_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn app_paths_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let paths = AppPaths::new(temp_dir.path());
        paths.ensure().unwrap();
        assert!(paths.base_dir.exists());
        assert!(paths.profiles_dir.exists());
        assert!(paths.runtime_dir.exists());
        assert!(paths.logs_dir.exists());
        assert!(paths.sessions_logs_dir().exists());
        assert!(paths.database_path.parent().unwrap().exists());
    }

    #[test]
    fn app_paths_failed_connection_log_path() {
        let temp_dir = TempDir::new().unwrap();
        let paths = AppPaths::new(temp_dir.path());
        let log_path = paths.failed_connection_log_path();
        assert_eq!(
            log_path,
            temp_dir.path().join("logs").join("last-failed-openvpn.log")
        );
    }

    #[test]
    fn app_paths_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("openwrap").join("data");
        let paths = AppPaths::new(&nested_path);
        assert_eq!(paths.base_dir, nested_path);
        assert_eq!(paths.profiles_dir, nested_path.join("profiles"));
        assert_eq!(paths.runtime_dir, nested_path.join("runtime"));
        assert_eq!(paths.logs_dir, nested_path.join("logs"));
        assert_eq!(paths.database_path, nested_path.join("openwrap.sqlite3"));
    }

    #[test]
    fn app_paths_ensure_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let paths = AppPaths::new(temp_dir.path());
        paths.ensure().unwrap();
        paths.ensure().unwrap();
        assert!(paths.base_dir.exists());
    }

    #[test]
    fn app_paths_session_log_dir_includes_date_and_session_id() {
        let temp_dir = TempDir::new().unwrap();
        let paths = AppPaths::new(temp_dir.path());
        let session_id = SessionId::new();
        let session_dir = paths.session_log_dir(&session_id);

        // Should contain "logs/sessions/YYYY-MM-DD/session-{id}"
        assert!(session_dir.starts_with(paths.sessions_logs_dir()));
        assert!(session_dir.to_string_lossy().contains("session-"));
        assert!(session_dir.to_string_lossy().contains(&session_id.to_string()));
    }

    #[test]
    fn app_paths_session_log_dir_at_uses_provided_time() {
        use chrono::{TimeZone, Utc};

        let temp_dir = TempDir::new().unwrap();
        let paths = AppPaths::new(temp_dir.path());
        let session_id = SessionId::new();

        // Use a specific date to verify the path uses it
        let specific_time = Utc.with_ymd_and_hms(2024, 3, 15, 10, 30, 0).unwrap();
        let session_dir = paths.session_log_dir_at(&session_id, &specific_time);

        // Should contain "logs/sessions/2024-03-15/session-{id}"
        assert!(session_dir.to_string_lossy().contains("2024-03-15"));
        assert!(session_dir.to_string_lossy().contains(&session_id.to_string()));
    }
}
