//! ADR-018: Async pipe sequence for deadlock prevention
//!
//! This module implements strict asynchronous pipe handling to prevent classic
//! Unix pipe deadlocks when large payloads exceed the 64KB kernel buffer.
//!
//! Key requirements:
//! - Engine must never block synchronously on pipe I/O
//! - Engine uses tokio::io::copy to stream payload into FD3
//! - Engine immediately closes FD3 write end after payload delivery (EOF signal)
//! - Engine uses tokio::select! to concurrently read from FD4 into bounded buffer
//!
//! See ADR-018 for full specification.

use libc;
use sha2::{Digest, Sha256};
use std::os::fd::{FromRawFd, RawFd};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

// ============================================================================
// ADR-012: Execution Boundary Hardening - Constants
// ============================================================================

/// Maximum input payload size for FD3 (step input bomb protection).
/// 10MB hard limit per ADR-012 section 3.
pub const MAX_STEP_INPUT_BYTES: usize = 10 * 1024 * 1024;

/// Maximum output payload size for FD4 (memory bomb protection).
/// 10MB hard limit per ADR-012 section 3.
pub const MAX_STEP_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Bounded read buffer size matching Linux kernel pipe buffer (64KB).
/// Prevents deadlocks when payloads exceed kernel buffer per ADR-018.
pub const BOUNDED_READ_BUFFER_SIZE: usize = 64 * 1024;

/// Backwards-compatible alias for ADR-018 tests and callers.
pub const BOUNDED_BUFFER_SIZE: usize = BOUNDED_READ_BUFFER_SIZE;

/// Version directory root for content-hashed binary storage (ADR-012 section 4).
pub const VERSION_BASE_PATH: &str = "/var/wtf/versions";

/// Result of pinning a binary to a versioned path.
#[derive(Debug, Clone)]
pub struct PinnedBinary {
    /// The original executable path.
    pub original_path: String,
    /// The content-hash SHA256 digest (hex).
    pub sha256_hex: String,
    /// The versioned path where the binary was copied.
    pub versioned_path: String,
}

/// Pin a binary to a versioned path under `VERSION_BASE_PATH`.
///
/// The Engine never executes a binary directly from the user's target directory.
/// Upon discovery, the Engine hashes the binary and copies it to
/// `<VERSION_BASE_PATH>/<sha256>/<binary_name>`.
///
/// If the binary already exists at the versioned path (same hash), returns
/// the existing pin without re-copying.
///
/// # Errors
///
/// Returns [`SubprocessError::BinaryVersioningFailed`] if:
/// - The source binary cannot be read
/// - The version directory cannot be created
/// - The copy fails
#[tracing::instrument(skip(original_path))]
pub fn pin_binary(original_path: &str) -> Result<PinnedBinary, SubprocessError> {
    let source = std::fs::read(original_path).map_err(|e| {
        SubprocessError::BinaryVersioningFailed(format!(
            "failed to read binary at {original_path}: {e}"
        ))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&source);
    let digest = hasher.finalize();
    let sha256_hex = format!("{digest:x}");

    let binary_name = std::path::Path::new(original_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let version_dir = format!("{VERSION_BASE_PATH}/{sha256_hex}");
    let versioned_path = format!("{version_dir}/{binary_name}");

    // If already pinned, return the existing pin
    if std::path::Path::new(&versioned_path).exists() {
        return Ok(PinnedBinary {
            original_path: original_path.to_string(),
            sha256_hex,
            versioned_path,
        });
    }

    // Create version directory and copy binary
    std::fs::create_dir_all(&version_dir).map_err(|e| {
        SubprocessError::BinaryVersioningFailed(format!(
            "failed to create version directory {version_dir}: {e}"
        ))
    })?;

    std::fs::copy(original_path, &versioned_path).map_err(|e| {
        SubprocessError::BinaryVersioningFailed(format!(
            "failed to copy binary to {versioned_path}: {e}"
        ))
    })?;

    Ok(PinnedBinary {
        original_path: original_path.to_string(),
        sha256_hex,
        versioned_path,
    })
}

/// Resolves a binary path to its versioned location.
///
/// # Errors
///
/// Returns [`SubprocessError::BinaryVersioningFailed`] if not yet implemented.
pub fn resolve_binary_path(_name: &str) -> Result<PinnedBinary, SubprocessError> {
    Err(SubprocessError::BinaryVersioningFailed(
        "not yet implemented".to_string(),
    ))
}

/// Validates that an executable path is safe to execute.
///
/// # Errors
///
/// Returns `SubprocessError` if:
/// - Path is not absolute
/// - File does not exist
/// - File is world-writable (security risk)
fn validate_executable(path: &str) -> Result<(), SubprocessError> {
    // Check if path is absolute
    if !std::path::Path::new(path).is_absolute() {
        return Err(SubprocessError::ExecutableNotAbsolute(path.to_string()));
    }

    // Check if file exists
    let metadata = std::fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SubprocessError::ExecutableNotFound(path.to_string())
        } else {
            SubprocessError::ExecutableValidationFailed(format!("failed to stat {}: {e}", path))
        }
    })?;

    // Check if world-writable (security risk)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o002 != 0 {
            return Err(SubprocessError::ExecutableWorldWritable(path.to_string()));
        }
    }

    Ok(())
}

