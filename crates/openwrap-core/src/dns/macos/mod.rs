mod fragments;
mod paths;
mod render;

use std::fs;
use std::path::{Path, PathBuf};

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::ProfileId;

pub use paths::{
    bridge_dir, persistent_state_dir, quote_openvpn_arg, shell_single_quote,
    shell_single_quote_str, GLOBAL_STATE_NAME, ROUTE_STATE_NAME, SCOPED_STATE_NAME,
};

pub fn append_launch_config(
    config: &mut String,
    runtime_dir: &Path,
    profile_id: &ProfileId,
    dns_policy: &DnsPolicy,
) -> Result<Vec<PathBuf>, AppError> {
    if matches!(dns_policy, DnsPolicy::ObserveOnly) {
        return Ok(Vec::new());
    }

    let state_dir = paths::persistent_state_dir(runtime_dir, profile_id)?;
    let bridge_dir = paths::bridge_dir(runtime_dir);
    let up_script = bridge_dir.join(paths::UP_SCRIPT_NAME);
    let down_script = bridge_dir.join(paths::DOWN_SCRIPT_NAME);

    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&bridge_dir)?;

    let scoped_state = state_dir.join(paths::SCOPED_STATE_NAME);
    let global_state = state_dir.join(paths::GLOBAL_STATE_NAME);
    let route_state = state_dir.join(paths::ROUTE_STATE_NAME);

    let (up_script_body, down_script_body) = match dns_policy {
        DnsPolicy::SplitDnsPreferred => (
            render::render_scoped_up_script(&scoped_state, &global_state, &route_state, profile_id),
            render::render_scoped_down_script(
                &scoped_state,
                &global_state,
                &route_state,
                profile_id,
            ),
        ),
        DnsPolicy::FullOverride => (
            render::render_global_up_script(&global_state, &route_state),
            render::render_global_down_script(&global_state, &route_state),
        ),
        DnsPolicy::ObserveOnly => unreachable!(),
    };

    fs::write(&up_script, up_script_body)?;
    fs::write(&down_script, down_script_body)?;
    fragments::make_executable(&up_script)?;
    fragments::make_executable(&down_script)?;

    config.push_str("script-security 2\n");
    config.push_str(&format!(
        "route-up {}\n",
        paths::quote_openvpn_arg(&up_script)
    ));
    config.push_str(&format!(
        "route-pre-down {}\n",
        paths::quote_openvpn_arg(&down_script)
    ));
    config.push_str(&format!(
        "down {}\n",
        paths::quote_openvpn_arg(&down_script)
    ));
    config.push_str("down-pre\n");

    Ok(vec![bridge_dir])
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::dns::DnsPolicy;
    use crate::profiles::ProfileId;

    use super::append_launch_config;

    #[test]
    fn appends_scoped_runtime_dns_scripts_to_launch_config() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-split");
        let profile_id = ProfileId::new();
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &profile_id,
            &DnsPolicy::SplitDnsPreferred,
        )
        .unwrap();

        assert!(config.contains("script-security 2"));
        assert!(config.contains("route-up "));
        assert!(config.contains("route-pre-down "));
        assert!(config.contains("down "));
        assert!(config.contains("openwrap-dns-up.sh"));
        assert!(config.contains("openwrap-dns-down.sh"));
        assert_eq!(cleanup_paths.len(), 1);
        assert!(cleanup_paths[0].join("openwrap-dns-up.sh").exists());
        assert!(cleanup_paths[0].join("openwrap-dns-down.sh").exists());

        let up_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-up.sh")).unwrap();
        assert!(up_script.contains("collect_match_domains()"));
        assert!(up_script.contains("collect_search_domains()"));
        assert!(up_script.contains("OPENWRAP_DNS_DEBUG: $*\" >&2"));
        assert!(up_script.contains("OPENWRAP_DNS_ERROR: $*\" >&2"));
        assert!(up_script.contains("active network devices:"));
        assert!(up_script.contains("selected active service"));
        assert!(up_script.contains("observed VPN DNS servers:"));
        assert!(up_script.contains("printf 'search %s\\n' \"$domain\""));
        assert!(up_script.contains("apply_global_override()"));
        assert!(up_script.contains("AUTO_PROMOTED_FULL_OVERRIDE"));
        assert!(up_script.contains("if [ \"$current_dns\" = \"$desired_dns\" ]; then"));
        assert!(up_script.contains("\"$active_device_file\""));
        assert!(!up_script.contains("active_devices_file"));
    }

    #[test]
    fn appends_transactional_global_dns_scripts() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-full");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &ProfileId::new(),
            &DnsPolicy::FullOverride,
        )
        .unwrap();

        let up_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-up.sh")).unwrap();
        let down_script =
            std::fs::read_to_string(cleanup_paths[0].join("openwrap-dns-down.sh")).unwrap();
        assert!(up_script.contains("OPENWRAP_DNS_DEBUG: $*\" >&2"));
        assert!(up_script.contains("OPENWRAP_DNS_ERROR: $*\" >&2"));
        assert!(up_script.contains("verify_service_dns()"));
        assert!(up_script.contains("rollback_global_state_preserve()"));
        assert!(up_script.contains("active network devices:"));
        assert!(up_script.contains("selected active service"));
        assert!(up_script.contains("observed VPN DNS servers:"));
        assert!(up_script.contains("global DNS override applied successfully"));
        assert!(up_script.contains("if [ \"$current_dns\" = \"$dns_servers\" ]; then"));
        assert!(up_script.contains("\"$active_device_file\""));
        assert!(!up_script.contains("active_devices_file"));
        assert!(down_script.contains("RESTORE_PENDING_RECONCILE"));
    }

    #[test]
    fn skips_script_injection_for_observe_only_profiles() {
        let temp = tempdir().unwrap();
        let runtime_dir = temp
            .path()
            .join("runtime")
            .join("profile")
            .join("session-observe");
        std::fs::create_dir_all(&runtime_dir).unwrap();
        let mut config = String::from("client\n");

        let cleanup_paths = append_launch_config(
            &mut config,
            &runtime_dir,
            &ProfileId::new(),
            &DnsPolicy::ObserveOnly,
        )
        .unwrap();

        assert_eq!(config, "client\n");
        assert!(cleanup_paths.is_empty());
    }
}
