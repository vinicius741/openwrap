#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use crate::reconcile::cleanup_transient_dns_files;
    use crate::reconcile::dns::{
        is_resolver_file_path, parse_global_override_state_line, read_state_file,
        reconcile_dns_routes, reconcile_global_override, reconcile_scoped_resolvers,
    };
    use crate::reconcile::processes::{extract_managed_openvpn_config, parse_ps_line};
    use crate::request::{
        is_allowed_openvpn_location, validate_config_path, validate_openvpn_binary,
    };
    use crate::system::normalize_networksetup_dns_output;

    #[test]
    fn accepts_runtime_launch_configs_anywhere_under_runtime_root() {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().join("OpenWrap");
        let profiles_dir = base_dir.join("profiles");
        let runtime_root = base_dir.join("runtime");
        let stale_runtime_dir = runtime_root.join("profile-a").join("stale-session");
        let active_runtime_dir = runtime_root.join("profile-a").join("active-session");
        let config_path = active_runtime_dir.join("profile.ovpn");

        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&stale_runtime_dir).unwrap();
        fs::create_dir_all(&active_runtime_dir).unwrap();
        fs::write(&config_path, "client\n").unwrap();

        assert!(validate_config_path(&config_path, &profiles_dir, &runtime_root).is_ok());
    }

    #[test]
    fn rejects_configs_outside_managed_roots() {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().join("OpenWrap");
        let profiles_dir = base_dir.join("profiles");
        let runtime_root = base_dir.join("runtime");
        let external_dir = temp.path().join("external");
        let config_path = external_dir.join("profile.ovpn");

        fs::create_dir_all(&profiles_dir).unwrap();
        fs::create_dir_all(&runtime_root).unwrap();
        fs::create_dir_all(&external_dir).unwrap();
        fs::write(&config_path, "client\n").unwrap();

        let error = validate_config_path(&config_path, &profiles_dir, &runtime_root).unwrap_err();
        assert!(error.contains("config path escapes the OpenWrap managed directory"));
    }

    #[test]
    fn normalizes_networksetup_empty_dns_output() {
        assert_eq!(
            normalize_networksetup_dns_output("There aren't any DNS Servers set on Wi-Fi.\n"),
            "__EMPTY__"
        );
    }

    #[test]
    fn normalizes_networksetup_dns_list() {
        assert_eq!(
            normalize_networksetup_dns_output("10.0.1.50\n10.0.1.51\n"),
            "10.0.1.50 10.0.1.51"
        );
    }

    #[test]
    fn extracts_managed_openvpn_config_under_runtime_root() {
        let runtime_root = Path::new("/Users/test/Library/Application Support/OpenWrap/runtime");
        let command = "/opt/homebrew/sbin/openvpn --config /Users/test/Library/Application Support/OpenWrap/runtime/profile-a/session/profile.ovpn --auth-nocache --verb 3";

        let extracted = extract_managed_openvpn_config(command, runtime_root).unwrap();
        assert_eq!(
            extracted,
            Path::new(
                "/Users/test/Library/Application Support/OpenWrap/runtime/profile-a/session/profile.ovpn"
            )
        );
    }

    #[test]
    fn ignores_openvpn_configs_outside_runtime_root() {
        let runtime_root = Path::new("/Users/test/Library/Application Support/OpenWrap/runtime");
        let command =
            "/opt/homebrew/sbin/openvpn --config /tmp/profile.ovpn --auth-nocache --verb 3";

        assert!(extract_managed_openvpn_config(command, runtime_root).is_none());
    }

    #[test]
    fn parses_ps_lines_with_variable_spacing() {
        let parsed = parse_ps_line("1     0 /sbin/launchd").unwrap();
        assert_eq!(parsed.0, 1);
        assert_eq!(parsed.1, 0);
        assert_eq!(parsed.2, "/sbin/launchd");
    }

    #[test]
    fn ignores_malformed_ps_lines() {
        assert!(parse_ps_line("garbage").is_none());
        assert!(parse_ps_line("123 onlypid").is_none());
    }

    #[test]
    fn removes_transient_dns_state_files() {
        let temp = tempdir().unwrap();
        let profile_dir = temp.path().join("dns-state").join("profile-a");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("global.tsv.targets.123"), "Wi-Fi\n").unwrap();
        fs::write(profile_dir.join("global.tsv.tmp"), "").unwrap();
        fs::write(profile_dir.join("global.tsv"), "Wi-Fi\t__EMPTY__\n").unwrap();

        cleanup_transient_dns_files(&profile_dir).unwrap();

        assert!(!profile_dir.join("global.tsv.targets.123").exists());
        assert!(!profile_dir.join("global.tsv.tmp").exists());
        assert!(profile_dir.join("global.tsv").exists());
    }

    #[test]
    fn rejects_corrupted_global_restore_state_lines() {
        assert!(parse_global_override_state_line(
            "OPENWRAP_DNS_DEBUG: active network devices: en0\t__EMPTY__"
        )
        .is_none());
        assert!(
            parse_global_override_state_line("Wi-Fi\tOPENWRAP_DNS_DEBUG: bad dns output").is_none()
        );
    }

    #[test]
    fn corrupted_global_restore_state_is_pruned() {
        let temp = tempdir().unwrap();
        let state_file = temp.path().join("global.tsv");
        fs::write(
            &state_file,
            "OPENWRAP_DNS_DEBUG: active network devices: en0\tOPENWRAP_DNS_DEBUG: not a service\n",
        )
        .unwrap();

        reconcile_global_override(&state_file).unwrap();

        assert!(!state_file.exists());
    }

    #[test]
    fn state_files_are_never_read_or_written_through_symlinks() {
        let temp = tempdir().unwrap();
        let secret = temp.path().join("root-only.txt");
        fs::write(&secret, "secret line\n").unwrap();
        let state_file = temp.path().join("global.tsv");
        std::os::unix::fs::symlink(&secret, &state_file).unwrap();

        assert!(read_state_file(&state_file).is_err());
        assert!(reconcile_global_override(&state_file).is_err());
        assert!(reconcile_scoped_resolvers(&state_file, "profile").is_err());
        assert_eq!(fs::read_to_string(&secret).unwrap(), "secret line\n");

        assert_eq!(
            read_state_file(&temp.path().join("missing.tsv")).unwrap(),
            None
        );
    }

    #[test]
    fn scoped_reconcile_only_removes_files_in_etc_resolver() {
        let temp = tempdir().unwrap();
        let victim = temp.path().join("victim");
        fs::write(&victim, "# OpenWrap managed DNS\n# profile_id=p1\n").unwrap();
        let state_file = temp.path().join("scoped.tsv");
        fs::write(&state_file, format!("corp.example\t{}\n", victim.display())).unwrap();

        let error = reconcile_scoped_resolvers(&state_file, "p1").unwrap_err();

        assert!(error.contains("not a file in /etc/resolver"));
        assert!(victim.exists());
        assert!(is_resolver_file_path(Path::new(
            "/etc/resolver/corp.example"
        )));
        assert!(!is_resolver_file_path(Path::new(
            "/etc/resolver/../sudoers"
        )));
        assert!(!is_resolver_file_path(Path::new("/etc/resolver")));
        assert!(!is_resolver_file_path(Path::new("/etc/resolver/a/b")));
    }

    #[test]
    fn route_reconcile_rejects_entries_that_are_not_ip_addresses() {
        let temp = tempdir().unwrap();
        let state_file = temp.path().join("dns-routes.tsv");
        fs::write(&state_file, "default\t\n10.0.0.53\t-ifscope en0\n").unwrap();

        let error = reconcile_dns_routes(&state_file).unwrap_err();

        assert_eq!(error.matches("is not an IP address").count(), 2);
    }

    #[test]
    fn only_known_openvpn_install_locations_are_launched() {
        assert!(is_allowed_openvpn_location(Path::new(
            "/opt/homebrew/sbin/openvpn"
        )));
        assert!(is_allowed_openvpn_location(Path::new(
            "/usr/local/sbin/openvpn"
        )));
        assert!(!is_allowed_openvpn_location(Path::new(
            "/Users/me/bin/openvpn"
        )));
        assert!(!is_allowed_openvpn_location(Path::new("/bin/sh")));
        let error = validate_openvpn_binary(Path::new("/tmp/openvpn")).unwrap_err();
        assert!(error.contains("not in a supported install location"));
    }
}
