use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::Duration;

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::openvpn::backend::{BackendEvent, ConnectRequest, ReconcileDnsRequest, SpawnedSession};
use crate::openvpn::helper_protocol::HelperEvent;
use crate::VpnBackend;

/// How long app shutdown waits for the helper to exit after SIGTERM. OpenVPN
/// runs the DNS `down` script before it exits, so waiting lets DNS be
/// restored before the runtime directory (and that script) is deleted.
const HELPER_SHUTDOWN_EXIT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for the helper's last output lines after it exits.
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct ChildHandle {
    pid: Option<u32>,
    child: Arc<AsyncMutex<Child>>,
    exited: Arc<ExitSignal>,
}

/// Set once the helper process has exited. Waitable without a Tokio runtime,
/// because shutdown runs on the main thread during app termination.
#[derive(Debug, Default)]
struct ExitSignal {
    exited: StdMutex<bool>,
    changed: Condvar,
}

impl ExitSignal {
    fn notify(&self) {
        *self.exited.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.changed.notify_all();
    }

    fn wait(&self, timeout: Duration) -> bool {
        let guard = self.exited.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |exited| !*exited)
            .unwrap_or_else(|e| e.into_inner());
        *guard
    }
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
        let exited = Arc::new(ExitSignal::default());

        self.children.lock().insert(
            request.session_id.clone(),
            ChildHandle {
                pid,
                child: child.clone(),
                exited: exited.clone(),
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

        let stdout_task = stdout.map(|stdout| {
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
            })
        });

        let stderr_task = stderr.map(|stderr| {
            let tx = event_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(BackendEvent::Stderr(format!("helper: {line}")));
                }
            })
        });

        {
            let tx = event_tx.clone();
            let child = child.clone();
            let children = self.children.clone();
            let session_id = request.session_id.clone();
            tokio::spawn(async move {
                let status = child.lock().await.wait().await.ok();
                exited.notify();
                children.lock().remove(&session_id);
                // Deliver OpenVPN's last lines (AUTH_FAILED, the fatal error)
                // before Exited: the connection manager stops reading at
                // Exited and decides between retry and error from them.
                let drain = async {
                    if let Some(task) = stdout_task {
                        let _ = task.await;
                    }
                    if let Some(task) = stderr_task {
                        let _ = task.await;
                    }
                };
                let _ = tokio::time::timeout(OUTPUT_DRAIN_TIMEOUT, drain).await;
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

            // Only reap the child asynchronously when a Tokio runtime is
            // entered on this thread. disconnect() is also reached via
            // ConnectionManager::shutdown(), which runs on the main thread
            // from macOS' nounwind application_will_terminate callback during
            // app termination — where no runtime is entered and a bare
            // tokio::spawn would panic ("there is no reactor running"),
            // escalate to panic_cannot_unwind, and abort the process. The
            // SIGTERM above already stops the process; if we skip the reaper
            // here the OS reaps any zombie when the app exits.
            if tokio::runtime::Handle::try_current().is_ok() {
                tokio::spawn(async move {
                    let mut child = handle.child.lock().await;
                    let _ = child.wait().await;
                });
            }
        }
        Ok(())
    }

    fn disconnect_on_shutdown(&self, session_id: SessionId) -> Result<(), AppError> {
        let Some(handle) = self.children.lock().remove(&session_id) else {
            return Ok(());
        };
        if let Some(pid) = handle.pid {
            let _ = ProcessCommand::new("/bin/kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status();
        }
        // The helper exits after OpenVPN, which exits after its DNS `down`
        // script. Bounded so a stuck helper never blocks quitting.
        if !handle.exited.wait(HELPER_SHUTDOWN_EXIT_TIMEOUT) {
            eprintln!(
                "warning: openwrap-helper did not exit within {}s of SIGTERM during shutdown",
                HELPER_SHUTDOWN_EXIT_TIMEOUT.as_secs()
            );
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
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{reconcile_dns_error_message, ExitSignal};

    #[test]
    fn exit_signal_wakes_waiters_and_times_out_when_unset() {
        let signal = Arc::new(ExitSignal::default());
        let started = Instant::now();
        assert!(!signal.wait(Duration::from_millis(20)));
        assert!(started.elapsed() >= Duration::from_millis(20));

        let notifier = signal.clone();
        let thread = std::thread::spawn(move || notifier.notify());
        assert!(signal.wait(Duration::from_secs(5)));
        thread.join().unwrap();
        // Already set: returns at once.
        assert!(signal.wait(Duration::ZERO));
    }

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
