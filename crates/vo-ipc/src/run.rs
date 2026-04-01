use crate::config::SubprocessConfig;
use crate::envelope;
use crate::error::IpcError;
use crate::stderr::{read_bounded_stderr, StderrCapture};
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::task::JoinHandle;

const FD3_NUMBER: RawFd = 3;
const FD4_NUMBER: RawFd = 4;
const SIGTERM_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub fd4_bytes: Vec<u8>,
    pub stderr_bytes: Vec<u8>,
    pub stderr_truncated: bool,
}

/// Runs the configured subprocess with fd3/fd4 IPC, bounded stderr capture, and timeout
/// enforcement.
///
/// # Errors
/// Returns [`IpcError`] when validation, pipe setup, spawning, fd4 decoding, stderr capture,
/// waiting, or timeout escalation fails.
pub async fn run_subprocess(config: SubprocessConfig) -> Result<SubprocessOutput, IpcError> {
    run_subprocess_unix(config).await
}

#[cfg(unix)]
async fn run_subprocess_unix(config: SubprocessConfig) -> Result<SubprocessOutput, IpcError> {
    let start = Instant::now();
    let (fd3_read, fd3_write) = create_pipe()?;
    let (fd4_read, fd4_write) = create_pipe()?;

    let mut command = Command::new(config.executable_path());
    command
        .args(config.argv())
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    configure_child_fds(&mut command, &fd3_read, &fd4_write);

    let mut child = command.spawn().map_err(|error| IpcError::SpawnFailed {
        detail: error.to_string(),
    })?;

    let pid = child.id().map_or_else(
        || {
            Err(IpcError::WaitFailed {
                detail: String::from("child pid unavailable immediately after spawn"),
            })
        },
        |value| {
            i32::try_from(value).map_err(|_| IpcError::WaitFailed {
                detail: format!("child pid out of i32 range: {value}"),
            })
        },
    )?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| IpcError::StderrReadFailed {
            detail: String::from("stderr pipe was not available"),
        })?;

    drop(fd3_read);
    drop(fd4_write);

    let fd3_task = spawn_fd3_writer(fd3_write, config.fd3_payload().to_vec());
    let fd4_task = spawn_fd4_reader(fd4_read);
    let stderr_task = tokio::spawn(async move { read_bounded_stderr(stderr).await });

    let timeout = Duration::from_millis(config.timeout_ms());
    let wait_result = tokio::time::timeout(timeout, child.wait()).await;

    match wait_result {
        Ok(result) => complete_non_timeout(result, stderr_task, fd4_task, fd3_task).await,
        Err(_) => complete_timeout(child, pid, start, stderr_task, fd4_task, fd3_task).await,
    }
}

#[cfg(not(unix))]
async fn run_subprocess_unix(_config: SubprocessConfig) -> Result<SubprocessOutput, IpcError> {
    Err(IpcError::UnsupportedPlatform)
}

#[cfg(unix)]
fn configure_child_fds(command: &mut Command, fd3_read: &OwnedFd, fd4_write: &OwnedFd) {
    let fd3_read_raw = fd3_read.as_raw_fd();
    let fd4_write_raw = fd4_write.as_raw_fd();

    // SAFETY: pre_exec only runs in the freshly forked child. The closure only calls
    // async-signal-safe libc functions to establish the process group and wire the
    // dedicated pipe endpoints onto file descriptors 3 and 4 before exec.
    unsafe {
        command.pre_exec(move || {
            wire_child_fd(fd3_read_raw, FD3_NUMBER)?;
            wire_child_fd(fd4_write_raw, FD4_NUMBER)?;

            if libc::setpgid(0, 0) != 0 {
                return Err(Error::last_os_error());
            }

            Ok(())
        });
    }
}

