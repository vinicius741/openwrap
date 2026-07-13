//! Optional Packet Tunnel / Network Extension VPN backend.
//!
//! # Not the default (intentional)
//!
//! This module is **not** selected by default. OpenWrap is a hobby project and
//! the maintainer will not pay the Apple Developer Program fee for Network
//! Extension entitlements. The default backend is the free setuid helper — see
//! `backend_factory.rs` and `AGENTS.md` ("Hobby constraint").
//!
//! Activate only with `OPENWRAP_VPN_BACKEND=packet-tunnel` **and** a signed,
//! provisioned app that embeds the system extension. Unsigned builds correctly
//! fail activation with a missing `system-extension.install` entitlement; that
//! is expected, not a regression.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{c_char, c_uchar, CStr, CString};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use openwrap_core::connection::SessionId;
use openwrap_core::dns::DnsPolicy;
use openwrap_core::openvpn::{BackendEvent, ConnectRequest, SpawnedSession, VpnBackendMode};
use openwrap_core::{AppError, VpnBackend};
use serde::Serialize;
use tokio::sync::mpsc;
use zeroize::{Zeroize, Zeroizing};

const PROVIDER_BUNDLE_ID: &str = "app.openwrap.desktop.PacketTunnel";
const STOP_EVENT_GRACE_PERIOD: Duration = Duration::from_secs(10);

type EventSender = mpsc::UnboundedSender<BackendEvent>;

fn sessions() -> &'static Mutex<HashMap<String, EventSender>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, EventSender>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketTunnelPayload {
    config: String,
    /// Relative asset paths (`assets/ca.crt`) mapped to base64-encoded file bytes.
    assets: BTreeMap<String, String>,
    username: Option<String>,
    password: Option<String>,
    dns_policy: &'static str,
    dns_intent: Vec<String>,
}

impl Drop for PacketTunnelPayload {
    fn drop(&mut self) {
        self.config.zeroize();
        for value in self.assets.values_mut() {
            value.zeroize();
        }
        if let Some(username) = self.username.as_mut() {
            username.zeroize();
        }
        if let Some(password) = self.password.as_mut() {
            password.zeroize();
        }
    }
}

pub struct NetworkExtensionBackend;

impl NetworkExtensionBackend {
    pub fn new() -> Self {
        Self
    }
}

impl VpnBackend for NetworkExtensionBackend {
    fn mode(&self) -> VpnBackendMode {
        VpnBackendMode::PacketTunnel
    }

    fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError> {
        let payload = build_payload(&request)?;
        let payload = Zeroizing::new(
            serde_json::to_vec(&payload)
                .map_err(|error| AppError::Serialization(error.to_string()))?,
        );
        let session_id = request.session_id.to_string();
        let c_session_id = CString::new(session_id.as_str())
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let c_provider_id = CString::new(PROVIDER_BUNDLE_ID).expect("static bundle id is valid");
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        sessions()
            .lock()
            .map_err(|_| AppError::ConnectionState("session registry is poisoned".into()))?
            .insert(session_id.clone(), event_tx.clone());
        let _ = event_tx.send(BackendEvent::Started(None));

        let accepted = unsafe {
            openwrap_ne_start(
                c_session_id.as_ptr(),
                c_provider_id.as_ptr(),
                payload.as_ptr(),
                payload.len(),
                network_extension_event,
            )
        };
        if !accepted {
            sessions()
                .lock()
                .ok()
                .and_then(|mut map| map.remove(&session_id));
            return Err(AppError::OpenVpnLaunch(
                "macOS rejected the Packet Tunnel activation request".into(),
            ));
        }

        Ok(SpawnedSession {
            session_id: request.session_id,
            pid: None,
            event_rx,
        })
    }

    fn disconnect(&self, session_id: SessionId) -> Result<(), AppError> {
        let value = session_id.to_string();
        let c_session_id = CString::new(value.as_str())
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let accepted = unsafe { openwrap_ne_stop(c_session_id.as_ptr()) };
        if !accepted {
            complete_disconnect_if_registered(&value, false);
        } else {
            let fallback_session_id = value.clone();
            let fallback = thread::Builder::new()
                .name("openwrap-ne-stop-fallback".into())
                .spawn(move || {
                    thread::sleep(STOP_EVENT_GRACE_PERIOD);
                    complete_disconnect_if_registered(&fallback_session_id, true);
                });
            if fallback.is_err() {
                complete_disconnect_if_registered(&value, true);
            }
        }
        Ok(())
    }
}

