use crate::config::SubprocessConfig;
use crate::envelope;
use crate::error::IpcError;
use crate::pipe::create_pipe;
use crate::stderr::{read_bounded_stderr, StderrCapture};
use std::os::fd::FromRawFd;
use std::os::unix::process::ExitStatusExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub fd4_bytes: Vec<u8>,
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
    let pipe3 = create_pipe()?;
    let pipe4 = create_pipe()?;

    let mut command = tokio::process::Command::new(config.executable_path());
    command.args(config.argv());
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::piped());

    {
        let fd3_read = pipe3.read_fd();
        let fd4_write = pipe4.write_fd();

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
    drop(pipe3.read_fd());
    drop(pipe4.write_fd());

    let fd3_writer = unsafe { std::fs::File::from_raw_fd(pipe3.write_fd()) };
    let fd3_writer = tokio::fs::File::from_std(fd3_writer);
    let fd4_reader = unsafe { std::fs::File::from_raw_fd(pipe4.read_fd()) };
    let fd4_reader = tokio::fs::File::from_std(fd4_reader);
    let stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| IpcError::StderrReadFailed {
            detail: "Failed to take stderr".to_string(),
        })?;

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload().to_vec();

    let stderr_task = tokio::task::spawn(read_bounded_stderr(stderr_reader));

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
        let capture = stderr_res.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to capture stderr during timeout");
            StderrCapture::empty()
        });

        return Err(IpcError::Timeout {
            elapsed_ms: timeout_ms,
            stderr_bytes: capture.bytes,
            stderr_truncated: capture.truncated,
        });
    };

    let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
        detail: e.to_string(),
    })?;
    let capture = stderr_res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to capture stderr after process exit");
        StderrCapture::empty()
    });

    match res {
        Ok(mut output) => {
            output.stderr_bytes = capture.bytes;
            output.stderr_truncated = capture.truncated;
            Ok(output)
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => Err(IpcError::ProcessFailed {
            exit_code,
            stderr_bytes: capture.bytes,
            stderr_truncated: capture.truncated,
        }),
        Err(e) => Err(e),
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
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    } else {
        Err(IpcError::ProcessFailed {
            exit_code: map_exit_code(exit_status),
            stderr_bytes: vec![],
            stderr_truncated: false,
        })
    }
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
