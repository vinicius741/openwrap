//! SQLite-based repository implementation.
//!
//! This module provides a SQLite-backed implementation of the profile repository
//! and settings storage. The implementation is split across several submodules:
//!
//! - [`schema`] - Database migrations and schema management
//! - [`profile_queries`] - Profile-related database operations
//! - [`settings_queries`] - Settings-related database operations
//! - [`mappers`] - Row-to-domain object mapping functions
//! - [`codec`] - Enum serialization helpers

mod codec;
mod mappers;
mod profile_queries;
mod schema;
mod settings_queries;
#[cfg(test)]
mod tests;

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::openvpn::runtime::Settings;
use crate::profiles::repository::ProfileRepository;
use crate::profiles::{ProfileDetail, ProfileId, ProfileImportResult, ProfileSummary, ValidationFinding};


/// SQLite-backed repository for profiles and settings.
#[derive(Debug)]
pub struct SqliteRepository {
    connection: Mutex<Connection>,
}

impl SqliteRepository {
    /// Creates a new repository at the given path.
    ///
    /// This will create the database file if it doesn't exist and run
    /// any pending migrations.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let connection = Connection::open(path)?;
        let repository = Self {
            connection: Mutex::new(connection),
        };
        repository.run_migrations()?;
        Ok(repository)
    }

    /// Runs database migrations.
    fn run_migrations(&self) -> Result<(), AppError> {
        let connection = self.connection.lock();
        schema::migrate(&connection)
    }
}

impl ProfileRepository for SqliteRepository {
    fn save_import(&self, import: ProfileImportResult) -> Result<ProfileDetail, AppError> {
        let connection = self.connection.lock();
        profile_queries::save_import(&connection, &import)
    }

    fn list_profiles(&self) -> Result<Vec<ProfileSummary>, AppError> {
        let connection = self.connection.lock();
        profile_queries::list_profiles(&connection)
    }

    fn get_profile(&self, profile_id: &ProfileId) -> Result<ProfileDetail, AppError> {
        let connection = self.connection.lock();
        profile_queries::get_profile(&connection, profile_id)
    }

    fn update_has_saved_credentials(
        &self,
        profile_id: &ProfileId,
        has_saved_credentials: bool,
    ) -> Result<(), AppError> {
        let connection = self.connection.lock();
        profile_queries::update_has_saved_credentials(&connection, profile_id, has_saved_credentials)
    }

    fn touch_last_used(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        let connection = self.connection.lock();
        profile_queries::touch_last_used(&connection, profile_id)
    }

    fn get_settings(&self) -> Result<Settings, AppError> {
        let connection = self.connection.lock();
        settings_queries::get_settings(&connection)
    }

    fn save_settings(&self, settings: &Settings) -> Result<(), AppError> {
        let connection = self.connection.lock();
        settings_queries::save_settings(&connection, settings)
    }

    fn list_validation_findings(
        &self,
        profile_id: &ProfileId,
    ) -> Result<Vec<ValidationFinding>, AppError> {
        let connection = self.connection.lock();
        profile_queries::list_validation_findings(&connection, profile_id)
    }

    fn update_profile_dns_policy(
        &self,
        profile_id: &ProfileId,
        policy: DnsPolicy,
    ) -> Result<ProfileDetail, AppError> {
        let connection = self.connection.lock();
        profile_queries::update_profile_dns_policy(&connection, profile_id, policy)?;
        profile_queries::get_profile(&connection, profile_id)
    }

    fn set_last_selected_profile(&self, profile_id: Option<&ProfileId>) -> Result<(), AppError> {
        let connection = self.connection.lock();
        settings_queries::set_last_selected_profile(&connection, profile_id)
    }

    fn get_last_selected_profile(&self) -> Result<Option<ProfileId>, AppError> {
        let connection = self.connection.lock();
        settings_queries::get_last_selected_profile(&connection)
    }

    fn delete_profile(&self, profile_id: &ProfileId) -> Result<(), AppError> {
        let connection = self.connection.lock();
        profile_queries::delete_profile(&connection, profile_id)
    }
}