fn build_payload(request: &ConnectRequest) -> Result<PacketTunnelPayload, AppError> {
    let config = fs::read_to_string(&request.config_path)?;
    let assets = read_assets(&request.runtime_dir.join("assets"))?;
    let (username, password) = match request.auth_file.as_deref() {
        Some(path) => read_auth_file(path)?,
        None => (None, None),
    };
    Ok(PacketTunnelPayload {
        config,
        assets,
        username,
        password,
        dns_policy: dns_policy_label(&request.dns_policy),
        dns_intent: request.dns_intent.clone(),
    })
}

fn dns_policy_label(policy: &DnsPolicy) -> &'static str {
    match policy {
        DnsPolicy::SplitDnsPreferred => "splitDnsPreferred",
        DnsPolicy::FullOverride => "fullOverride",
        DnsPolicy::ObserveOnly => "observeOnly",
    }
}

fn read_assets(path: &Path) -> Result<BTreeMap<String, String>, AppError> {
    let mut assets = BTreeMap::new();
    if !path.exists() {
        return Ok(assets);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| AppError::OpenVpnLaunch("asset name is not valid UTF-8".into()))?;
        let bytes = fs::read(entry.path())?;
        assets.insert(format!("assets/{name}"), BASE64.encode(bytes));
    }
    Ok(assets)
}

fn read_auth_file(path: &Path) -> Result<(Option<String>, Option<String>), AppError> {
    let mut content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let credentials = (
        lines.next().map(ToOwned::to_owned),
        lines.next().map(ToOwned::to_owned),
    );
    content.zeroize();
    Ok(credentials)
}

fn complete_disconnect_if_registered(session_id: &str, timed_out: bool) {
    let Ok(mut registry) = sessions().lock() else {
        return;
    };
    let Some(sender) = registry.remove(session_id) else {
        return;
    };
    if timed_out {
        let _ = sender.send(BackendEvent::Stdout(
            "Packet Tunnel stop confirmation timed out; completing local disconnect".into(),
        ));
    }
    let _ = sender.send(BackendEvent::Exited(Some(0)));
}

/// Map a native bridge event into manager-facing backend events.
///
/// Returns `(events, terminal)` where `terminal` means the session registry
/// entry should be removed after delivery.
fn map_bridge_event(event: i32, message: String) -> (Vec<BackendEvent>, bool) {
    match event {
        0 => (vec![BackendEvent::Stdout(message)], false),
        1 => (
            vec![BackendEvent::Stdout(
                "Packet Tunnel provider is connecting".into(),
            )],
            false,
        ),
        2 => (
            // Preserve the manager's existing, well-tested connection-state signal.
            vec![BackendEvent::Stdout(
                "Initialization Sequence Completed".into(),
            )],
            false,
        ),
        3 => {
            // User-requested or clean provider stop (bridge distinguishes failures as event 4).
            let mut events = Vec::new();
            if !message.is_empty() {
                events.push(BackendEvent::Stdout(message));
            }
            events.push(BackendEvent::Exited(Some(0)));
            (events, true)
        }
        4 => {
            let detail = if message.is_empty() {
                "Packet Tunnel failed".into()
            } else {
                message
            };
            (
                vec![
                    BackendEvent::Stderr(detail),
                    BackendEvent::Exited(Some(1)),
                ],
                true,
            )
        }
        5 => (
            vec![BackendEvent::Stdout(
                "macOS is waiting for approval of the OpenWrap system extension".into(),
            )],
            false,
        ),
        _ => (Vec::new(), false),
    }
}

extern "C" fn network_extension_event(
    session_id: *const c_char,
    event: i32,
    message: *const c_char,
) {
    if session_id.is_null() {
        return;
    }
    let session_id = unsafe { CStr::from_ptr(session_id) }
        .to_string_lossy()
        .into_owned();
    let message = if message.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };

    let (events, terminal) = map_bridge_event(event, message);

    let Ok(mut registry) = sessions().lock() else {
        return;
    };
    let Some(sender) = registry.get(&session_id).cloned() else {
        return;
    };
    for event in events {
        let _ = sender.send(event);
    }
    if terminal {
        registry.remove(&session_id);
    }
}

type NativeEventCallback = extern "C" fn(*const c_char, i32, *const c_char);

