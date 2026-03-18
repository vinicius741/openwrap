use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::connection::SessionId;
use crate::profiles::ProfileId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub session_id: SessionId,
    pub profile_id: ProfileId,
    pub openvpn_binary: PathBuf,
    pub config_path: PathBuf,
    pub auth_file: Option<PathBuf>,
    pub runtime_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileDnsRequest {
    pub runtime_root: PathBuf,
}

/// Events emitted by the VPN backend during process lifecycle.
///
/// `PartialEq` is derived to enable equality assertions in tests for
/// serialization roundtrip verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackendEvent {
    Started(Option<u32>),
    Stdout(String),
    Stderr(String),
    Exited(Option<i32>),
}

#[derive(Debug)]
pub struct SpawnedSession {
    pub session_id: SessionId,
    pub pid: Option<u32>,
    pub event_rx: mpsc::UnboundedReceiver<BackendEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::ProfileId;

    #[test]
    fn connect_request_structure() {
        let request = ConnectRequest {
            session_id: SessionId::new(),
            profile_id: ProfileId::new(),
            openvpn_binary: PathBuf::from("/usr/bin/openvpn"),
            config_path: PathBuf::from("/tmp/profile.ovpn"),
            auth_file: Some(PathBuf::from("/tmp/auth.txt")),
            runtime_dir: PathBuf::from("/tmp/runtime"),
        };
        assert!(request.session_id.0 != uuid::Uuid::nil());
        assert!(request.profile_id.0 != uuid::Uuid::nil());
        assert_eq!(request.openvpn_binary, PathBuf::from("/usr/bin/openvpn"));
        assert!(request.auth_file.is_some());
    }

    #[test]
    fn connect_request_without_auth_file() {
        let request = ConnectRequest {
            session_id: SessionId::new(),
            profile_id: ProfileId::new(),
            openvpn_binary: PathBuf::from("/usr/bin/openvpn"),
            config_path: PathBuf::from("/tmp/profile.ovpn"),
            auth_file: None,
            runtime_dir: PathBuf::from("/tmp/runtime"),
        };
        assert!(request.auth_file.is_none());
    }

    #[test]
    fn reconcile_dns_request_structure() {
        let request = ReconcileDnsRequest {
            runtime_root: PathBuf::from("/tmp/runtime"),
        };
        assert_eq!(request.runtime_root, PathBuf::from("/tmp/runtime"));
    }

    #[test]
    fn backend_event_serialization() {
        let events = vec![
            BackendEvent::Started(Some(1234)),
            BackendEvent::Started(None),
            BackendEvent::Stdout("test output".to_string()),
            BackendEvent::Stderr("test error".to_string()),
            BackendEvent::Exited(Some(0)),
            BackendEvent::Exited(None),
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let roundtrip: BackendEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, roundtrip);
        }
    }

    #[test]
    fn backend_event_display() {
        let event = BackendEvent::Stdout("hello".to_string());
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("hello"));
    }
}