#[cfg(unix)]
fn wire_child_fd(source: RawFd, target: RawFd) -> std::io::Result<()> {
    if source == target {
        return clear_cloexec(target);
    }

    // SAFETY: both descriptors are valid in the child process. dup2 installs the source onto
    // the target descriptor without taking ownership of source.
    if unsafe { libc::dup2(source, target) } == -1 {
        return Err(Error::last_os_error());
    }

    // SAFETY: source is still valid after dup2 and is intentionally closed so the child keeps
    // only the requested public descriptor number.
    if unsafe { libc::close(source) } == -1 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

#[cfg(unix)]
fn clear_cloexec(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: fcntl queries the existing descriptor flags for a valid descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        return Err(Error::last_os_error());
    }

    // SAFETY: fcntl updates the descriptor flags for the valid descriptor. The new value only
    // clears FD_CLOEXEC so the child keeps fd3/fd4 across exec.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } == -1 {
        return Err(Error::last_os_error());
    }

    Ok(())
}

#[cfg(unix)]
fn create_pipe() -> Result<(OwnedFd, OwnedFd), IpcError> {
    let mut descriptors = [0_i32; 2];

    // SAFETY: descriptors points to storage for two file descriptors. pipe2 initializes both on
    // success and O_CLOEXEC prevents inheritance of the original pipe endpoints.
    let created = unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) } == 0;

    created
        .then(|| {
            // SAFETY: pipe2 succeeded, so each descriptor is valid and uniquely owned.
            let read_fd = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
            // SAFETY: pipe2 succeeded, so each descriptor is valid and uniquely owned.
            let write_fd = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
            (read_fd, write_fd)
        })
        .ok_or_else(|| IpcError::PipeSetupFailed {
            detail: Error::last_os_error().to_string(),
        })
}

fn spawn_fd3_writer(fd: OwnedFd, payload: Vec<u8>) -> JoinHandle<std::io::Result<()>> {
    tokio::task::spawn_blocking(move || {
        let mut file = File::from(fd);
        file.write_all(&payload)?;
        file.flush()
    })
}

fn spawn_fd4_reader(fd: OwnedFd) -> JoinHandle<std::io::Result<Vec<u8>>> {
    tokio::task::spawn_blocking(move || read_fd4_payload(File::from(fd)))
}

fn read_fd4_payload(mut file: File) -> std::io::Result<Vec<u8>> {
    let mut first = [0_u8; 1];
    let first_read = file.read(&mut first)?;

    if first_read == 0 {
        return Ok(Vec::new());
    }

    let mut rest = [0_u8; 3];
    file.read_exact(&mut rest)?;

    let header = [first[0], rest[0], rest[1], rest[2]];
    let payload_u32 = u32::from_be_bytes(header);
    
    if payload_u32 > envelope::MAX_PAYLOAD_SIZE {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("fd4 payload too large: {payload_u32} bytes (max {})", envelope::MAX_PAYLOAD_SIZE),
        ));
    }

    let payload_length = usize::try_from(payload_u32).map_err(|_| {
        Error::new(
            ErrorKind::InvalidData,
            "fd4 payload length does not fit in usize",
        )
    })?;

    let mut payload = vec![0_u8; payload_length];
    file.read_exact(&mut payload)?;
    Ok(payload)
}

async fn complete_non_timeout(
    result: std::io::Result<std::process::ExitStatus>,
    stderr_task: JoinHandle<std::io::Result<StderrCapture>>,
    fd4_task: JoinHandle<std::io::Result<Vec<u8>>>,
    fd3_task: JoinHandle<std::io::Result<()>>,
) -> Result<SubprocessOutput, IpcError> {
    let exit_status = result.map_err(|error| IpcError::WaitFailed {
        detail: error.to_string(),
    })?;

    let stderr_capture = join_stderr_task(stderr_task).await?;

    if exit_status.success() {
        if let Err(error) = join_fd3_task(fd3_task).await {
            tracing::warn!("secondary error joining fd3 task after process success: {error}");
        }
        let fd4_bytes = join_fd4_task(fd4_task).await?;

        Ok(SubprocessOutput {
            fd4_bytes,
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        })
    } else {
        if let Err(error) = join_fd3_task(fd3_task).await {
            tracing::warn!("secondary error joining fd3 task after process failure: {error}");
        }

        if let Err(error) = join_fd4_task(fd4_task).await {
            tracing::warn!("secondary error joining fd4 task after process failure: {error}");
        }

        Err(IpcError::ProcessFailed {
            exit_code: map_exit_code(exit_status),
            stderr_bytes: stderr_capture.bytes,
            stderr_truncated: stderr_capture.truncated,
        })
    }
}

