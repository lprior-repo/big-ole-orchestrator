use crate::config::SubprocessConfig;
use crate::envelope::{self, VersionHandshake, CURRENT_IPC_VERSION};
use crate::error::IpcError;
use crate::stderr::{read_bounded_stderr, StderrCapture};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub fd4_bytes: Vec<u8>,
    pub stdout_bytes: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_bytes: Vec<u8>,
    pub stderr_truncated: bool,
}

/// Runs a subprocess and performs IPC.
///
/// # Errors
///
/// Returns `IpcError` if:
/// - Pipe setup fails
/// - Subprocess fails to spawn
/// - IPC fails
/// - Subprocess times out
#[tracing::instrument(skip(config))]
pub async fn run_subprocess(config: SubprocessConfig) -> Result<SubprocessOutput, IpcError> {
    let (fd3_read, fd3_write) = create_pipe()?;
    let (fd4_read, fd4_write) = create_pipe()?;

    let mut command = tokio::process::Command::new(config.executable_path());
    command.args(config.argv());
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    {
        // SAFETY: pre_exec is unsafe because it runs in the child before exec.
        // We set PR_SET_PDEATHSIG so the child dies if parent exits,
        // setpgid to prevent SIGTTIN, and dup2 to set up fd3/fd4.
        // The file descriptors are valid pipe ends created with O_CLOEXEC.
        unsafe {
            command.pre_exec(move || {
                // SAFETY: prctl with PR_SET_PDEATHSIG is a safe syscall that
                // only affects signal handling. Setting to SIGTERM is standard.
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // SAFETY: setpgid is a safe syscall to create process group.
                // This prevents the child from receiving SIGTTIN from terminal.
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // SAFETY: dup2 with explicit fd numbers 3 and 4 is safe because
                // we created these FDs and they are not used elsewhere in the child.
                if libc::dup2(fd3_read, 3) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd4_write, 4) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = command.spawn().map_err(|e| IpcError::SpawnFailed {
        detail: e.to_string(),
    })?;

    // Parent closes child's ends of the pipes.
    // The child now owns these FDs via dup2; parent must close them.
    let _ = fd3_read;
    let _ = fd4_write;

    let fd3_writer = unsafe { std::fs::File::from_raw_fd(fd3_write) };
    let fd3_writer = tokio::fs::File::from_std(fd3_writer);
    let fd4_reader = unsafe { std::fs::File::from_raw_fd(fd4_read) };
    let fd4_reader = tokio::fs::File::from_std(fd4_reader);
    let stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| IpcError::StderrReadFailed {
            detail: "Failed to take stderr".to_string(),
        })?;
    let stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| IpcError::StdoutReadFailed {
            detail: "Failed to take stdout".to_string(),
        })?;

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload().to_vec();

    let stderr_task = tokio::task::spawn(read_bounded_stderr(stderr_reader));
    let stdout_task = tokio::task::spawn(read_bounded_stderr(stdout_reader));

    let timeout_res = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        perform_ipc(&mut child, fd3_writer, fd4_reader, fd3_payload),
    )
    .await;

    let Ok(res) = timeout_res else {
        let Some(_pid) = child.id() else {
            return Err(IpcError::SignalFailed {
                detail: "PID not found".to_string(),
            });
        };
        terminate_child(&mut child).await;

        let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
            detail: e.to_string(),
        })?;
        let stdout_res = stdout_task.await.map_err(|e| IpcError::StdoutReadFailed {
            detail: e.to_string(),
        })?;
        let stderr_capture = stderr_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stderr during timeout");
            StderrCapture::empty()
        });
        let stdout_capture = stdout_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stdout during timeout");
            StderrCapture::empty()
        });

        return Err(IpcError::Timeout {
            elapsed_ms: timeout_ms,
            stdout_bytes: stdout_capture.bytes,
            stdout_truncated: stdout_capture.truncated,
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        });
    };

    let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
        detail: e.to_string(),
    })?;
    let stdout_res = stdout_task.await.map_err(|e| IpcError::StdoutReadFailed {
        detail: e.to_string(),
    })?;
    let stderr_capture = stderr_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to capture stderr after process exit");
        StderrCapture::empty()
    });
    let stdout_capture = stdout_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to capture stdout after process exit");
        StderrCapture::empty()
    });

    match res {
        Ok(mut output) => {
            output.stdout_bytes = stdout_capture.bytes;
            output.stdout_truncated = stdout_capture.truncated;
            output.stderr_bytes = stderr_capture.bytes;
            output.stderr_truncated = stderr_capture.truncated;
            Ok(output)
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => Err(IpcError::ProcessFailed {
            exit_code,
            stdout_bytes: stdout_capture.bytes,
            stdout_truncated: stdout_capture.truncated,
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        }),
        Err(e) => Err(e),
    }
}

