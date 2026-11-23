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
        authorize_caller, is_allowed_openvpn_location, read_owner_binding, validate_config_path,
        validate_openvpn_binary, OwnerBinding,
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
    fn state_files_are_never_read_or_written_through_hard_links() {
        let temp = tempdir().unwrap();
        let victim = temp.path().join("system-file");
        fs::write(&victim, "Wi-Fi\t1.1.1.1\n").unwrap();
        let state_file = temp.path().join("global.tsv");
        fs::hard_link(&victim, &state_file).unwrap();

        assert!(read_state_file(&state_file).is_err());
        assert!(reconcile_global_override(&state_file).is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "Wi-Fi\t1.1.1.1\n");
    }

    #[test]
    fn a_fifo_state_file_does_not_block_the_helper() {
        let temp = tempdir().unwrap();
        let state_file = temp.path().join("global.tsv");
        let path = std::ffi::CString::new(state_file.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

        // Returns at once instead of waiting for a writer.
        assert!(read_state_file(&state_file).is_err());
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

    fn no_console() -> Result<u32, String> {
        Err("console not consulted".into())
    }

    #[test]
    fn bound_helper_serves_only_the_installing_account() {
        assert!(authorize_caller(501, OwnerBinding::Bound(501), no_console).is_ok());
        // Another account at the screen is refused without asking the console.
        assert!(authorize_caller(502, OwnerBinding::Bound(501), || Ok(502)).is_err());
        assert!(authorize_caller(0, OwnerBinding::Bound(501), no_console).is_ok());
    }

    #[test]
    fn unbound_helper_falls_back_to_the_console_user() {
        assert!(authorize_caller(501, OwnerBinding::Unbound, || Ok(501)).is_ok());
        assert!(authorize_caller(502, OwnerBinding::Unbound, || Ok(501)).is_err());
        assert!(authorize_caller(501, OwnerBinding::Unbound, no_console).is_err());
    }

    #[test]
    fn reads_owner_binding_only_from_a_safe_file() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let me = unsafe { libc::getuid() };
        let temp = tempdir().unwrap();
        let owner = temp.path().join("helper.owner");

        assert_eq!(
            read_owner_binding(&owner, me).unwrap(),
            OwnerBinding::Unbound
        );

        fs::write(&owner, "501\n").unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_owner_binding(&owner, me).unwrap(),
            OwnerBinding::Bound(501)
        );
        // Wrong owner (the test does not run as this uid).
        assert!(read_owner_binding(&owner, me.wrapping_add(1)).is_err());

        fs::set_permissions(&owner, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(read_owner_binding(&owner, me).is_err());
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o644)).unwrap();

        fs::write(&owner, "not-a-uid\n").unwrap();
        assert!(read_owner_binding(&owner, me).is_err());

        let link = temp.path().join("link.owner");
        fs::write(&owner, "501\n").unwrap();
        symlink(&owner, &link).unwrap();
        assert!(read_owner_binding(&link, me).is_err());
    }
}
