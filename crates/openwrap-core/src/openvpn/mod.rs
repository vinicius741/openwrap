pub mod backend;
pub mod direct_launcher;
pub mod helper_launcher;
pub mod helper_protocol;
pub mod runtime;

use std::path::{Path, PathBuf};

use crate::errors::AppError;

pub use backend::{BackendEvent, ConnectRequest, ReconcileDnsRequest, SpawnedSession};
pub use direct_launcher::DirectOpenVpnBackend;
pub use helper_launcher::HelperOpenVpnBackend;

pub fn config_working_dir(config_path: &Path) -> Result<PathBuf, AppError> {
    config_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            AppError::OpenVpnLaunch(format!(
                "config path has no parent directory: {}",
                config_path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::config_working_dir;

    #[test]
    fn returns_parent_directory_for_absolute_config_path() {
        let working_dir = config_working_dir(Path::new("/tmp/openwrap/profile.ovpn")).unwrap();
        assert_eq!(working_dir, Path::new("/tmp/openwrap"));
    }

    #[test]
    fn rejects_config_paths_without_parent_directory() {
        let error = config_working_dir(Path::new("profile.ovpn")).unwrap_err();
        assert!(error
            .to_string()
            .contains("config path has no parent directory"));
    }
}
