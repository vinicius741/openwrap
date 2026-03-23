//! Settings-related database queries.

use std::str::FromStr;

use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::AppError;
use crate::openvpn::runtime::Settings;
use crate::profiles::ProfileId;

/// Gets the application settings.
pub fn get_settings(connection: &Connection) -> Result<Settings, AppError> {
    let value = connection
        .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;

    match value {
        Some(value) => {
            serde_json::from_str(&value).map_err(|error| AppError::Serialization(error.to_string()))
        }
        None => Ok(Settings::default()),
    }
}

/// Saves the application settings.
pub fn save_settings(connection: &Connection, settings: &Settings) -> Result<(), AppError> {
    let value = serde_json::to_string(settings)
        .map_err(|error| AppError::Serialization(error.to_string()))?;
    connection.execute(
        "INSERT INTO settings (key, value) VALUES ('app', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![value],
    )?;
    Ok(())
}

/// Sets the last selected profile.
pub fn set_last_selected_profile(
    connection: &Connection,
    profile_id: Option<&ProfileId>,
) -> Result<(), AppError> {
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

/// Gets the last selected profile ID.
pub fn get_last_selected_profile(connection: &Connection) -> Result<Option<ProfileId>, AppError> {
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
