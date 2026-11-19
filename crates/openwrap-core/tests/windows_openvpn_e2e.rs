#![cfg(windows)]

//! End-to-end Windows validation against a real OpenVPN Community installation.
//!
//! Run on an elevated Windows machine (or `windows-latest` CI runner) with
//! OpenVPN 2.6+ installed:
//!   cargo test -p openwrap-core --test windows_openvpn_e2e -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use openwrap_core::app_state::AppPaths;
use openwrap_core::connection::{
    ConnectionManager, ConnectionState, CoreEvent, CredentialSubmission,
};
use openwrap_core::dns::DnsPolicy;
use openwrap_core::openvpn::DirectOpenVpnBackend;
use openwrap_core::profiles::{ImportProfileRequest, ImportStatus, ProfileImporter};
use openwrap_core::secrets::{KeychainSecretStore, StoredSecretKind};
use openwrap_core::storage::sqlite::SqliteRepository;
use openwrap_core::windows::{is_elevated, ExitEvent, ProcessHandle};
use openwrap_core::{detect_openvpn_binaries, ProfileRepository, SecretStore};
use tempfile::tempdir;
use tokio::sync::broadcast;
use tokio::time::timeout;

const TEST_USER: &str = "openwrap-test";
const TEST_PASS: &str = "correct-pass-123";
const PUSHED_ROUTE_PREFIX: &str = "198.51.100.0";
const PUSHED_DNS: &str = "10.200.200.1";
const PUSHED_DOMAIN: &str = "corp.openwrap.test";
const SERVER_PORT: u16 = 11947;

const SERVER_ADAPTER_NAME: &str = "OpenWrapTestServer";

struct ServerGuard {
    exit_event: ExitEvent,
    process: Option<ProcessHandle>,
    child: Child,
    tapctl_exe: Option<PathBuf>,
    adapter_name: Option<String>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.exit_event.signal();
        if let Some(process) = &self.process {
            process.wait_or_terminate(Duration::from_secs(5));
        }
        let _ = self.child.wait();
        if let (Some(tapctl), Some(name)) = (&self.tapctl_exe, &self.adapter_name) {
            let _ = Command::new(tapctl).args(["delete", name]).output();
        }
    }
}

