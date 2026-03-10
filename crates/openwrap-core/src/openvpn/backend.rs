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
