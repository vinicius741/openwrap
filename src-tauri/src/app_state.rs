use std::sync::Arc;

use openwrap_core::app_state::AppPaths;
use openwrap_core::connection::ConnectionManager;
use openwrap_core::openvpn::{DirectOpenVpnBackend, HelperOpenVpnBackend};
use openwrap_core::profiles::ProfileImporter;
use openwrap_core::secrets::KeychainSecretStore;
use openwrap_core::storage::sqlite::SqliteRepository;
use openwrap_core::{AppError, ProfileRepository, SecretStore, VpnBackend};

pub struct AppState {
    pub repository: Arc<SqliteRepository>,
    pub importer: Arc<ProfileImporter>,
    pub connection_manager: Arc<ConnectionManager>,
    pub secret_store: Arc<KeychainSecretStore>,
    pub paths: AppPaths,
}

impl AppState {
    pub fn new(base_dir: std::path::PathBuf) -> Result<Self, AppError> {
        let paths = AppPaths::new(base_dir);
        paths.ensure()?;

        let repository = Arc::new(SqliteRepository::new(&paths.database_path)?);
        let importer = Arc::new(ProfileImporter::new(paths.clone(), repository.clone()));
        let secret_store = Arc::new(KeychainSecretStore::new());
        let backend = build_backend();
        #[cfg(target_os = "macos")]
        if let Err(error) = backend.reconcile_dns(openwrap_core::openvpn::ReconcileDnsRequest {
            runtime_root: paths.runtime_dir.clone(),
        }) {
            eprintln!("Failed to reconcile DNS state during startup: {error}");
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

fn build_backend() -> Arc<dyn VpnBackend> {
    #[cfg(target_os = "macos")]
    {
        return Arc::new(HelperOpenVpnBackend::new(resolve_helper_binary()));
    }

    #[allow(unreachable_code)]
    Arc::new(DirectOpenVpnBackend::new())
}

#[cfg(target_os = "macos")]
fn resolve_helper_binary() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("OPENWRAP_HELPER_PATH") {
        return path.into();
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let sibling = exe_dir.join("openwrap-helper");
            if sibling.exists() {
                return sibling;
            }
        }
    }

    std::path::PathBuf::from("openwrap-helper")
}