pub const HANDSHAKE_TIMEOUT_MS: u64 = 5000;

#[tracing::instrument(skip(config))]
pub async fn run_subprocess_with_handshake(
    config: SubprocessConfig,
) -> Result<SubprocessOutput, IpcError> {
    let (fd3_read, fd3_write) = create_pipe()?;
    let (fd4_read, fd4_write) = create_pipe()?;

    let mut command = tokio::process::Command::new(config.executable_path());
    command.args(config.argv());
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    {
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd3_read, 3) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd4_write, 4) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = command.spawn().map_err(|e| IpcError::SpawnFailed {
        detail: e.to_string(),
    })?;

    let _ = fd3_read;
    let _ = fd4_write;

    let fd3_writer = unsafe { std::fs::File::from_raw_fd(fd3_write) };
    let fd3_writer = tokio::fs::File::from_std(fd3_writer);
    let fd4_reader = unsafe { std::fs::File::from_raw_fd(fd4_read) };
    let fd4_reader = tokio::fs::File::from_std(fd4_reader);

    let handshake_res = perform_version_handshake(fd3_writer, fd4_reader).await;

    let stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| IpcError::StderrReadFailed {
            detail: "Failed to take stderr".to_string(),
        })?;
    let stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| IpcError::StdoutReadFailed {
            detail: "Failed to take stdout".to_string(),
        })?;

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload().to_vec();

    let stderr_task = tokio::task::spawn(read_bounded_stderr(stderr_reader));
    let stdout_task = tokio::task::spawn(read_bounded_stderr(stdout_reader));

    let negotiated_version = match handshake_res {
        Ok(v) => v,
        Err(e) => {
            let _ = stderr_task.await;
            let _ = stdout_task.await;
            terminate_child(&mut child).await;
            return Err(e);
        }
    };

    let timeout_res = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        perform_ipc_with_handshake(&mut child, negotiated_version, fd3_payload, fd3_writer, fd4_reader),
    )
    .await;

    let Ok(res) = timeout_res else {
        let Some(_pid) = child.id() else {
            return Err(IpcError::SignalFailed {
                detail: "PID not found".to_string(),
            });
        };
        terminate_child(&mut child).await;

        let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
            detail: e.to_string(),
        })?;
        let stdout_res = stdout_task.await.map_err(|e| IpcError::StdoutReadFailed {
            detail: e.to_string(),
        })?;
        let stderr_capture = stderr_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stderr during timeout");
            StderrCapture::empty()
        });
        let stdout_capture = stdout_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stdout during timeout");
            StderrCapture::empty()
        });

        return Err(IpcError::Timeout {
            elapsed_ms: timeout_ms,
            stdout_bytes: stdout_capture.bytes,
            stdout_truncated: stdout_capture.truncated,
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        });
    };

    let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
        detail: e.to_string(),
    })?;
    let stdout_res = stdout_task.await.map_err(|e| IpcError::StdoutReadFailed {
        detail: e.to_string(),
    })?;
    let stderr_capture = stderr_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to capture stderr after process exit");
        StderrCapture::empty()
    });
    let stdout_capture = stdout_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to capture stdout after process exit");
        StderrCapture::empty()
    });

    match res {
        Ok(mut output) => {
            output.stdout_bytes = stdout_capture.bytes;
            output.stdout_truncated = stdout_capture.truncated;
            output.stderr_bytes = stderr_capture.bytes;
            output.stderr_truncated = stderr_capture.truncated;
            Ok(output)
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => Err(IpcError::ProcessFailed {
            exit_code,
            stdout_bytes: stdout_capture.bytes,
            stdout_truncated: stdout_capture.truncated,
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        }),
        Err(e) => Err(e),
    }
}

