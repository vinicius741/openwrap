use std::sync::Arc;

use openwrap_core::app_state::AppPaths;
use openwrap_core::connection::ConnectionManager;
use openwrap_core::openvpn::DirectOpenVpnBackend;
use openwrap_core::profiles::ProfileImporter;
use openwrap_core::secrets::KeychainSecretStore;
use openwrap_core::storage::sqlite::SqliteRepository;
use openwrap_core::{AppError, ProfileRepository};

pub struct AppState {
    pub repository: Arc<SqliteRepository>,
    pub importer: Arc<ProfileImporter>,
    pub connection_manager: Arc<ConnectionManager>,
}

impl AppState {
    pub fn new(base_dir: std::path::PathBuf) -> Result<Self, AppError> {
        let paths = AppPaths::new(base_dir);
        paths.ensure()?;

        let repository = Arc::new(SqliteRepository::new(&paths.database_path)?);
        let importer = Arc::new(ProfileImporter::new(paths.clone(), repository.clone()));
        let connection_manager = Arc::new(ConnectionManager::new(
            paths.clone(),
            repository.clone(),
            Arc::new(KeychainSecretStore::new()),
            Arc::new(DirectOpenVpnBackend::new()),
        ));

        Ok(Self {
            repository,
            importer,
            connection_manager,
        })
    }

    pub fn profile_repository(&self) -> Arc<dyn ProfileRepository> {
        self.repository.clone()
    }
}