unsafe extern "C" {
    fn openwrap_ne_start(
        session_id: *const c_char,
        provider_bundle_id: *const c_char,
        payload: *const c_uchar,
        payload_len: usize,
        callback: NativeEventCallback,
    ) -> bool;
    fn openwrap_ne_stop(session_id: *const c_char) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use openwrap_core::connection::SessionId;
    use openwrap_core::dns::DnsPolicy;
    use openwrap_core::profiles::ProfileId;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn dns_policy_labels_match_provider_contract() {
        assert_eq!(
            dns_policy_label(&DnsPolicy::SplitDnsPreferred),
            "splitDnsPreferred"
        );
        assert_eq!(dns_policy_label(&DnsPolicy::FullOverride), "fullOverride");
        assert_eq!(dns_policy_label(&DnsPolicy::ObserveOnly), "observeOnly");
    }

    #[test]
    fn read_assets_encodes_base64_and_prefixes_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let asset_path = dir.path().join("ca.crt");
        fs::write(&asset_path, b"CERT-BYTES").expect("write asset");

        let assets = read_assets(dir.path()).expect("read assets");
        assert_eq!(assets.len(), 1);
        let encoded = assets.get("assets/ca.crt").expect("asset key");
        assert_eq!(BASE64.decode(encoded).expect("decode"), b"CERT-BYTES");
    }

    #[test]
    fn build_payload_splits_auth_and_maps_dns_policy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_path = dir.path().join("profile.ovpn");
        fs::write(&config_path, "client\nremote example.com 1194\n").expect("config");
        let assets_dir = dir.path().join("assets");
        fs::create_dir_all(&assets_dir).expect("assets dir");
        fs::write(assets_dir.join("ca.crt"), b"CA").expect("ca");
        let auth_path = dir.path().join("auth.txt");
        fs::write(&auth_path, "alice\nsecret\n").expect("auth");

        let request = ConnectRequest {
            session_id: SessionId(Uuid::nil()),
            profile_id: ProfileId(Uuid::nil()),
            openvpn_binary: PathBuf::from("/usr/sbin/openvpn"),
            config_path,
            auth_file: Some(auth_path),
            runtime_dir: dir.path().to_path_buf(),
            dns_policy: DnsPolicy::FullOverride,
            dns_intent: vec!["DNS 1.1.1.1".into()],
        };

        let payload = build_payload(&request).expect("payload");
        assert!(payload.config.contains("remote example.com"));
        assert_eq!(payload.username.as_deref(), Some("alice"));
        assert_eq!(payload.password.as_deref(), Some("secret"));
        assert_eq!(payload.dns_policy, "fullOverride");
        assert_eq!(payload.dns_intent, vec!["DNS 1.1.1.1".to_string()]);
        assert_eq!(
            BASE64
                .decode(payload.assets.get("assets/ca.crt").expect("ca asset"))
                .expect("decode"),
            b"CA"
        );
    }

    #[test]
    fn map_bridge_event_connected_uses_initialization_signal() {
        let (events, terminal) = map_bridge_event(2, String::new());
        assert!(!terminal);
        assert_eq!(
            events,
            vec![BackendEvent::Stdout(
                "Initialization Sequence Completed".into()
            )]
        );
    }

    #[test]
    fn map_bridge_event_disconnect_is_clean_exit() {
        let (events, terminal) = map_bridge_event(3, "Packet Tunnel disconnected".into());
        assert!(terminal);
        assert!(matches!(events.last(), Some(BackendEvent::Exited(Some(0)))));
    }

    #[test]
    fn map_bridge_event_error_is_failed_exit() {
        let (events, terminal) = map_bridge_event(4, "AUTH_FAILED".into());
        assert!(terminal);
        assert_eq!(
            events,
            vec![
                BackendEvent::Stderr("AUTH_FAILED".into()),
                BackendEvent::Exited(Some(1)),
            ]
        );
    }

    #[test]
    fn map_bridge_event_log_and_approval_are_non_terminal() {
        let (logs, terminal) = map_bridge_event(0, "openvpn: hello".into());
        assert!(!terminal);
        assert_eq!(logs, vec![BackendEvent::Stdout("openvpn: hello".into())]);

        let (approval, terminal) = map_bridge_event(5, String::new());
        assert!(!terminal);
        assert!(matches!(approval.first(), Some(BackendEvent::Stdout(_))));
    }
}
