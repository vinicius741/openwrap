mod backend_factory;
mod startup;

pub use backend_factory::build_backend;

use std::sync::Arc;

use openwrap_core::app_state::AppPaths;
use openwrap_core::connection::ConnectionManager;
use openwrap_core::profiles::ProfileImporter;
use openwrap_core::secrets::{CompositeSecretStore, KeychainSecretStore, LocalSecretStore};
use openwrap_core::storage::sqlite::SqliteRepository;
use openwrap_core::{AppError, ProfileRepository, SecretStore};

pub struct AppState {
    pub repository: Arc<SqliteRepository>,
    pub importer: Arc<ProfileImporter>,
    pub connection_manager: Arc<ConnectionManager>,
    pub secret_store: Arc<CompositeSecretStore>,
    pub paths: AppPaths,
}

impl AppState {
    pub fn new(base_dir: std::path::PathBuf) -> Result<Self, AppError> {
        let paths = AppPaths::new(base_dir);
        paths.ensure()?;

        let repository = Arc::new(SqliteRepository::new(&paths.database_path)?);
        let importer = Arc::new(ProfileImporter::new(paths.clone(), repository.clone()));
        let secret_store = Arc::new(CompositeSecretStore::new(
            KeychainSecretStore::new(),
            LocalSecretStore::new(&paths.secrets_database_path)?,
        ));
        let backend = build_backend();

        #[cfg(target_os = "macos")]
        if let Err(error) = startup::reconcile_dns(&backend, &paths) {
            eprintln!("Warning: DNS reconciliation failed (continuing anyway): {error}");
        }

        let connection_manager = Arc::new(ConnectionManager::new(
            paths.clone(),
            repository.clone(),
            secret_store.clone(),
            backend,
        ));

        Ok(Self {
            repository,
            importer,
            connection_manager,
            secret_store,
            paths,
        })
    }

    pub fn profile_repository(&self) -> Arc<dyn ProfileRepository> {
        self.repository.clone()
    }

    pub fn secret_store(&self) -> Arc<dyn SecretStore> {
        self.secret_store.clone()
    }
}
