pub mod app_state;
pub mod config;
pub mod connection;
pub mod dns;
pub mod errors;
pub mod openvpn;
pub mod profiles;
pub mod secrets;
pub mod storage;

pub use app_state::AppPaths;
pub use connection::{ConnectionManager, CoreEvent};
pub use errors::AppError;
pub use openvpn::runtime::detect_openvpn_binaries;
pub use profiles::repository::ProfileRepository;
pub use storage::sqlite::SqliteRepository;

pub trait SecretStore: Send + Sync {
    fn get_password(
        &self,
        profile_id: &profiles::ProfileId,
    ) -> Result<Option<secrets::StoredSecret>, errors::AppError>;

    fn set_password(&self, secret: secrets::StoredSecret) -> Result<(), errors::AppError>;

    fn delete_password(&self, profile_id: &profiles::ProfileId) -> Result<(), errors::AppError>;
}

pub trait VpnBackend: Send + Sync {
    fn connect(
        &self,
        request: openvpn::ConnectRequest,
    ) -> Result<openvpn::SpawnedSession, errors::AppError>;

    fn disconnect(&self, session_id: connection::SessionId) -> Result<(), errors::AppError>;
}