async fn perform_version_handshake(
    mut fd3_writer: tokio::fs::File,
    mut fd4_reader: tokio::fs::File,
) -> Result<u8, IpcError> {
    let handshake = VersionHandshake {
        version: CURRENT_IPC_VERSION,
    };

    let mut json_bytes = serde_json::to_vec(&handshake)
        .map_err(|e| IpcError::InvalidJson(e.to_string()))?;

    let len = u32::try_from(json_bytes.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "handshake payload exceeds u32::MAX",
        )
    })?;

    fd3_writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
    fd3_writer
        .write_all(&json_bytes)
        .await
        .map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
    fd3_writer
        .flush()
        .await
        .map_err(|e| IpcError::Fd3WriteFailed { detail: e.to_string() })?;
    drop(fd3_writer);

    let read_timeout = tokio::time::timeout(
        std::time::Duration::from_millis(HANDSHAKE_TIMEOUT_MS),
        read_version_response(&mut fd4_reader),
    )
    .await;

    let peer_version = match read_timeout {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return Err(e),
        Err(()) => return Err(IpcError::HandshakeTimeout),
    };

    let negotiation = envelope::VersionNegotiation::new();
    negotiation
        .negotiate(peer_version)
        .map_err(|_| IpcError::VersionMismatch(peer_version))
}

async fn read_version_response(fd4_reader: &mut tokio::fs::File) -> Result<u8, IpcError> {
    let mut header = [0u8; 4];
    let mut total_read = 0;
    while total_read < 4 {
        let n = fd4_reader
            .read(&mut header[total_read..])
            .await
            .map_err(|e| IpcError::Fd4ReadFailed { detail: e.to_string() })?;
        if n == 0 {
            if total_read == 0 {
                return Err(IpcError::Fd4ReadFailed {
                    detail: "unexpected EOF reading version response".to_string(),
                });
            }
            return Err(IpcError::IncompleteRead {
                expected: 4,
                actual: total_read,
            });
        }
        total_read += n;
    }

    let len = u32::from_be_bytes(header);
    if len > envelope::MAX_PAYLOAD_SIZE {
        return Err(IpcError::PayloadTooLarge(len));
    }

    if len == 0 {
        return Err(IpcError::Fd4ReadFailed {
            detail: "empty version response".to_string(),
        });
    }

    let mut bytes = vec![0u8; len as usize];
    fd4_reader
        .read_exact(&mut bytes)
        .await
        .map_err(|e| IpcError::Fd4ReadFailed { detail: e.to_string() })?;

    let response: VersionHandshake =
        serde_json::from_slice(&bytes).map_err(|e| IpcError::InvalidJson(e.to_string()))?;

    Ok(response.version)
}

