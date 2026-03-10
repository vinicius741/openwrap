use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::openvpn::backend::{BackendEvent, ConnectRequest, SpawnedSession};
use crate::VpnBackend;

#[derive(Debug, Default)]
pub struct DirectOpenVpnBackend {
    children: Arc<Mutex<HashMap<SessionId, Arc<AsyncMutex<Child>>>>>,
}

impl DirectOpenVpnBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl VpnBackend for DirectOpenVpnBackend {
    fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError> {
        let mut command = Command::new(&request.openvpn_binary);
        command.arg("--config").arg(&request.config_path);
        command.arg("--auth-nocache");
        command.arg("--verb").arg("3");

        if let Some(auth_file) = &request.auth_file {
            command.arg("--auth-user-pass").arg(auth_file);
        }

        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&request.runtime_dir);

        let mut child = command
            .spawn()
            .map_err(|error| AppError::OpenVpnLaunch(error.to_string()))?;

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(AsyncMutex::new(child));
        self.children
            .lock()
            .insert(request.session_id.clone(), child.clone());

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        if let Some(stdout) = stdout {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(BackendEvent::Stdout(line));
                }
            });
        }

        if let Some(stderr) = stderr {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(BackendEvent::Stderr(line));
                }
            });
        }

        {
            let tx = event_tx.clone();
            let child = child.clone();
            tokio::spawn(async move {
                let status = child.lock().await.wait().await.ok();
                let _ = tx.send(BackendEvent::Exited(status.and_then(|status| status.code())));
            });
        }

        Ok(SpawnedSession {
            session_id: request.session_id,
            pid,
            event_rx,
        })
    }

    fn disconnect(&self, session_id: SessionId) -> Result<(), AppError> {
        if let Some(child) = self.children.lock().remove(&session_id) {
            tokio::spawn(async move {
                let _ = child.lock().await.kill().await;
            });
        }
        Ok(())
    }
}

