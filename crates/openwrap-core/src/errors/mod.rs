use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("validation failed")]
    Validation {
        title: String,
        message: String,
        directive: Option<String>,
        line: Option<usize>,
    },

    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    #[error("settings error: {0}")]
    Settings(String),

    #[error("openvpn binary not found")]
    OpenVpnBinaryNotFound,

    #[error("openvpn launch failed: {0}")]
    OpenVpnLaunch(String),

    #[error("keychain error: {0}")]
    Keychain(String),

    #[error("connection state error: {0}")]
    ConnectionState(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("unsupported absolute path: {0}")]
    UnsupportedAbsolutePath(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFacingError {
    pub code: String,
    pub title: String,
    pub message: String,
    pub suggested_fix: Option<String>,
    pub details_safe: Option<String>,
}

impl From<&AppError> for UserFacingError {
    fn from(value: &AppError) -> Self {
        match value {
            AppError::Validation {
                title,
                message,
                directive,
                line,
            } => Self {
                code: "validation_failed".into(),
                title: title.clone(),
                message: message.clone(),
                suggested_fix: directive.as_ref().map(|directive| {
                    format!("Remove or change the unsupported '{directive}' directive.")
                }),
                details_safe: line.map(|line| format!("line {line}")),
            },
            AppError::OpenVpnBinaryNotFound => Self {
                code: "openvpn_not_found".into(),
                title: "OpenVPN not found".into(),
                message: "OpenWrap could not find the OpenVPN community binary.".into(),
                suggested_fix: Some(
                    "Install OpenVPN via Homebrew or set a custom binary path in Settings.".into(),
                ),
                details_safe: None,
            },
            AppError::OpenVpnLaunch(message) => Self {
                code: "openvpn_launch_failed".into(),
                title: "OpenVPN failed to start".into(),
                message: message.clone(),
                suggested_fix: Some(
                    "Check the selected OpenVPN binary path and profile validity.".into(),
                ),
                details_safe: None,
            },
            AppError::Keychain(message) => Self {
                code: "keychain_failed".into(),
                title: "Keychain access failed".into(),
                message: message.clone(),
                suggested_fix: Some("Review macOS Keychain permissions for OpenWrap.".into()),
                details_safe: None,
            },
            AppError::Settings(message) => Self {
                code: "settings_error".into(),
                title: "Settings error".into(),
                message: message.clone(),
                suggested_fix: None,
                details_safe: None,
            },
            AppError::ConnectionState(message) => Self {
                code: "connection_error".into(),
                title: "Connection error".into(),
                message: message.clone(),
                suggested_fix: None,
                details_safe: None,
            },
            other => Self {
                code: "internal_error".into(),
                title: "Unexpected error".into(),
                message: other.to_string(),
                suggested_fix: None,
                details_safe: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_facing_error_from_validation() {
        let error = AppError::Validation {
            title: "Invalid config".to_string(),
            message: "Directive not allowed".to_string(),
            directive: Some("script-security".to_string()),
            line: Some(10),
        };
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "validation_failed");
        assert_eq!(user_error.title, "Invalid config");
        assert!(user_error.suggested_fix.is_some());
        assert!(user_error.details_safe.is_some());
    }

    #[test]
    fn user_facing_error_from_openvpn_not_found() {
        let error = AppError::OpenVpnBinaryNotFound;
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "openvpn_not_found");
        assert!(user_error.suggested_fix.is_some());
    }

    #[test]
    fn user_facing_error_from_openvpn_launch() {
        let error = AppError::OpenVpnLaunch("Failed to start".to_string());
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "openvpn_launch_failed");
        assert!(user_error.suggested_fix.is_some());
    }

    #[test]
    fn user_facing_error_from_keychain() {
        let error = AppError::Keychain("Access denied".to_string());
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "keychain_failed");
        assert!(user_error.suggested_fix.is_some());
    }

    #[test]
    fn user_facing_error_from_profile_not_found() {
        let error = AppError::ProfileNotFound("test-profile".to_string());
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "internal_error");
        assert_eq!(user_error.title, "Unexpected error");
    }

    #[test]
    fn user_facing_error_from_settings() {
        let error = AppError::Settings("Invalid setting".to_string());
        let user_error = UserFacingError::from(&error);
        assert_eq!(user_error.code, "settings_error");
    }

    #[test]
    fn user_facing_error_serialization() {
        let error = AppError::OpenVpnBinaryNotFound;
        let user_error = UserFacingError::from(&error);
        let json = serde_json::to_string(&user_error).unwrap();
        let roundtrip: UserFacingError = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.code, user_error.code);
        assert_eq!(roundtrip.title, user_error.title);
    }

    #[test]
    fn app_error_display() {
        let error = AppError::ProfileNotFound("my-profile".to_string());
        assert!(error.to_string().contains("my-profile"));
    }

    #[test]
    fn app_error_io() {
        let error = AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(error.to_string().contains("io error"));
    }

    #[test]
    fn app_error_unsupported_absolute_path() {
        let path = std::path::PathBuf::from("/etc/passwd");
        let error = AppError::UnsupportedAbsolutePath(path.clone());
        assert!(error.to_string().contains("/etc/passwd"));
    }
}