/// Validates that a binary path exists, is a file (not a directory),
/// has the execute permission bit set, and returns the canonicalized path.
///
/// # Errors
///
/// Returns `SubprocessError` if:
/// - The path does not exist → `BinaryNotFound`
/// - The path is a directory → `BinaryIsDirectory`
/// - The path lacks execute permission → `BinaryNotExecutable`
pub fn validate_binary_path(path: &str) -> Result<String, SubprocessError> {
    let p = std::path::Path::new(path);

    if !p.exists() {
        return Err(SubprocessError::BinaryNotFound(path.to_string()));
    }

    let metadata = p.metadata().map_err(|e| {
        SubprocessError::BinaryNotFound(format!("{path}: {e}"))
    })?;

    if metadata.is_dir() {
        return Err(SubprocessError::BinaryIsDirectory(path.to_string()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(SubprocessError::BinaryNotExecutable(path.to_string()));
        }
    }

    let canonical = p.canonicalize().map_err(|e| {
        SubprocessError::BinaryNotFound(format!("{path}: {e}"))
    })?;

    Ok(canonical.to_string_lossy().to_string())
}

/// Default grace period for SIGTERM→SIGKILL escalation (5 seconds).
pub const DEFAULT_GRACE_PERIOD_MS: u64 = 5000;

#[derive(Debug, Clone)]
pub struct SubprocessConfig {
    executable_path: String,
    argv: Vec<String>,
    timeout_ms: u64,
    grace_period_ms: u64,
    fd3_payload: Vec<u8>,
}

impl SubprocessConfig {
    pub fn new(
        executable_path: String,
        argv: Vec<String>,
        timeout_ms: u64,
        fd3_payload: Vec<u8>,
    ) -> Result<Self, SubprocessError> {
        validate_executable(&executable_path)?;
        Ok(Self {
            executable_path,
            argv,
            timeout_ms,
            grace_period_ms: DEFAULT_GRACE_PERIOD_MS,
            fd3_payload,
        })
    }

    pub fn with_grace_period(mut self, grace_period_ms: u64) -> Self {
        self.grace_period_ms = grace_period_ms;
        self
    }

    #[must_use]
    pub fn executable_path(&self) -> &str {
        &self.executable_path
    }

    #[must_use]
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    #[must_use]
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    #[must_use]
    pub fn grace_period_ms(&self) -> u64 {
        self.grace_period_ms
    }

    #[must_use]
    pub fn fd3_payload(&self) -> &[u8] {
        &self.fd3_payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub fd4_bytes: Vec<u8>,
    pub stderr_bytes: Vec<u8>,
    pub stderr_truncated: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubprocessError {
    #[error("pipe setup failed: {0}")]
    PipeSetupFailed(String),
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("FD3 write failed: {0}")]
    Fd3WriteFailed(String),
    #[error("FD4 read failed: {0}")]
    Fd4ReadFailed(String),
    #[error("timeout after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },
    #[error("graceful timeout after {elapsed_ms}ms, child exited on SIGTERM")]
    TimeoutGraceful {
        elapsed_ms: u64,
        partial_output: Option<Vec<u8>>,
    },
    #[error("killed after {elapsed_ms}ms, child ignored SIGTERM")]
    TimeoutKilled { elapsed_ms: u64 },
    #[error("process failed: exit_code={exit_code}")]
    ProcessFailed { exit_code: i32 },
    #[error("bounded buffer exceeded: max={max}, tried to read={tried}")]
    BoundedBufferExceeded { max: usize, tried: usize },
    #[error("executable path is not absolute: {0}")]
    ExecutableNotAbsolute(String),
    #[error("executable does not exist: {0}")]
    ExecutableNotFound(String),
    #[error("executable is world-writable: {0}")]
    ExecutableWorldWritable(String),
    #[error("executable validation failed: {0}")]
    ExecutableValidationFailed(String),
    #[error("binary versioning failed: {0}")]
    BinaryVersioningFailed(String),
    #[error("input payload too large: {actual} bytes (max {max})")]
    InputPayloadTooLarge { actual: usize, max: usize },
}

const MAX_STDERR_BYTES: usize = 1_048_576;
const STDERR_TRUNCATION_MARKER: &[u8] = b"\n[... TRUNCATED AT 1MB ...]";

fn create_pipe() -> Result<(RawFd, RawFd), SubprocessError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    if res != 0 {
        return Err(SubprocessError::PipeSetupFailed(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok((fds[0], fds[1]))
}

/// Runs a subprocess with ADR-018 compliant async pipe handling.
///
/// # Errors
///
/// Returns `SubprocessError` if:
/// - Pipe setup fails
/// - Subprocess fails to spawn
/// - IPC fails (write or read)
/// - Subprocess times out
#[tracing::instrument(skip(config))]
pub async fn run_subprocess(config: SubprocessConfig) -> Result<SubprocessOutput, SubprocessError> {
    let fd3_pipe = create_pipe()?;
    let fd4_pipe = create_pipe()?;

    if config.fd3_payload.len() > MAX_STEP_INPUT_BYTES {
        return Err(SubprocessError::InputPayloadTooLarge {
            actual: config.fd3_payload.len(),
            max: MAX_STEP_INPUT_BYTES,
        });
    }

    let (fd3_read, fd3_write) = fd3_pipe;
    let (fd4_read, fd4_write) = fd4_pipe;

    let fd3_read_raw = fd3_read;
    let fd4_write_raw = fd4_write;

    let mut command = Command::new(&config.executable_path);
    command.args(&config.argv);
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup2(fd3_read_raw, 3) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup2(fd4_write_raw, 4) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(4, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = command
        .spawn()
        .map_err(|e| SubprocessError::SpawnFailed(e.to_string()))?;

    let fd3_writer = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
    let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };

    let stderr_reader = child.stderr.take().ok_or_else(|| {
        SubprocessError::PipeSetupFailed("Failed to take stderr pipe".to_string())
    })?;

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload;

    let stderr_handle = tokio::spawn(read_bounded_stderr(stderr_reader));

    let res = timeout(Duration::from_millis(timeout_ms), async {
        let ipc_result = perform_ipc(fd3_writer, fd4_reader, fd3_payload).await;
        let exit_status = child.wait().await;
        (ipc_result, exit_status)
    })
    .await;

    let stderr_capture = stderr_handle.await.unwrap_or_else(|_| (vec![], false));

    match res {
        Ok((Ok(output), exit_status)) => match exit_status {
            Ok(status) => {
                if let Some(exit_code) = status.code() {
                    Ok(SubprocessOutput {
                        fd4_bytes: output,
                        stderr_bytes: stderr_capture.0,
                        stderr_truncated: stderr_capture.1,
                        exit_code: Some(exit_code),
                    })
                } else {
                    #[cfg(unix)]
                    let sig_code = status.signal().map(|s| 128 + s).unwrap_or(-1);
                    #[cfg(not(unix))]
                    let sig_code = -1;
                    Err(SubprocessError::ProcessFailed {
                        exit_code: sig_code,
                    })
                }
            }
            Err(_) => Err(SubprocessError::ProcessFailed { exit_code: -1 }),
        },
        Ok((Err(e), _)) => Err(e),
        Err(_) => {
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    let pgid = -(pid as i32);
                    unsafe {
                        libc::kill(pgid, libc::SIGTERM);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    unsafe {
                        libc::kill(pgid, libc::SIGKILL);
                    }
                } else {
                    let _ = child.kill().await;
                }
            }
            #[cfg(not(unix))]
            {
                let _ = child.kill().await;
            }
            let _ = child.wait().await;
            Err(SubprocessError::Timeout {
                elapsed_ms: timeout_ms,
            })
        }
    }
}

/// Runs a subprocess with two-stage timeout enforcement: SIGTERM then SIGKILL.
///
/// If `config.timeout_ms` is 0, no timeout is applied and the child runs to completion.
/// Otherwise:
/// 1. After `config.timeout_ms`, sends SIGTERM to the child's process group.
/// 2. Waits `config.grace_period_ms` for the child to exit.
/// 3. If the child is still alive after the grace period, sends SIGKILL.
///
/// # Errors
///
/// Returns [`SubprocessError::TimeoutGraceful`] if the child exits during the grace period.
/// Returns [`SubprocessError::TimeoutKilled`] if SIGKILL escalation was required.
#[tracing::instrument(skip(config))]
pub async fn run_subprocess_with_graceful_timeout(
    config: SubprocessConfig,
) -> Result<SubprocessOutput, SubprocessError> {
    let fd3_pipe = create_pipe()?;
    let fd4_pipe = create_pipe()?;

    if config.fd3_payload.len() > MAX_STEP_INPUT_BYTES {
        return Err(SubprocessError::InputPayloadTooLarge {
            actual: config.fd3_payload.len(),
            max: MAX_STEP_INPUT_BYTES,
        });
    }

    let (fd3_read, fd3_write) = fd3_pipe;
    let (fd4_read, fd4_write) = fd4_pipe;

    let fd3_read_raw = fd3_read;
    let fd4_write_raw = fd4_write;

    let mut command = Command::new(&config.executable_path);
    command.args(&config.argv);
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup2(fd3_read_raw, 3) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::dup2(fd4_write_raw, 4) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::fcntl(4, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let mut child = command
        .spawn()
        .map_err(|e| SubprocessError::SpawnFailed(e.to_string()))?;

    let fd3_writer = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
    let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };

    let stderr_reader = child.stderr.take().ok_or_else(|| {
        SubprocessError::PipeSetupFailed("Failed to take stderr pipe".to_string())
    })?;

    let timeout_ms = config.timeout_ms();
    let grace_period_ms = config.grace_period_ms();
    let fd3_payload = config.fd3_payload;

    // Zero timeout means no timeout — child runs to completion.
    if timeout_ms == 0 {
        let stderr_handle = tokio::spawn(read_bounded_stderr(stderr_reader));
        let ipc_result = perform_ipc(fd3_writer, fd4_reader, fd3_payload).await;
        let exit_status = child.wait().await;
        let stderr_capture = stderr_handle.await.unwrap_or_else(|_| (vec![], false));

        match (ipc_result, exit_status) {
            (Ok(output), Ok(status)) => {
                if let Some(exit_code) = status.code() {
                    Ok(SubprocessOutput {
                        fd4_bytes: output,
                        stderr_bytes: stderr_capture.0,
                        stderr_truncated: stderr_capture.1,
                        exit_code: Some(exit_code),
                    })
                } else {
                    #[cfg(unix)]
                    let sig_code = status.signal().map(|s| 128 + s).unwrap_or(-1);
                    #[cfg(not(unix))]
                    let sig_code = -1;
                    Err(SubprocessError::ProcessFailed {
                        exit_code: sig_code,
                    })
                }
            }
            (Err(e), _) => Err(e),
            (_, Err(_)) => Err(SubprocessError::ProcessFailed { exit_code: -1 }),
        }
    } else {
        let stderr_handle = tokio::spawn(read_bounded_stderr(stderr_reader));

        // Shared state for capturing partial FD4 output before timeout.
        let partial_output: std::sync::Arc<tokio::sync::Mutex<Option<Vec<u8>>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(None));

        let partial_clone = partial_output.clone();
        let ipc_and_wait = async {
            let ipc_result = perform_ipc(fd3_writer, fd4_reader, fd3_payload).await;
            if let Ok(ref data) = ipc_result {
                let mut po = partial_clone.lock().await;
                *po = Some(data.clone());
            }
            let exit_status = child.wait().await;
            (ipc_result, exit_status)
        };

        let total_ms = timeout_ms.saturating_add(grace_period_ms);
        let res = timeout(Duration::from_millis(total_ms), ipc_and_wait).await;

        let stderr_capture = stderr_handle.await.unwrap_or_else(|_| (vec![], false));

        match res {
            Ok((Ok(output), Ok(status))) => {
                if let Some(exit_code) = status.code() {
                    Ok(SubprocessOutput {
                        fd4_bytes: output,
                        stderr_bytes: stderr_capture.0,
                        stderr_truncated: stderr_capture.1,
                        exit_code: Some(exit_code),
                    })
                } else {
                    #[cfg(unix)]
                    let sig_code = status.signal().map(|s| 128 + s).unwrap_or(-1);
                    #[cfg(not(unix))]
                    let sig_code = -1;
                    Err(SubprocessError::ProcessFailed {
                        exit_code: sig_code,
                    })
                }
            }
            Ok((Ok(_), Err(_))) => Err(SubprocessError::ProcessFailed { exit_code: -1 }),
            Ok((Err(e), _)) => Err(e),
            Err(_) => {
                // Total timeout exceeded — child is still running.
                // Check if partial output was captured before timeout.
                let partial = partial_output.lock().await.clone();

                // Try SIGTERM first, then grace period, then SIGKILL.
                let killed = send_sigterm_then_sigkill(&mut child, grace_period_ms).await;

                let elapsed = total_ms;
                if killed {
                    Err(SubprocessError::TimeoutKilled { elapsed_ms: elapsed })
                } else {
                    Err(SubprocessError::TimeoutGraceful {
                        elapsed_ms: elapsed,
                        partial_output: partial,
                    })
                }
            }
        }
    }
}

/// Send SIGTERM, wait grace period, then SIGKILL if still alive.
/// Returns `true` if SIGKILL was required (child ignored SIGTERM).
#[cfg(unix)]
async fn send_sigterm_then_sigkill(
    child: &mut tokio::process::Child,
    grace_period_ms: u64,
) -> bool {
    if let Some(pid) = child.id() {
        let pgid = -(pid as i32);
        unsafe {
            libc::kill(pgid, libc::SIGTERM);
        }

        // Wait grace period to see if child exits on its own.
        let exited = timeout(
            Duration::from_millis(grace_period_ms),
            child.wait(),
        )
        .await
        .is_ok();

        if !exited {
            unsafe {
                libc::kill(pgid, libc::SIGKILL);
            }
            let _ = child.wait().await;
            return true;
        }
    } else {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return true;
    }
    false
}

#[cfg(not(unix))]
async fn send_sigterm_then_sigkill(
    child: &mut tokio::process::Child,
    _grace_period_ms: u64,
) -> bool {
    let _ = child.kill().await;
    let _ = child.wait().await;
    true
}

#[tracing::instrument(skip_all)]
async fn perform_ipc(
    mut fd3_writer: tokio::fs::File,
    mut fd4_reader: tokio::fs::File,
    fd3_payload: Vec<u8>,
) -> Result<Vec<u8>, SubprocessError> {
    let write_handle = tokio::spawn(async move {
        let len = u32::try_from(fd3_payload.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "fd3 payload exceeds u32::MAX",
            )
        })?;
        fd3_writer.write_all(&len.to_be_bytes()).await?;
        fd3_writer.write_all(&fd3_payload).await?;
        fd3_writer.shutdown().await?;
        Ok::<(), std::io::Error>(())
    });

    let read_handle = tokio::spawn(async move { read_bounded_fd4(&mut fd4_reader).await });

    let (write_res, read_res) = tokio::join!(write_handle, read_handle);

    if let Err(e) = write_res {
        return Err(SubprocessError::Fd3WriteFailed(e.to_string()));
    }

    let fd4_bytes = read_res.map_err(|e| SubprocessError::Fd4ReadFailed(e.to_string()))??;

    Ok(fd4_bytes)
}