async fn perform_ipc_with_handshake(
    child: &mut tokio::process::Child,
    negotiated_version: u8,
    fd3_payload: Vec<u8>,
    mut fd3_writer: tokio::fs::File,
    mut fd4_reader: tokio::fs::File,
) -> Result<SubprocessOutput, IpcError> {
    let _ = negotiated_version;

    let write_task = async {
        let len = u32::try_from(fd3_payload.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "fd3 payload exceeds u32::MAX",
            )
        })?;
        if len > envelope::MAX_PAYLOAD_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("fd3 payload too large: {len} bytes"),
            ));
        }
        fd3_writer.write_all(&len.to_be_bytes()).await?;
        match fd3_writer.write_all(&fd3_payload).await {
            Ok(()) => {
                drop(fd3_writer.flush().await);
                drop(fd3_writer);
                Ok::<(), std::io::Error>(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
        }
    };

    let read_fd4_task = async {
        let mut header = [0u8; 4];
        let mut total_read = 0;
        while total_read < 4 {
            let n = fd4_reader.read(&mut header[total_read..]).await?;
            if n == 0 {
                if total_read == 0 {
                    return Ok::<Vec<u8>, std::io::Error>(vec![]);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "early eof",
                ));
            }
            total_read += n;
        }
        let len = u32::from_be_bytes(header);
        if len > envelope::MAX_PAYLOAD_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("fd4 payload too large: {len} bytes"),
            ));
        }
        let mut bytes = vec![0u8; len as usize];
        fd4_reader.read_exact(&mut bytes).await?;
        Ok::<Vec<u8>, std::io::Error>(bytes)
    };

    let (w_res, r4_res) = tokio::join!(write_task, read_fd4_task);

    if let Err(e) = w_res {
        return Err(IpcError::Fd3WriteFailed {
            detail: e.to_string(),
        });
    }

    let fd4_bytes = r4_res.map_err(|e| IpcError::Fd4ReadFailed {
        detail: e.to_string(),
    })?;

    let exit_status = child.wait().await.map_err(|e| IpcError::WaitFailed {
        detail: e.to_string(),
    })?;

    if exit_status.success() {
        Ok(SubprocessOutput {
            fd4_bytes,
            stdout_bytes: vec![],
            stdout_truncated: false,
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    } else {
        Err(IpcError::ProcessFailed {
            exit_code: map_exit_code(exit_status),
            stdout_bytes: vec![],
            stdout_truncated: false,
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    }
}

#[tracing::instrument(skip_all)]
async fn perform_ipc(
    child: &mut tokio::process::Child,
    mut fd3_writer: tokio::fs::File,
    mut fd4_reader: tokio::fs::File,
    fd3_payload: Vec<u8>,
) -> Result<SubprocessOutput, IpcError> {
    let write_task = async {
        let len = u32::try_from(fd3_payload.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "fd3 payload exceeds u32::MAX",
            )
        })?;
        if len > envelope::MAX_PAYLOAD_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("fd3 payload too large: {len} bytes"),
            ));
        }
        fd3_writer.write_all(&len.to_be_bytes()).await?;
        match fd3_writer.write_all(&fd3_payload).await {
            Ok(()) => {
                drop(fd3_writer.flush().await);
                drop(fd3_writer);
                Ok::<(), std::io::Error>(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
        }
    };

    let read_fd4_task = async {
        let mut header = [0u8; 4];
        let mut total_read = 0;
        while total_read < 4 {
            let n = fd4_reader.read(&mut header[total_read..]).await?;
            if n == 0 {
                if total_read == 0 {
                    return Ok::<Vec<u8>, std::io::Error>(vec![]);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "early eof",
                ));
            }
            total_read += n;
        }
        let len = u32::from_be_bytes(header);
        if len > envelope::MAX_PAYLOAD_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("fd4 payload too large: {len} bytes"),
            ));
        }
        let mut bytes = vec![0u8; len as usize];
        fd4_reader.read_exact(&mut bytes).await?;
        Ok::<Vec<u8>, std::io::Error>(bytes)
    };

    let (w_res, r4_res) = tokio::join!(write_task, read_fd4_task);

    if let Err(e) = w_res {
        return Err(IpcError::Fd3WriteFailed {
            detail: e.to_string(),
        });
    }

    let fd4_bytes = r4_res.map_err(|e| IpcError::Fd4ReadFailed {
        detail: e.to_string(),
    })?;

    let exit_status = child.wait().await.map_err(|e| IpcError::WaitFailed {
        detail: e.to_string(),
    })?;

    if exit_status.success() {
        Ok(SubprocessOutput {
            fd4_bytes,
            stdout_bytes: vec![],
            stdout_truncated: false,
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    } else {
        Err(IpcError::ProcessFailed {
            exit_code: map_exit_code(exit_status),
            stdout_bytes: vec![],
            stdout_truncated: false,
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    }
}

fn create_pipe() -> Result<(RawFd, RawFd), IpcError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(IpcError::PipeSetupFailed {
            detail: std::io::Error::last_os_error().to_string(),
        });
    }
    Ok(fds.into())
}

#[tracing::instrument]
async fn terminate_child(child: &mut tokio::process::Child) {
    let Some(pid) = child.id() else {
        return;
    };
    let kill_pgid = pid.cast_signed();

    // SAFETY: kill with negative PID sends signal to process group.
    // We created this process group with setpgid in pre_exec,
    // so sending SIGTERM to the group is safe for cleanup.
    unsafe {
        libc::kill(-kill_pgid, libc::SIGTERM);
    }
    let res = tokio::time::timeout(std::time::Duration::from_millis(100), child.wait()).await;
    if res.is_err() {
        // SAFETY: SIGKILL is force-kill when graceful termination fails.
        // This is safe because we own this process group.
        unsafe {
            libc::kill(-kill_pgid, libc::SIGKILL);
        }
    }
}

#[must_use]
pub(crate) fn map_exit_code(status: std::process::ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| status.signal().map_or(-1, |s| 128 + s))
}
