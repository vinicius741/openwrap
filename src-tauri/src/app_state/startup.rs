use std::sync::Arc;

use openwrap_core::app_state::AppPaths;
use openwrap_core::VpnBackend;

#[cfg(target_os = "macos")]
pub fn reconcile_dns(
    backend: &Arc<dyn VpnBackend>,
    paths: &AppPaths,
) -> Result<(), openwrap_core::AppError> {
    backend
        .reconcile_dns(openwrap_core::openvpn::ReconcileDnsRequest {
            runtime_root: paths.runtime_dir.clone(),
        })
        .map_err(|error| {
            eprintln!("Failed to reconcile DNS state during startup: {error}");
            error
        })
}

#[cfg(not(target_os = "macos"))]
pub fn reconcile_dns(
    _backend: &Arc<dyn VpnBackend>,
    _paths: &AppPaths,
) -> Result<(), openwrap_core::AppError> {
    Ok(())
}
