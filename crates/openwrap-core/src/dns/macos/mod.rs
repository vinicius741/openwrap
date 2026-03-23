mod paths;
mod render;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::dns::DnsPolicy;
use crate::errors::AppError;
use crate::profiles::ProfileId;

pub use paths::{bridge_dir, persistent_state_dir, quote_openvpn_arg};
pub use render::{
    render_global_down_script, render_global_up_script, render_scoped_down_script,
    render_scoped_up_script, render_scripts,
};

const UP_SCRIPT_NAME: &str = "openwrap-dns-up.sh";
const DOWN_SCRIPT_NAME: &str = "openwrap-dns-down.sh";
const GLOBAL_STATE_NAME: &str = "global.tsv";
const SCOPED_STATE_NAME: &str = "scoped.tsv";
const ROUTE_STATE_NAME: &str = "dns-routes.tsv";

pub fn append_launch_config(
    config: &mut String,
    runtime_dir: &Path,
    profile_id: &ProfileId,
    dns_policy: &DnsPolicy,
) -> Result<Vec<PathBuf>, AppError> {
    if matches!(dns_policy, DnsPolicy::ObserveOnly) {
        return Ok(Vec::new());
    }

    let state_dir = persistent_state_dir(runtime_dir, profile_id)?;
    let br_dir = bridge_dir(runtime_dir);
    let up_script = br_dir.join(UP_SCRIPT_NAME);
    let down_script = br_dir.join(DOWN_SCRIPT_NAME);

    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&br_dir)?;

    let scoped_state = state_dir.join(SCOPED_STATE_NAME);
    let global_state = state_dir.join(GLOBAL_STATE_NAME);
    let route_state = state_dir.join(ROUTE_STATE_NAME);

    let (up_script_body, down_script_body) = render::render_scripts(
        dns_policy,
        &scoped_state,
        &global_state,
        &route_state,
        profile_id,
    );

    fs::write(&up_script, &up_script_body)?;
    fs::write(&down_script, &down_script_body)?;
    make_executable(&up_script)?;
    make_executable(&down_script)?;

    config.push_str("script-security 2\n");
    config.push_str(&format!("route-up {}\n", quote_openvpn_arg(&up_script)));
    config.push_str(&format!(
        "route-pre-down {}\n",
        quote_openvpn_arg(&down_script)
    ));
    config.push_str(&format!("down {}\n", quote_openvpn_arg(&down_script)));
    config.push_str("down-pre\n");

    Ok(vec![br_dir])
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), AppError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), AppError> {
    Ok(())
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
