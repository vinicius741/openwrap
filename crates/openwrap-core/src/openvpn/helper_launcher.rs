use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::openvpn::backend::{BackendEvent, ConnectRequest, ReconcileDnsRequest, SpawnedSession};
use crate::openvpn::helper_protocol::HelperEvent;
use crate::VpnBackend;

#[derive(Debug)]
struct ChildHandle {
    pid: Option<u32>,
    child: Arc<AsyncMutex<Child>>,
}

#[derive(Debug)]
pub struct HelperOpenVpnBackend {
    helper_binary: PathBuf,
    children: Arc<Mutex<HashMap<SessionId, ChildHandle>>>,
}

impl HelperOpenVpnBackend {
    pub fn new(helper_binary: PathBuf) -> Self {
        Self {
            helper_binary,
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl VpnBackend for HelperOpenVpnBackend {
    fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError> {
        validate_helper_binary(&self.helper_binary)?;

        let mut command = Command::new(&self.helper_binary);
        command.arg("connect");
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn().map_err(map_spawn_error)?;
        let pid = child.id();
        let stdin = child.stdin.take();
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

        if let Some(mut stdin) = stdin {
            let payload = serde_json::to_vec(&request)
                .map_err(|error| AppError::Serialization(error.to_string()))?;
            tokio::spawn(async move {
                let _ = stdin.write_all(&payload).await;
            });
        }

        if let Some(stdout) = stdout {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    match serde_json::from_str::<HelperEvent>(&line) {
                        Ok(HelperEvent::Started { pid }) => {
                            let _ = tx.send(BackendEvent::Started(pid));
                        }
                        Ok(HelperEvent::Stdout { line }) => {
                            let _ = tx.send(BackendEvent::Stdout(line));
                        }
                        Ok(HelperEvent::Stderr { line }) => {
                            let _ = tx.send(BackendEvent::Stderr(line));
                        }
                        Err(_) => {
                            let _ = tx.send(BackendEvent::Stderr(format!(
                                "helper protocol error: {line}"
                            )));
                        }
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(BackendEvent::Stderr(format!("helper: {line}")));
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
                let _ = child.wait().await;
            });
        }
        Ok(())
    }

    fn reconcile_dns(&self, request: ReconcileDnsRequest) -> Result<(), AppError> {
        validate_helper_binary(&self.helper_binary)?;

        let mut child = ProcessCommand::new(&self.helper_binary)
            .arg("reconcile-dns")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(map_spawn_error)?;

        if let Some(mut stdin) = child.stdin.take() {
            let payload = serde_json::to_vec(&request)
                .map_err(|error| AppError::Serialization(error.to_string()))?;
            use std::io::Write;
            stdin
                .write_all(&payload)
                .map_err(|error| AppError::OpenVpnLaunch(error.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| AppError::OpenVpnLaunch(error.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(AppError::OpenVpnLaunch(reconcile_dns_error_message(
                &stderr,
            )))
        }
    }
}

fn reconcile_dns_error_message(stderr: &str) -> String {
    if stderr.contains("usage: openwrap-helper connect") && !stderr.contains("reconcile-dns") {
        "Privileged helper is outdated and does not support DNS reconciliation. Rebuild and reinstall openwrap-helper. See docs/helper-setup.md.".into()
    } else if stderr.is_empty() {
        "Privileged DNS reconciliation failed.".into()
    } else {
        format!("Privileged DNS reconciliation failed: {stderr}")
    }
}

fn map_spawn_error(error: std::io::Error) -> AppError {
    match error.kind() {
        ErrorKind::NotFound => AppError::HelperIssue(
            "Privileged helper binary was not found. See docs/helper-setup.md.".into(),
        ),
        ErrorKind::PermissionDenied => AppError::HelperIssue(
            "Privileged helper is not executable. See docs/helper-setup.md.".into(),
        ),
        _ => AppError::OpenVpnLaunch(error.to_string()),
    }
}

#[cfg(unix)]
fn validate_helper_binary(path: &PathBuf) -> Result<(), AppError> {
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = fs::metadata(path).map_err(map_spawn_error)?;
    let mode = metadata.permissions().mode();
    let owner_is_root = metadata.uid() == 0;
    let setuid = (mode & 0o4000) != 0;

    if owner_is_root && setuid {
        Ok(())
    } else {
        Err(AppError::HelperIssue(
            "Privileged helper is not installed with root ownership and setuid. See docs/helper-setup.md.".into(),
        ))
    }
}

#[cfg(not(unix))]
fn validate_helper_binary(_path: &PathBuf) -> Result<(), AppError> {
    Err(AppError::HelperIssue(
        "Privileged helper is only supported on macOS.".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::reconcile_dns_error_message;

    #[test]
    fn maps_outdated_helper_usage_to_actionable_error() {
        let message = reconcile_dns_error_message("usage: openwrap-helper connect");
        assert!(message.contains("outdated"));
        assert!(message.contains("reinstall"));
    }

    #[test]
    fn prefixes_other_reconcile_failures() {
        let message = reconcile_dns_error_message("permission denied");
        assert_eq!(
            message,
            "Privileged DNS reconciliation failed: permission denied"
        );
    }
}