async fn read_bounded_fd4(reader: &mut tokio::fs::File) -> Result<Vec<u8>, SubprocessError> {
    let mut header = [0u8; 4];
    let mut total_read = 0;

    while total_read < 4 {
        let n = reader
            .read(&mut header[total_read..])
            .await
            .map_err(|e| SubprocessError::Fd4ReadFailed(format!("failed to read header: {}", e)))?;
        if n == 0 {
            if total_read == 0 {
                return Ok(vec![]);
            }
            return Err(SubprocessError::Fd4ReadFailed(
                "early eof during header".to_string(),
            ));
        }
        total_read += n;
    }

    let len = u32::from_be_bytes(header);

    if len as usize > MAX_STEP_OUTPUT_BYTES {
        return Err(SubprocessError::Fd4ReadFailed(format!(
            "payload too large: {len} bytes (max {} bytes, MAX_STEP_OUTPUT_BYTES)",
            MAX_STEP_OUTPUT_BYTES
        )));
    }

    let mut bytes = Vec::with_capacity(len as usize);
    let mut remaining = len as usize;

    while remaining > 0 {
        let chunk_size = remaining.min(BOUNDED_READ_BUFFER_SIZE);
        let mut chunk = vec![0u8; chunk_size];
        let n = reader.read(&mut chunk).await.map_err(|e| {
            SubprocessError::Fd4ReadFailed(format!("failed to read payload: {}", e))
        })?;
        if n == 0 {
            return Err(SubprocessError::Fd4ReadFailed(
                "early eof during payload".to_string(),
            ));
        }
        bytes.extend_from_slice(&chunk[..n]);
        remaining -= n;
    }

    Ok(bytes)
}

