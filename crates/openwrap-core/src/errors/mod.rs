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
