use std::sync::Arc;

use openwrap_core::openvpn::{DirectOpenVpnBackend, HelperOpenVpnBackend};
use openwrap_core::VpnBackend;

pub const BUNDLED_HELPER_NAME: &str = "openwrap-helper-bundled";
pub const INSTALLED_HELPER_PATH: &str =
    "/Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper";

pub fn build_backend() -> Arc<dyn VpnBackend> {
    #[cfg(target_os = "macos")]
    {
        return Arc::new(HelperOpenVpnBackend::new(resolve_helper_binary()));
    }

    #[allow(unreachable_code)]
    Arc::new(DirectOpenVpnBackend::new())
}

#[cfg(target_os = "macos")]
pub fn resolve_helper_binary() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("OPENWRAP_HELPER_PATH") {
        return path.into();
    }

    installed_helper_path()
}

pub fn installed_helper_path() -> std::path::PathBuf {
    std::path::PathBuf::from(INSTALLED_HELPER_PATH)
}
