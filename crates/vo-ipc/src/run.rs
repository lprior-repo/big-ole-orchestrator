use crate::config::SubprocessConfig;
use crate::envelope;
use crate::error::IpcError;
use crate::stderr::read_bounded_stderr;
use std::os::fd::{FromRawFd, RawFd};
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
pub async fn run_subprocess(config: SubprocessConfig) -> Result<SubprocessOutput, IpcError> {
    let (fd3_read, fd3_write) = create_pipe()?;
    let (fd4_read, fd4_write) = create_pipe()?;

    let mut command = tokio::process::Command::new(config.executable_path());
    command.args(config.argv());
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::piped());

    unsafe {
        command.pre_exec(move || {
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

    let mut child = command.spawn().map_err(|e| IpcError::SpawnFailed {
        detail: e.to_string(),
    })?;

    // Parent closes child's ends
    unsafe {
        libc::close(fd3_read);
        libc::close(fd4_write);
    }

    let fd3_writer = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
    let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };
    let stderr_reader = child.stderr.take().ok_or_else(|| IpcError::StderrReadFailed {
        detail: "Failed to take stderr".to_string(),
    })?;

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload().to_vec();

    let stderr_task = tokio::task::spawn(read_bounded_stderr(stderr_reader));

    let res = tokio::select! {
        res = perform_ipc(&mut child, fd3_writer, fd4_reader, fd3_payload) => res,
        () = tokio::time::sleep(std::time::Duration::from_millis(timeout_ms)) => {
            let Some(pid) = child.id() else {
                return Err(IpcError::SignalFailed { detail: "PID not found".to_string() });
            };
            terminate_pg(pid).await?;
            
            let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
                detail: e.to_string(),
            })?;
            let capture = stderr_res.unwrap_or_default();
            
            return Err(IpcError::Timeout {
                elapsed_ms: timeout_ms,
                stderr_bytes: capture.bytes,
                stderr_truncated: capture.truncated,
            });
        }
    };

    let stderr_res = stderr_task.await.map_err(|e| IpcError::StderrReadFailed {
        detail: e.to_string(),
    })?;
    let capture = stderr_res.unwrap_or_default();

    match res {
        Ok(mut output) => {
            output.stderr_bytes = capture.bytes;
            output.stderr_truncated = capture.truncated;
            Ok(output)
        }
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            Err(IpcError::ProcessFailed {
                exit_code,
                stderr_bytes: capture.bytes,
                stderr_truncated: capture.truncated,
            })
        }
        Err(e) => Err(e),
    }
}

async fn perform_ipc(
    child: &mut tokio::process::Child,
    mut fd3_writer: tokio::fs::File,
    mut fd4_reader: tokio::fs::File,
    fd3_payload: Vec<u8>,
) -> Result<SubprocessOutput, IpcError> {
    let write_task = async {
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

async fn terminate_pg(pid: u32) -> Result<(), IpcError> {
    const GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(100);
    let kill_pgid = pid.cast_signed();
    unsafe {
        libc::kill(-kill_pgid, libc::SIGTERM);
    }
    // Give the process group a moment to exit gracefully before SIGKILL.
    tokio::time::sleep(GRACE_PERIOD).await;
    unsafe {
        libc::kill(-kill_pgid, libc::SIGKILL);
    }
    Ok(())
}

#[must_use]
pub(crate) fn map_exit_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or_else(|| status.signal().map_or(-1, |s| 128 + s))
}
