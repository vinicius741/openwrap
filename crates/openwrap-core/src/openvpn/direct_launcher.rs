use std::collections::HashMap;
use std::io::ErrorKind;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::openvpn::backend::{BackendEvent, ConnectRequest, SpawnedSession};
use crate::VpnBackend;

#[derive(Debug)]
struct ChildHandle {
    pid: Option<u32>,
    child: Arc<AsyncMutex<Child>>,
}

#[derive(Debug, Default)]
pub struct DirectOpenVpnBackend {
    children: Arc<Mutex<HashMap<SessionId, ChildHandle>>>,
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

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().map_err(map_spawn_error)?;
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(AsyncMutex::new(child));

        self.children.lock().insert(
            request.session_id.clone(),
            ChildHandle {
                pid,
                child: child.clone(),
            },
        );

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
            let children = self.children.clone();
            let session_id = request.session_id.clone();
            tokio::spawn(async move {
                let status = child.lock().await.wait().await.ok();
                children.lock().remove(&session_id);
                let _ = tx.send(BackendEvent::Exited(
                    status.and_then(|status| status.code()),
                ));
            });
        }

        Ok(SpawnedSession {
            session_id: request.session_id,
            pid,
            event_rx,
        })
    }

    fn disconnect(&self, session_id: SessionId) -> Result<(), AppError> {
        if let Some(handle) = self.children.lock().remove(&session_id) {
            if let Some(pid) = handle.pid {
                let _ = ProcessCommand::new("/bin/kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .status();
            }

            tokio::spawn(async move {
                let mut child = handle.child.lock().await;
                let _ = child.start_kill();
                let _ = child.wait().await;
            });
        }
        Ok(())
    }
}

fn map_spawn_error(error: std::io::Error) -> AppError {
    match error.kind() {
        ErrorKind::NotFound => AppError::OpenVpnBinaryNotFound,
        ErrorKind::PermissionDenied => {
            AppError::OpenVpnLaunch("Selected OpenVPN binary is not executable.".into())
        }
        _ => AppError::OpenVpnLaunch(error.to_string()),
    }
}