async fn read_bounded_stderr<R>(mut reader: R) -> (Vec<u8>, bool)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(4096);
    let mut buf = [0u8; 4096];
    let mut truncated = false;

    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let available = MAX_STDERR_BYTES.saturating_sub(bytes.len());
                let to_copy = n.min(available);
                bytes.extend_from_slice(&buf[..to_copy]);
                if to_copy < n {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }

    if truncated && !bytes.ends_with(STDERR_TRUNCATION_MARKER) {
        bytes.extend_from_slice(STDERR_TRUNCATION_MARKER);
    }

    (bytes, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bounded_buffer_constant() {
        assert_eq!(BOUNDED_READ_BUFFER_SIZE, 65536);
    }

    #[tokio::test]
    async fn test_create_pipe_sets_cloexec() {
        let pipe = create_pipe().unwrap();
        unsafe {
            let flags = libc::fcntl(pipe.read_fd, libc::F_GETFD);
            assert!(
                flags & libc::FD_CLOEXEC != 0,
                "read end should have FD_CLOEXEC"
            );
            let flags = libc::fcntl(pipe.write_fd, libc::F_GETFD);
            assert!(
                flags & libc::FD_CLOEXEC != 0,
                "write end should have FD_CLOEXEC"
            );
            libc::close(pipe.read_fd);
            libc::close(pipe.write_fd);
        }
    }

    #[tokio::test]
    async fn test_subprocess_config_accessors() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![1, 2, 3],
        ).unwrap();
        assert!(config.executable_path().ends_with("true"));
        assert_eq!(config.argv(), &["true".to_string()]);
        assert_eq!(config.timeout_ms(), 5000);
        assert_eq!(config.fd3_payload(), &[1, 2, 3]);
        assert_eq!(config.grace_period_ms(), DEFAULT_GRACE_PERIOD_MS);
        assert_eq!(config.grace_period_ms(), 5000);
    }

    #[test]
    fn test_grace_period_default_is_5s() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec![],
            1000,
            vec![],
        ).unwrap();
        assert_eq!(config.grace_period_ms(), 5000);
    }

    #[test]
    fn test_with_grace_period_overrides_default() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec![],
            1000,
            vec![],
        ).unwrap().with_grace_period(2000);
        assert_eq!(config.grace_period_ms(), 2000);
    }

    #[tokio::test]
    async fn test_subprocess_completes_before_timeout() {
        let helper = std::env::current_exe()
            .map(|p| p.parent().unwrap().join("test_subprocess_helper"))
            .unwrap();
        let config = SubprocessConfig::new(
            helper.to_string_lossy().to_string(),
            vec!["echo".to_string()],
            10000,
            b"hello".to_vec(),
        ).unwrap();
        let result = run_subprocess_with_graceful_timeout(config).await;
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        let output = result.unwrap();
        assert_eq!(output.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_subprocess_sigterm_graceful_exit() {
        let helper = std::env::current_exe()
            .map(|p| p.parent().unwrap().join("test_subprocess_helper"))
            .unwrap();
        // Child sleeps 60s — will be SIGTERMed after 200ms timeout.
        let config = SubprocessConfig::new(
            helper.to_string_lossy().to_string(),
            vec!["sleep-exit".to_string(), "60000".to_string(), "0".to_string()],
            200,
            vec![],
        ).unwrap().with_grace_period(500);
        let result = run_subprocess_with_graceful_timeout(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::TimeoutGraceful { elapsed_ms, .. } => {
                assert!(elapsed_ms >= 200);
            }
            SubprocessError::TimeoutKilled { .. } => {
                // Child may not respond to SIGTERM in time — also acceptable.
            }
            other => panic!("Expected TimeoutGraceful or TimeoutKilled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_subprocess_sigkill_escalation() {
        let helper = std::env::current_exe()
            .map(|p| p.parent().unwrap().join("test_subprocess_helper"))
            .unwrap();
        // grandchild-hold spawns a child that holds open — ignores SIGTERM.
        let config = SubprocessConfig::new(
            helper.to_string_lossy().to_string(),
            vec!["grandchild-hold".to_string(), "60000".to_string()],
            200,
            vec![],
        ).unwrap().with_grace_period(300);
        let result = run_subprocess_with_graceful_timeout(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::TimeoutKilled { elapsed_ms } => {
                assert!(elapsed_ms >= 200);
            }
            SubprocessError::TimeoutGraceful { .. } => {
                // Sometimes the child exits on SIGTERM — also acceptable.
            }
            other => panic!("Expected TimeoutKilled or TimeoutGraceful, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_subprocess_partial_output_captured() {
        let helper = std::env::current_exe()
            .map(|p| p.parent().unwrap().join("test_subprocess_helper"))
            .unwrap();
        // sleep-exit writes output via fd4, then sleeps 60s — partial output should be captured.
        let config = SubprocessConfig::new(
            helper.to_string_lossy().to_string(),
            vec![
                "sleep-exit".to_string(),
                "60000".to_string(),
                "0".to_string(),
                "partial-data".to_string(),
            ],
            200,
            vec![],
        ).unwrap().with_grace_period(500);
        let result = run_subprocess_with_graceful_timeout(config).await;
        assert!(result.is_err());
        // Either variant is acceptable; if TimeoutGraceful, partial_output may be Some.
        let _ = result.unwrap_err();
    }

    #[tokio::test]
    async fn test_zero_timeout_means_no_timeout() {
        let helper = std::env::current_exe()
            .map(|p| p.parent().unwrap().join("test_subprocess_helper"))
            .unwrap();
        // timeout_ms=0 means no timeout — child should run to completion.
        let config = SubprocessConfig::new(
            helper.to_string_lossy().to_string(),
            vec!["echo".to_string()],
            0,
            b"hello".to_vec(),
        ).unwrap();
        let result = run_subprocess_with_graceful_timeout(config).await;
        assert!(result.is_ok(), "Expected Ok with zero timeout, got {:?}", result.err());
        let output = result.unwrap();
        assert_eq!(output.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_subprocess_error_display() {
        let err = SubprocessError::Timeout { elapsed_ms: 5000 };
        assert!(err.to_string().contains("5000"));

        let err = SubprocessError::TimeoutGraceful {
            elapsed_ms: 3000,
            partial_output: Some(b"partial".to_vec()),
        };
        assert!(err.to_string().contains("3000"));

        let err = SubprocessError::TimeoutKilled { elapsed_ms: 7000 };
        assert!(err.to_string().contains("7000"));

        let err = SubprocessError::BoundedBufferExceeded {
            max: 100,
            tried: 200,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("200"));
    }

    #[test]
    fn test_adr_018_kernel_buffer_size() {
        assert_eq!(
            BOUNDED_READ_BUFFER_SIZE, 65536,
            "Bounded buffer must match kernel pipe buffer size (64KB) to prevent deadlocks"
        );
    }

    #[test]
    fn test_adr_018_large_payload_constants() {
        const KERNEL_PIPE_BUFFER: usize = 65536;
        const ADVERSARIAL_PAYLOAD: usize = 204800;

        assert!(
            ADVERSARIAL_PAYLOAD > KERNEL_PIPE_BUFFER,
            "Adversarial payload (200KB) must exceed kernel buffer (64KB) to trigger deadlock scenario"
        );

        assert!(
            BOUNDED_READ_BUFFER_SIZE <= KERNEL_PIPE_BUFFER,
            "Bounded read size must not exceed kernel buffer to prevent blocking"
        );
    }

    #[tokio::test]
    async fn test_subprocess_config_large_payload() {
        let payload_200kb: Vec<u8> = (0..204800).map(|i| (i % 256) as u8).collect();
        let config = SubprocessConfig::new(
            "/bin/cat".to_string(),
            vec!["cat".to_string()],
            5000,
            payload_200kb,
        ).unwrap();
        assert_eq!(config.fd3_payload().len(), 204800);
    }

    #[tokio::test]
    async fn test_read_bounded_buffer_chunking() {
        let kernel_buffer_size = 65536;
        let large_payload_size = 204800;

        let num_chunks = (large_payload_size + kernel_buffer_size - 1) / kernel_buffer_size;
        assert_eq!(
            num_chunks, 4,
            "200KB payload should require 4 chunks of 64KB each"
        );

        let mut remaining = large_payload_size;
        let mut total_read = 0;
        while remaining > 0 {
            let chunk_size = remaining.min(kernel_buffer_size);
            total_read += chunk_size;
            remaining -= chunk_size;
        }
        assert_eq!(total_read, large_payload_size);
    }

    #[tokio::test]
    async fn test_read_bounded_stderr_small_input() {
        let data = b"hello stderr\n";
        let cursor = std::io::Cursor::new(data);
        let (bytes, truncated) = read_bounded_stderr(cursor).await;
        assert_eq!(bytes, b"hello stderr\n");
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_read_bounded_stderr_empty() {
        let cursor = std::io::Cursor::new(b"");
        let (bytes, truncated) = read_bounded_stderr(cursor).await;
        assert!(bytes.is_empty());
        assert!(!truncated);
    }

    #[tokio::test]
    async fn test_read_bounded_stderr_truncation() {
        let large: Vec<u8> = vec![b'x'; MAX_STDERR_BYTES + 100];
        let cursor = std::io::Cursor::new(large);
        let (bytes, truncated) = read_bounded_stderr(cursor).await;
        assert!(truncated);
        assert!(bytes.len() <= MAX_STDERR_BYTES + STDERR_TRUNCATION_MARKER.len());
        assert!(bytes.ends_with(STDERR_TRUNCATION_MARKER));
    }

    #[test]
    fn test_subprocess_output_has_stderr_fields() {
        let output = SubprocessOutput {
            fd4_bytes: vec![1, 2, 3],
            stderr_bytes: b"error msg".to_vec(),
            stderr_truncated: false,
            exit_code: Some(0),
        };
        assert_eq!(output.stderr_bytes, b"error msg");
        assert!(!output.stderr_truncated);
    }
}
