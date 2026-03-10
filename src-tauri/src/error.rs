use serde::Serialize;

use openwrap_core::errors::{AppError, UserFacingError};

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub message: String,
    pub user_facing: UserFacingError,
}

impl From<AppError> for CommandError {
    fn from(value: AppError) -> Self {
        let user_facing = UserFacingError::from(&value);
        Self {
            message: value.to_string(),
            user_facing,
        }
    }
}