#[cfg(unix)]
async fn complete_timeout(
    mut child: tokio::process::Child,
    pid: i32,
    start: Instant,
    stderr_task: JoinHandle<std::io::Result<StderrCapture>>,
    fd4_task: JoinHandle<std::io::Result<Vec<u8>>>,
    fd3_task: JoinHandle<std::io::Result<()>>,
) -> Result<SubprocessOutput, IpcError> {
    send_signal_to_process_group(pid, libc::SIGTERM)?;

    let grace_result = tokio::time::timeout(SIGTERM_GRACE_PERIOD, child.wait()).await;

    if grace_result.is_err() {
        send_signal_to_process_group(pid, libc::SIGKILL)?;
        child.wait().await.map_err(|error| IpcError::WaitFailed {
            detail: error.to_string(),
        })?;
    } else {
        grace_result
            .map_err(|_| IpcError::WaitFailed {
                detail: String::from("grace-period wait unexpectedly timed out"),
            })?
            .map_err(|error| IpcError::WaitFailed {
                detail: error.to_string(),
            })?;
    }

    let stderr_capture = join_stderr_task(stderr_task).await?;

    if let Err(error) = join_fd3_task(fd3_task).await {
        tracing::warn!("secondary error joining fd3 task after timeout: {error}");
    }

    if let Err(error) = join_fd4_task(fd4_task).await {
        tracing::warn!("secondary error joining fd4 task after timeout: {error}");
    }

    Err(IpcError::Timeout {
        elapsed_ms: elapsed_ms(start),
        stderr_bytes: stderr_capture.bytes,
        stderr_truncated: stderr_capture.truncated,
    })
}

async fn join_fd4_task(task: JoinHandle<std::io::Result<Vec<u8>>>) -> Result<Vec<u8>, IpcError> {
    task.await
        .map_err(|error| IpcError::Fd4ReadFailed {
            detail: error.to_string(),
        })?
        .map_err(|error| IpcError::Fd4ReadFailed {
            detail: error.to_string(),
        })
}

async fn join_fd3_task(task: JoinHandle<std::io::Result<()>>) -> Result<(), IpcError> {
    task.await
        .map_err(|error| IpcError::Fd3WriteFailed {
            detail: error.to_string(),
        })?
        .map_err(|error| IpcError::Fd3WriteFailed {
            detail: error.to_string(),
        })
}

async fn join_stderr_task(
    task: JoinHandle<std::io::Result<StderrCapture>>,
) -> Result<StderrCapture, IpcError> {
    task.await
        .map_err(|error| IpcError::StderrReadFailed {
            detail: error.to_string(),
        })?
        .map_err(|error| IpcError::StderrReadFailed {
            detail: error.to_string(),
        })
}

#[cfg(unix)]
fn send_signal_to_process_group(pid: i32, signal: i32) -> Result<(), IpcError> {
    let target = pid.checked_neg().ok_or_else(|| IpcError::SignalFailed {
        detail: format!("cannot negate pid {pid} for process-group signaling"),
    })?;

    // SAFETY: kill sends the requested signal to the child's dedicated process group.
    let outcome = unsafe { libc::kill(target, signal) };

    if outcome == 0 {
        return Ok(());
    }

    let error = Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(IpcError::SignalFailed {
            detail: error.to_string(),
        })
    }
}

#[cfg(not(unix))]
fn send_signal_to_process_group(_pid: i32, _signal: i32) -> Result<(), IpcError> {
    Err(IpcError::UnsupportedPlatform)
}

#[must_use]
pub(crate) fn map_exit_code(status: std::process::ExitStatus) -> i32 {
    let signal_code = status.signal().map_or(0, |signal| 128 + signal);
    status.code().map_or(signal_code, |code| code)
}

#[must_use]
#[cfg(test)]
pub(crate) fn encode_fd4_payload(payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).unwrap_or(u32::MAX);

    let mut bytes = length.to_be_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

#[must_use]
fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).map_or(u64::MAX, |value| value)
}
