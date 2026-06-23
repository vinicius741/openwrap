pub mod backend_factory;
mod startup;

pub use backend_factory::build_backend;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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
    shutdown_started: AtomicBool,
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
            shutdown_started: AtomicBool::new(false),
        })
    }

    pub fn profile_repository(&self) -> Arc<dyn ProfileRepository> {
        self.repository.clone()
    }

    pub fn secret_store(&self) -> Arc<dyn SecretStore> {
        self.secret_store.clone()
    }

    /// Run connection-manager shutdown unconditionally and without panicking.
    ///
    /// This is reached from Tauri's RunEvent::Exit callback, which on macOS
    /// fires from tao's `application_will_terminate` — an `extern "C"` (i.e.
    /// `nounwind`) boundary on the main thread. Any panic here escalates to
    /// `panic_cannot_unwind` and aborts the process (SIGABRT). Wrapping the
    /// whole subtree in catch_unwind guarantees that a bug in cleanup can
    /// never crash termination; we log and move on so the OS can tear down.
    pub fn shutdown(&self) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let result = catch_unwind(AssertUnwindSafe(|| self.connection_manager.shutdown()));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("Warning: shutdown DNS reconciliation failed: {error}");
            }
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                eprintln!("Warning: shutdown panicked and was suppressed: {msg}");
            }
        }
    }
}
