use std::sync::Arc;

use openwrap_core::openvpn::{DirectOpenVpnBackend, HelperOpenVpnBackend};
use openwrap_core::VpnBackend;

pub fn build_backend() -> Arc<dyn VpnBackend> {
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
