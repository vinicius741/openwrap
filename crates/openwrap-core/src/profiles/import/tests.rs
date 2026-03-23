//! Integration tests for the profile import module.

use std::fs;
use std::sync::Arc;

use tempfile::TempDir;

use crate::app_state::AppPaths;
use crate::profiles::import::{ImportProfileRequest, ProfileImporter};
use crate::profiles::ImportStatus;
use crate::storage::sqlite::SqliteRepository;

#[test]
fn imports_profile_and_rewrites_assets() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("ca.crt"), "certificate").unwrap();
    fs::write(
        source_dir.join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\nca ca.crt\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository.clone());

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_some());
    let profile = response.profile.unwrap();
    assert!(profile.profile.managed_ovpn_path.exists());
    let stored = fs::read_to_string(&profile.profile.managed_ovpn_path).unwrap();
    assert!(stored.contains("ca assets/ca.crt"));
}

#[test]
fn blocks_missing_assets() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\nca missing.crt\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_none());
    assert_eq!(response.report.status, ImportStatus::Blocked);
    assert_eq!(response.report.missing_files.len(), 1);
    assert_eq!(response.report.errors.len(), 1);
}

#[test]
fn blocks_asset_path_traversal() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(source_dir.join("nested")).unwrap();
    fs::write(temp.path().join("escape.key"), "secret").unwrap();
    fs::write(
        source_dir.join("nested").join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\nkey ../escape.key\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("nested").join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_none());
    assert_eq!(response.report.status, ImportStatus::Blocked);
    assert!(response
        .report
        .errors
        .iter()
        .any(|error| error.contains("Path traversal detected")));
}

#[test]
fn blocks_duplicate_inline_and_file_assets() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("ca.crt"), "certificate").unwrap();
    fs::write(
        source_dir.join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\nca ca.crt\n<ca>\ninline\n</ca>\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_none());
    assert!(response
        .report
        .errors
        .iter()
        .any(|error| error.contains("defined as both inline content and a file asset")));
}

#[test]
fn blocks_non_dns_dhcp_options() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\ndhcp-option NTP pool.ntp.org\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_none());
    assert_eq!(response.report.status, ImportStatus::Blocked);
    assert!(response
        .report
        .errors
        .iter()
        .any(|error| error.contains("dhcp-option")));
}

#[test]
fn allows_domain_based_dhcp_options_with_warning_approval() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("sample.ovpn"),
        "client\nremote vpn.example.com 1194\ndhcp-option DNS 10.0.1.50\ndhcp-option DOMAIN corp.example\ndhcp-option DOMAIN-SEARCH corp.example lab.example\n",
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let response = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("sample.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(response.profile.is_some());
    let profile = response.profile.unwrap();
    assert_eq!(
        profile.profile.dns_intent,
        vec![
            "DNS 10.0.1.50".to_string(),
            "DOMAIN corp.example".to_string(),
            "DOMAIN-SEARCH corp.example lab.example".to_string(),
        ]
    );
}

#[test]
fn bmw_profile_requires_approval_but_is_importable() {
    let temp = TempDir::new().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("bmw.ovpn"),
        r#"dev tun
persist-tun
persist-key
data-ciphers AES-256-CBC:AES-256-GCM:AES-256-CFB
data-ciphers-fallback AES-256-CFB
auth SHA256
tls-client
client
resolv-retry infinite
remote aws-b.ilia.digital 1197 udp4
nobind
auth-user-pass
remote-cert-tls server
explicit-exit-notify
reneg-sec 0
pull-filter ignore redirect-gateway
dhcp-option DNS 10.0.1.50
route 160.52.107.21 255.255.255.255
setenv CLIENT_CERT 0
key-direction 1
<ca>
certificate
</ca>
<tls-auth>
static-key
</tls-auth>
"#,
    )
    .unwrap();

    let paths = AppPaths::new(temp.path().join("app"));
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let importer = ProfileImporter::new(paths, repository);

    let preview = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("bmw.ovpn"),
            display_name: None,
            allow_warnings: false,
        })
        .unwrap();

    assert!(preview.profile.is_none());
    assert_eq!(preview.report.status, ImportStatus::NeedsApproval);
    assert!(preview.report.errors.is_empty());
    assert_eq!(preview.report.warnings.len(), 3);

    let imported = importer
        .import_profile(ImportProfileRequest {
            source_path: source_dir.join("bmw.ovpn"),
            display_name: None,
            allow_warnings: true,
        })
        .unwrap();

    assert!(imported.profile.is_some());
    assert_eq!(imported.report.status, ImportStatus::Imported);
    assert!(imported.report.errors.is_empty());

    let profile = imported.profile.unwrap();
    let stored = fs::read_to_string(&profile.profile.managed_ovpn_path).unwrap();
    assert!(stored.contains("ca assets/ca.crt"));
    assert!(stored.contains("tls-auth assets/tls-auth.key"));
    assert!(stored.contains("auth-nocache"));
}