fn find_openssl() -> PathBuf {
    let candidates = [
        PathBuf::from(r"C:\Program Files\OpenVPN\bin\openssl.exe"),
        PathBuf::from(r"C:\Program Files\Git\usr\bin\openssl.exe"),
        PathBuf::from(r"C:\Program Files\Git\mingw64\bin\openssl.exe"),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("openssl.exe")
}

fn run_cmd(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {program} {args:?}: {e}"));
    if !output.status.success() {
        panic!(
            "{program} {args:?} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run_powershell(script: &str) -> String {
    run_cmd("powershell.exe", &["-NoProfile", "-Command", script])
}

fn generate_pki(dir: &Path) {
    let openssl = find_openssl();
    let openssl_str = openssl.to_str().unwrap();

    let ca_ext = dir.join("ca.ext");
    fs::write(
        &ca_ext,
        "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n",
    )
    .unwrap();
    let server_ext = dir.join("server.ext");
    fs::write(
        &server_ext,
        "basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
    )
    .unwrap();
    let client_ext = dir.join("client.ext");
    fs::write(
        &client_ext,
        "basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=clientAuth\n",
    )
    .unwrap();

    // CA
    run_cmd(
        openssl_str,
        &[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-nodes",
            "-keyout",
            dir.join("ca.key").to_str().unwrap(),
            "-out",
            dir.join("ca.crt").to_str().unwrap(),
            "-days",
            "1",
            "-subj",
            "/CN=OpenWrapTestCA",
        ],
    );

    // Server
    run_cmd(
        openssl_str,
        &[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-nodes",
            "-keyout",
            dir.join("server.key").to_str().unwrap(),
            "-out",
            dir.join("server.csr").to_str().unwrap(),
            "-subj",
            "/CN=OpenWrapTestServer",
        ],
    );
    run_cmd(
        openssl_str,
        &[
            "x509",
            "-req",
            "-in",
            dir.join("server.csr").to_str().unwrap(),
            "-CA",
            dir.join("ca.crt").to_str().unwrap(),
            "-CAkey",
            dir.join("ca.key").to_str().unwrap(),
            "-CAcreateserial",
            "-out",
            dir.join("server.crt").to_str().unwrap(),
            "-days",
            "1",
            "-extfile",
            server_ext.to_str().unwrap(),
        ],
    );

    // Client
    run_cmd(
        openssl_str,
        &[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-nodes",
            "-keyout",
            dir.join("client.key").to_str().unwrap(),
            "-out",
            dir.join("client.csr").to_str().unwrap(),
            "-subj",
            "/CN=OpenWrapTestClient",
        ],
    );
    run_cmd(
        openssl_str,
        &[
            "x509",
            "-req",
            "-in",
            dir.join("client.csr").to_str().unwrap(),
            "-CA",
            dir.join("ca.crt").to_str().unwrap(),
            "-CAkey",
            dir.join("ca.key").to_str().unwrap(),
            "-CAcreateserial",
            "-out",
            dir.join("client.crt").to_str().unwrap(),
            "-days",
            "1",
            "-extfile",
            client_ext.to_str().unwrap(),
        ],
    );
}

fn create_server_adapter(openvpn_exe: &Path) -> (Option<PathBuf>, Option<String>, bool) {
    let tapctl = openvpn_exe
        .parent()
        .map(|dir| dir.join("tapctl.exe"))
        .filter(|path| path.is_file());
    let Some(tapctl) = tapctl else {
        println!(
            "[tapctl] tapctl.exe not found next to {}",
            openvpn_exe.display()
        );
        return (None, None, false);
    };

    if let Ok(list) = Command::new(&tapctl).arg("list").output() {
        println!(
            "[tapctl] adapters before:\n{}",
            String::from_utf8_lossy(&list.stdout)
        );
    }

    let _ = Command::new(&tapctl)
        .args(["delete", SERVER_ADAPTER_NAME])
        .output();

    for (hwid, is_tap) in [
        ("ovpn-dco", false),
        ("ovpn-dco-v2", false),
        ("root\\tap0901", true),
    ] {
        let output = Command::new(&tapctl)
            .args(["create", "--hwid", hwid, "--name", SERVER_ADAPTER_NAME])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                println!(
                    "[tapctl] created adapter {SERVER_ADAPTER_NAME} (hwid={hwid}): {}",
                    String::from_utf8_lossy(&out.stdout).trim()
                );
                if let Ok(list) = Command::new(&tapctl).arg("list").output() {
                    println!(
                        "[tapctl] adapters after:\n{}",
                        String::from_utf8_lossy(&list.stdout)
                    );
                }
                return (Some(tapctl), Some(SERVER_ADAPTER_NAME.into()), is_tap);
            }
            Ok(out) => {
                println!(
                    "[tapctl] create --hwid {hwid} failed ({:?}): {} {}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).trim(),
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
            Err(err) => {
                println!("[tapctl] failed to run tapctl create --hwid {hwid}: {err}");
            }
        }
    }

    panic!("Could not create a second tunnel adapter for the local OpenVPN test server");
}

fn start_local_server(openvpn_exe: &Path, server_dir: &Path) -> ServerGuard {
    let (tapctl_exe, adapter_name, server_uses_tap) = create_server_adapter(openvpn_exe);
    let verify_cmd = server_dir.join("verify-auth.cmd");
    fs::write(
        &verify_cmd,
        format!(
            "@echo off\r\nsetlocal\r\nset \"u=\"\r\nset \"p=\"\r\n(\r\n  set /p \"u=\"\r\n  set /p \"p=\"\r\n) < \"%~1\"\r\nif \"%u%\"==\"{TEST_USER}\" if \"%p%\"==\"{TEST_PASS}\" exit /b 0\r\nexit /b 1\r\n"
        ),
    )
    .unwrap();

    let server_log = server_dir.join("server.log");
    let server_conf = server_dir.join("server.ovpn");
    let quote = |p: &Path| format!("\"{}\"", p.to_string_lossy().replace('\\', "\\\\"));
    let mut driver_lines = String::new();
    if let Some(name) = &adapter_name {
        driver_lines.push_str(&format!("dev-node \"{name}\"\n"));
    }
    if server_uses_tap {
        driver_lines.push_str("disable-dco\n");
    }
    fs::write(
        &server_conf,
        format!(
            "\
mode server
tls-server
dev tun
{driver_lines}proto udp
local 127.0.0.1
port {SERVER_PORT}
topology subnet
server 10.200.200.0 255.255.255.0
dh none
ca {}
cert {}
key {}
script-security 2
auth-user-pass-verify {} via-file
push \"route {PUSHED_ROUTE_PREFIX} 255.255.255.0\"
push \"dhcp-option DNS {PUSHED_DNS}\"
push \"dhcp-option DOMAIN {PUSHED_DOMAIN}\"
keepalive 5 30
verb 3
log {}
",
            quote(&server_dir.join("ca.crt")),
            quote(&server_dir.join("server.crt")),
            quote(&server_dir.join("server.key")),
            quote(&verify_cmd),
            quote(&server_log),
        ),
    )
    .unwrap();

    let exit_event =
        ExitEvent::create(format!("openwrap-test-server-{}", uuid::Uuid::new_v4())).unwrap();
    let child = Command::new(openvpn_exe)
        .arg("--config")
        .arg(&server_conf)
        .arg("--service")
        .arg(exit_event.name())
        .arg("0")
        .current_dir(server_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn local OpenVPN test server");

    let process = ProcessHandle::open(child.id());
    let guard = ServerGuard {
        exit_event,
        process,
        child,
        tapctl_exe,
        adapter_name,
    };

    // Wait for server to finish creating its Wintun adapter and start listening.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(log) = fs::read_to_string(&server_log) {
            if log.contains("Initialization Sequence Completed") {
                println!("[server] ready:\n{log}");
                break;
            }
            if log.contains("Exiting due to fatal error") {
                panic!("OpenVPN test server failed to start:\n{log}");
            }
        }
        if Instant::now() > deadline {
            let log = fs::read_to_string(&server_log).unwrap_or_default();
            panic!("Timed out waiting for OpenVPN test server:\n{log}");
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    guard
}

async fn wait_for_state(
    manager: &ConnectionManager,
    rx: &mut broadcast::Receiver<CoreEvent>,
    target: ConnectionState,
    limit: Duration,
    collected_logs: &mut Vec<String>,
) -> openwrap_core::connection::ConnectionSnapshot {
    if manager.snapshot().state == target {
        return manager.snapshot();
    }
    timeout(limit, async {
        loop {
            match rx.recv().await {
                Ok(CoreEvent::LogLine(entry)) => {
                    println!("[client:{}] {}", entry.stream, entry.message);
                    collected_logs.push(entry.message);
                }
                Ok(CoreEvent::StateChanged(snapshot)) => {
                    println!(
                        "[state] {:?} substate={:?} err={:?}",
                        snapshot.state,
                        snapshot.substate,
                        snapshot.last_error.as_ref().map(|e| &e.code)
                    );
                    if snapshot.state == target {
                        return snapshot;
                    }
                    if target == ConnectionState::Connected
                        && snapshot.state == ConnectionState::Error
                    {
                        panic!(
                            "Connection failed while waiting for Connected: {:?}\nLogs:\n{}",
                            snapshot.last_error,
                            collected_logs.join("\n")
                        );
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if manager.snapshot().state == target {
                        return manager.snapshot();
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    panic!("CoreEvent channel closed unexpectedly");
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "Timed out waiting for state {:?}; current={:?}\nLogs:\n{}",
            target,
            manager.snapshot(),
            collected_logs.join("\n")
        )
    })
}

fn capture_routes() -> String {
    run_cmd("route", &["print", "-4"])
}

fn capture_dns_servers() -> String {
    run_powershell(
        "Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object { $_.ServerAddresses.Count -gt 0 } | ForEach-Object { \"$($_.InterfaceAlias): $($_.ServerAddresses -join ',')\" }",
    )
}

fn capture_nrpt_rules() -> String {
    run_powershell(
        "Get-DnsClientNrptRule | ForEach-Object { \"$($_.Namespace -join ',') => $($_.NameServers -join ',')\" }",
    )
}

#[tokio::test]
#[ignore = "requires Windows administrator privileges and installed OpenVPN Community"]
async fn windows_real_openvpn_end_to_end() {
    assert!(
        is_elevated(),
        "Windows E2E test must run from an elevated process"
    );

    // 1. OpenVPN detection (registry + Program Files) and custom path override.
    let detected = detect_openvpn_binaries(None);
    let openvpn_exe = detected
        .selected_path
        .clone()
        .expect("OpenVPN Community must be installed on the Windows test runner");
    println!(
        "[detect] selected={} discovered={:?}",
        openvpn_exe.display(),
        detected.discovered_paths
    );
    assert!(openvpn_exe.is_file());
    let override_detection = detect_openvpn_binaries(Some(openvpn_exe.clone()));
    assert_eq!(
        override_detection.selected_path.as_ref(),
        Some(&openvpn_exe)
    );
    let probed_version = openwrap_core::probe_openvpn(&openvpn_exe)
        .expect("probe_openvpn should parse version from real Windows openvpn.exe");
    println!("[probe] version={probed_version}");
    assert!(!probed_version.is_empty());

    // 2. Prepare temp directories, including spaces in the OpenWrap data path!
    let temp = tempdir().unwrap();
    let server_dir = temp.path().join("server");
    let import_dir = temp.path().join("import folder with spaces");
    let app_base_dir = temp.path().join("Test User Space").join("OpenWrap");
    fs::create_dir_all(&server_dir).unwrap();
    fs::create_dir_all(&import_dir).unwrap();

    generate_pki(&server_dir);
    for file in ["ca.crt", "client.crt", "client.key"] {
        fs::copy(server_dir.join(file), import_dir.join(file)).unwrap();
    }

    let client_ovpn = import_dir.join("windows-e2e.ovpn");
    fs::write(
        &client_ovpn,
        format!(
            "\
client
dev tun
proto udp
remote 127.0.0.1 {SERVER_PORT}
resolv-retry infinite
nobind
remote-cert-tls server
ca ca.crt
cert client.crt
key client.key
auth-user-pass
explicit-exit-notify 1
verb 3
"
        ),
    )
    .unwrap();

    // 3. Start local OpenVPN server.
    let _server = start_local_server(&openvpn_exe, &server_dir);

    // 4. Initialize OpenWrap core services using a space-containing AppPaths directory.
    let paths = AppPaths::new(app_base_dir.clone());
    paths.ensure().unwrap();
    let repository = Arc::new(SqliteRepository::new(&paths.database_path).unwrap());
    let secret_store = Arc::new(KeychainSecretStore::new());
    let backend = Arc::new(DirectOpenVpnBackend::new());
    let importer = ProfileImporter::new(paths.clone(), repository.clone());

    let imported = importer
        .import_profile(ImportProfileRequest {
            source_path: client_ovpn,
            display_name: Some("Windows E2E".into()),
            allow_warnings: true,
        })
        .unwrap();
    assert_eq!(imported.report.status, ImportStatus::Imported);
    let profile = imported.profile.expect("expected imported profile").profile;
    let profile_id = profile.id.clone();

    // Ensure Credential Manager entry is cleaned up even if an assertion fails.
    struct CredCleanup(openwrap_core::profiles::ProfileId);
    impl Drop for CredCleanup {
        fn drop(&mut self) {
            let _ = KeychainSecretStore::new().delete_password(&self.0);
        }
    }
    let _cred_cleanup = CredCleanup(profile_id.clone());

    let manager = ConnectionManager::new(
        paths.clone(),
        repository.clone(),
        secret_store.clone(),
        backend.clone(),
    );
    let mut rx = manager.subscribe();

    // Capture network state BEFORE connecting.
    let routes_before = capture_routes();
    let dns_before = capture_dns_servers();
    let nrpt_before = capture_nrpt_rules();
    println!("[before] DNS:\n{dns_before}\n[before] NRPT:\n{nrpt_before}");
    assert!(
        !routes_before.contains(PUSHED_ROUTE_PREFIX),
        "pushed route already present before connecting"
    );

    // -----------------------------------------------------------------------
    // Scenario 1: Wrong credentials -> AUTH_FAILED + auth file cleaned up
    // -----------------------------------------------------------------------
    let mut logs = Vec::new();
    let snap = manager.connect(profile_id.to_string()).await.unwrap();
    assert_eq!(snap.state, ConnectionState::AwaitingCredentials);

    manager
        .submit_credentials(CredentialSubmission {
            profile_id: profile_id.clone(),
            username: TEST_USER.into(),
            password: "wrong-password".into(),
            remember_in_keychain: false,
        })
        .await
        .unwrap();

    let failed = wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Error,
        Duration::from_secs(20),
        &mut logs,
    )
    .await;
    assert_eq!(
        failed.last_error.as_ref().map(|e| e.code.as_str()),
        Some("auth_failed"),
        "expected auth_failed error, got: {:?}",
        failed.last_error
    );
    assert!(
        !logs.is_empty(),
        "expected live stdout logs from openvpn.exe even with --service"
    );
    let profile_runtime_dir = paths.runtime_dir.join(profile_id.to_string());
    // auth.txt must already be removed immediately on AUTH_FAILED.
    if let Ok(entries) = fs::read_dir(&profile_runtime_dir) {
        for entry in entries.flatten() {
            let auth_candidate = entry.path().join("auth.txt");
            assert!(
                !auth_candidate.exists(),
                "auth.txt should be removed immediately after auth_failed: {}",
                auth_candidate.display()
            );
        }
    }
    // Wait for OpenVPN's explicit-exit-notify shutdown and exit-handler cleanup.
    let cleanup_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let cleaned = !profile_runtime_dir.exists()
            || fs::read_dir(&profile_runtime_dir)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true);
        if cleaned {
            break;
        }
        if Instant::now() > cleanup_deadline {
            let remaining: Vec<PathBuf> = fs::read_dir(&profile_runtime_dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            panic!(
                "runtime dir should be cleaned up after OpenVPN exits; remaining: {remaining:?}"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // -----------------------------------------------------------------------
    // Scenario 2: Valid connect (Split DNS), Remember username, live logs,
    // routes & DNS verification, graceful --service disconnect, reconnect
    // -----------------------------------------------------------------------
    logs.clear();
    repository
        .update_profile_dns_policy(&profile_id, DnsPolicy::SplitDnsPreferred)
        .unwrap();
    let snap = manager.connect(profile_id.to_string()).await.unwrap();
    assert_eq!(snap.state, ConnectionState::AwaitingCredentials);

    manager
        .submit_credentials(CredentialSubmission {
            profile_id: profile_id.clone(),
            username: TEST_USER.into(),
            password: TEST_PASS.into(),
            remember_in_keychain: true,
        })
        .await
        .unwrap();

    // Verify Windows Credential Manager entry exists and holds username ONLY.
    let saved = secret_store
        .get_password(&profile_id)
        .unwrap()
        .expect("expected saved username in Windows Credential Manager");
    assert_eq!(saved.kind, StoredSecretKind::UsernameOnly);
    assert_eq!(saved.username, TEST_USER);
    let cmdkey_out = run_cmd("cmdkey.exe", &["/list"]);
    let expected_target = format!("app.openwrap.credentials:profile-{profile_id}");
    assert!(
        cmdkey_out.contains(&expected_target),
        "cmdkey /list did not show {expected_target}:\n{cmdkey_out}"
    );

    let connected = wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Connected,
        Duration::from_secs(25),
        &mut logs,
    )
    .await;
    println!(
        "[connected] dns_observation={:?}",
        connected.dns_observation
    );
    assert!(
        logs.iter()
            .any(|line| line.contains("Initialization Sequence Completed")),
        "expected Initialization Sequence Completed in live stdout logs"
    );
    assert!(
        connected
            .dns_observation
            .runtime_pushed
            .iter()
            .any(|d| d.contains(PUSHED_DNS)),
        "expected pushed DNS in DnsObservation: {:?}",
        connected.dns_observation
    );

    // Check routes & DNS WHILE connected.
    let routes_connected = capture_routes();
    let dns_connected = capture_dns_servers();
    println!("[connected] DNS:\n{dns_connected}");
    assert!(
        routes_connected.contains(PUSHED_ROUTE_PREFIX),
        "pushed route {PUSHED_ROUTE_PREFIX} not found while connected:\n{routes_connected}"
    );
    assert!(
        dns_connected.contains(PUSHED_DNS),
        "pushed DNS {PUSHED_DNS} not found in Get-DnsClientServerAddress while connected:\n{dns_connected}"
    );

    // Graceful disconnect via --service exit event; measure elapsed time!
    let disconnect_started = Instant::now();
    manager.disconnect().await.unwrap();
    wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Idle,
        Duration::from_secs(15),
        &mut logs,
    )
    .await;
    let disconnect_elapsed = disconnect_started.elapsed();
    println!(
        "[disconnect] completed cleanly in {:.2}s",
        disconnect_elapsed.as_secs_f64()
    );
    assert!(
        disconnect_elapsed < Duration::from_secs(8),
        "disconnect took {:?} (looks like forced 10s termination instead of graceful --service exit!)",
        disconnect_elapsed
    );
    assert!(
        logs.iter()
            .any(|line| line.contains("SIGTERM") || line.contains("process exiting")),
        "expected graceful SIGTERM/process exiting log on disconnect; logs:\n{}",
        logs.join("\n")
    );

    // Verify routes and DNS returned to BEFORE state!
    tokio::time::sleep(Duration::from_millis(500)).await;
    let routes_after = capture_routes();
    let dns_after = capture_dns_servers();
    let nrpt_after = capture_nrpt_rules();
    println!("[after] DNS:\n{dns_after}\n[after] NRPT:\n{nrpt_after}");
    assert!(
        !routes_after.contains(PUSHED_ROUTE_PREFIX),
        "pushed route remained after disconnect:\n{routes_after}"
    );
    assert!(
        !dns_after.contains(PUSHED_DNS),
        "pushed DNS remained after disconnect:\n{dns_after}"
    );
    assert_eq!(nrpt_after, nrpt_before, "NRPT rules were not restored");

    // -----------------------------------------------------------------------
    // Scenario 3: FullOverride DNS policy (block-outside-dns / WFP) + reconnect
    // -----------------------------------------------------------------------
    logs.clear();
    repository
        .update_profile_dns_policy(&profile_id, DnsPolicy::FullOverride)
        .unwrap();

    let snap = manager.connect(profile_id.to_string()).await.unwrap();
    assert_eq!(snap.state, ConnectionState::AwaitingCredentials);
    manager
        .submit_credentials(CredentialSubmission {
            profile_id: profile_id.clone(),
            username: TEST_USER.into(),
            password: TEST_PASS.into(),
            remember_in_keychain: true,
        })
        .await
        .unwrap();

    wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Connected,
        Duration::from_secs(25),
        &mut logs,
    )
    .await;
    assert!(
        logs.iter()
            .any(|line| line.contains("Blocking DNS using WFP")
                || line.contains("block-outside-dns")),
        "expected WFP block-outside-dns log under FullOverride; logs:\n{}",
        logs.join("\n")
    );

    let disconnect_started = Instant::now();
    manager.disconnect().await.unwrap();
    wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Idle,
        Duration::from_secs(15),
        &mut logs,
    )
    .await;
    assert!(
        disconnect_started.elapsed() < Duration::from_secs(8),
        "FullOverride disconnect was not graceful"
    );

    // -----------------------------------------------------------------------
    // Scenario 4: ObserveOnly DNS policy (pull-filter ignore)
    // -----------------------------------------------------------------------
    logs.clear();
    repository
        .update_profile_dns_policy(&profile_id, DnsPolicy::ObserveOnly)
        .unwrap();

    manager.connect(profile_id.to_string()).await.unwrap();
    manager
        .submit_credentials(CredentialSubmission {
            profile_id: profile_id.clone(),
            username: TEST_USER.into(),
            password: TEST_PASS.into(),
            remember_in_keychain: true,
        })
        .await
        .unwrap();

    let observed = wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Connected,
        Duration::from_secs(25),
        &mut logs,
    )
    .await;
    let dns_observe_only = capture_dns_servers();
    println!("[observe-only] DNS:\n{dns_observe_only}");
    assert!(
        !dns_observe_only.contains(PUSHED_DNS),
        "ObserveOnly should prevent pushed DNS from being applied to Windows:\n{dns_observe_only}"
    );
    assert!(
        observed
            .dns_observation
            .runtime_pushed
            .iter()
            .any(|d| d.contains(PUSHED_DNS)),
        "ObserveOnly should still observe pushed DNS in logs: {:?}",
        observed.dns_observation
    );

    manager.disconnect().await.unwrap();
    wait_for_state(
        &manager,
        &mut rx,
        ConnectionState::Idle,
        Duration::from_secs(15),
        &mut logs,
    )
    .await;

    // -----------------------------------------------------------------------
    // Scenario 5: Credential deletion removes Credential Manager entry
    // -----------------------------------------------------------------------
    secret_store.delete_password(&profile_id).unwrap();
    assert!(secret_store.get_password(&profile_id).unwrap().is_none());
    let cmdkey_after = run_cmd("cmdkey.exe", &["/list"]);
    assert!(
        !cmdkey_after.contains(&expected_target),
        "cmdkey /list still contains deleted credential {expected_target}:\n{cmdkey_after}"
    );
}
