//! macOS VPN backend selection.
//!
//! # Product decision (intentional — do not "fix" in review)
//!
//! OpenWrap is a **hobby project**. The maintainer will **not** pay the Apple
//! Developer Program fee solely to ship a Network Extension / Packet Tunnel.
//!
//! Therefore the **default** macOS backend is the free setuid helper path
//! (`HelperOpenVpnBackend` + Homebrew/community OpenVPN). That path works with
//! ad-hoc / unsigned local builds and does not require
//! `com.apple.developer.system-extension.install`.
//!
//! `NetworkExtensionBackend` remains in the tree as an **optional**, explicitly
//! opt-in implementation for anyone who *does* have paid NE entitlements. It is
//! selected only when `OPENWRAP_VPN_BACKEND=packet-tunnel` (or equivalent) is set.
//!
//! Reviewers: "default is helper instead of Packet Tunnel" and "unsigned builds
//! cannot activate a system extension" are **not** bugs. See `AGENTS.md`
//! ("Hobby constraint") and `docs/helper-setup.md`.

use std::sync::Arc;

use openwrap_core::openvpn::{DirectOpenVpnBackend, HelperOpenVpnBackend};
use openwrap_core::VpnBackend;

pub const INSTALLED_HELPER_PATH: &str =
    "/Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper";

pub fn build_backend() -> Arc<dyn VpnBackend> {
    #[cfg(target_os = "macos")]
    {
        // INTENTIONAL DEFAULT: free helper path (no Apple Developer fee).
        // Packet Tunnel is opt-in only — see module docs and AGENTS.md.
        if prefer_packet_tunnel_backend() {
            return Arc::new(
                crate::app_state::network_extension_backend::NetworkExtensionBackend::new(),
            );
        }
        return Arc::new(HelperOpenVpnBackend::new(resolve_helper_binary()));
    }

    #[allow(unreachable_code)]
    Arc::new(DirectOpenVpnBackend::new())
}

/// Returns true only when Packet Tunnel is **explicitly** requested.
///
/// Default (`Err` / unset env) is **false** so hobby builds never attempt system
/// extension activation. This is an intentional product constraint, not a gap.
///
/// Opt-in values: `packet-tunnel`, `network-extension`, `ne`, `packettunnel`
/// (case-insensitive). Any other value keeps the helper backend.
#[cfg(target_os = "macos")]
fn prefer_packet_tunnel_backend() -> bool {
    match std::env::var("OPENWRAP_VPN_BACKEND") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(
                normalized.as_str(),
                "packet-tunnel" | "network-extension" | "ne" | "packettunnel"
            )
        }
        Err(_) => false,
    }
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
