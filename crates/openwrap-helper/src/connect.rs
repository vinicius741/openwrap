use std::process::Stdio;
use std::sync::Arc;

use openwrap_core::openvpn::config_working_dir;
use openwrap_core::openvpn::helper_protocol::HelperEvent;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;

use crate::request::{read_request, validate_request};
use crate::system::terminate_pid;

pub async fn run_connect() -> i32 {
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
) -> std::io::Result<()>
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
) -> std::io::Result<()> {
    let serialized = serde_json::to_string(event)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let mut writer = writer.lock().await;
    writer.write_all(serialized.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}
