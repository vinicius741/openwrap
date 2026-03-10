use std::ffi::CStr;
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use openwrap_core::openvpn::config_working_dir;
use openwrap_core::openvpn::helper_protocol::HelperEvent;
use openwrap_core::openvpn::ConnectRequest;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let exit_code = match std::env::args().nth(1).as_deref() {
        Some("connect") => run_connect().await,
        _ => {
            eprintln!("usage: openwrap-helper connect");
            64
        }
    };
    std::process::exit(exit_code);
}

async fn run_connect() -> i32 {
    let request = match read_request() {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return 64;
        }
    };

    if let Err(error) = validate_request(&request) {
        eprintln!("{error}");
        return 78;
    }

    let mut command = Command::new(&request.openvpn_binary);
    let working_dir = match config_working_dir(&request.config_path) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return 78;
        }
    };
    command.arg("--config").arg(&request.config_path);
    command.arg("--auth-nocache");
    command.arg("--verb").arg("3");
    command.env_clear();
    command.current_dir(working_dir);

    if let Some(auth_file) = &request.auth_file {
        command.arg("--auth-user-pass").arg(auth_file);
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("failed to launch openvpn: {error}");
            return 70;
        }
    };

    let openvpn_pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let writer = Arc::new(Mutex::new(BufWriter::new(tokio::io::stdout())));

    if let Err(error) = emit_event(&writer, &HelperEvent::Started { pid: openvpn_pid }).await {
        eprintln!("failed to emit helper startup event: {error}");
        return 70;
    }

    let stdout_task = stdout.map(|stdout| {
        let writer = writer.clone();
        tokio::spawn(async move { pipe_lines(stdout, writer, true).await })
    });
    let stderr_task = stderr.map(|stderr| {
        let writer = writer.clone();
        tokio::spawn(async move { pipe_lines(stderr, writer, false).await })
    });

    let mut sigterm = signal(SignalKind::terminate()).ok();
    let mut sigint = signal(SignalKind::interrupt()).ok();
    let status = tokio::select! {
        status = child.wait() => status.ok(),
        _ = recv_signal(&mut sigterm, &mut sigint) => {
            terminate_pid(openvpn_pid);
            child.wait().await.ok()
        }
    };

    if let Some(task) = stdout_task {
        let _ = task.await;
    }
    if let Some(task) = stderr_task {
        let _ = task.await;
    }

    status.and_then(|status| status.code()).unwrap_or(1)
}

fn read_request() -> Result<ConnectRequest, String> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|error| format!("failed to read request: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("invalid request payload: {error}"))
}

fn validate_request(request: &ConnectRequest) -> Result<(), String> {
    let home_dir = real_user_home_dir()?;
    let base_dir = home_dir.join("Library/Application Support/OpenWrap");
    let profiles_dir = base_dir.join("profiles");
    let runtime_dir = base_dir.join("runtime");

    validate_scoped_path("config", &request.config_path, &profiles_dir)?;
    validate_scoped_path("runtime", &request.runtime_dir, &runtime_dir)?;
    if let Some(auth_file) = &request.auth_file {
        validate_scoped_path("auth file", auth_file, &request.runtime_dir)?;
    }
    validate_openvpn_binary(&request.openvpn_binary)?;
    Ok(())
}

fn validate_scoped_path(label: &str, path: &Path, root: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} path must be absolute"));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve allowed {label} root: {error}"))?;
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {label} path: {error}"))?;
    if canonical_path.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(format!(
            "{label} path escapes the OpenWrap managed directory: {}",
            path.display()
        ))
    }
}

fn validate_openvpn_binary(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("openvpn binary path must be absolute".into());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve openvpn binary: {error}"))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("failed to inspect openvpn binary: {error}"))?;
    let mode = metadata.permissions().mode();
    if metadata.is_file() && (mode & 0o111) != 0 {
        Ok(())
    } else {
        Err(format!(
            "openvpn binary is not executable: {}",
            canonical.display()
        ))
    }
}

fn real_user_home_dir() -> Result<PathBuf, String> {
    let uid = unsafe { libc::getuid() };
    let mut passwd = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut result = std::ptr::null_mut();
    let mut buffer = vec![0u8; 4096];

    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &mut passwd,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };

    if status != 0 || result.is_null() {
        return Err("failed to resolve the invoking user's home directory".into());
    }

    let home = unsafe { CStr::from_ptr(passwd.pw_dir) };
    Ok(PathBuf::from(std::ffi::OsStr::from_bytes(home.to_bytes())))
}

fn terminate_pid(pid: Option<u32>) {
    if let Some(pid) = pid {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

async fn recv_signal(
    sigterm: &mut Option<tokio::signal::unix::Signal>,
    sigint: &mut Option<tokio::signal::unix::Signal>,
) {
    match (sigterm.as_mut(), sigint.as_mut()) {
        (Some(sigterm), Some(sigint)) => {
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sigint.recv() => {}
            }
        }
        (Some(sigterm), None) => {
            let _ = sigterm.recv().await;
        }
        (None, Some(sigint)) => {
            let _ = sigint.recv().await;
        }
        (None, None) => std::future::pending::<()>().await,
    }
}

async fn pipe_lines<T>(
    stream: T,
    writer: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    is_stdout: bool,
) -> io::Result<()>
where
    T: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await? {
        let event = if is_stdout {
            HelperEvent::Stdout { line }
        } else {
            HelperEvent::Stderr { line }
        };
        emit_event(&writer, &event).await?;
    }
    Ok(())
}

async fn emit_event(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    event: &HelperEvent,
) -> io::Result<()> {
    let serialized = serde_json::to_string(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut writer = writer.lock().await;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}
