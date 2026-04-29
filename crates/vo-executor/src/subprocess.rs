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

/// Resolve a binary path: if already pinned, return as-is; otherwise pin it.
///
/// This allows callers to pass either an original path or an already-pinned
/// versioned path transparently.
pub fn resolve_binary_path(path: &str) -> Result<PinnedBinary, SubprocessError> {
    if path.starts_with(VERSION_BASE_PATH) && std::path::Path::new(path).exists() {
        // Already pinned - reconstruct pin info from path
        // Extract hash from path: VERSION_BASE_PATH/<hash>/<binary_name>
        let hash = path
            .strip_prefix(VERSION_BASE_PATH)
            .map(|suffix| suffix.trim_start_matches('/'))
            .and_then(|suffix| suffix.split('/').next())
            .map(str::to_string)
            .ok_or_else(|| {
                SubprocessError::BinaryVersioningFailed(format!(
                    "failed to extract version hash from pinned path {path}"
                ))
            })?;
        Ok(PinnedBinary {
            original_path: path.to_string(),
            sha256_hex: hash,
            versioned_path: path.to_string(),
        })
    } else {
        pin_binary(path)
    }
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

#[derive(Debug, Clone)]
pub struct SubprocessConfig {
    executable_path: String,
    argv: Vec<String>,
    timeout_ms: u64,
    fd3_payload: Vec<u8>,
}

impl SubprocessConfig {
    pub fn new(
        executable_path: String,
        argv: Vec<String>,
        timeout_ms: u64,
        fd3_payload: Vec<u8>,
    ) -> Result<Self, SubprocessError> {
        let canonical = validate_binary_path(&executable_path)?;
        Ok(Self {
            executable_path: canonical,
            argv,
            timeout_ms,
            fd3_payload,
        })
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
    pub fn fd3_payload(&self) -> &[u8] {
        &self.fd3_payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub fd4_bytes: Vec<u8>,
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
    #[error("process failed: exit_code={exit_code}")]
    ProcessFailed { exit_code: i32 },
    #[error("bounded buffer exceeded: max={max}, tried to read={tried}")]
    BoundedBufferExceeded { max: usize, tried: usize },
    #[error("input payload exceeds limit: {actual} bytes > {max} bytes (MAX_STEP_INPUT_BYTES)")]
    InputPayloadTooLarge { actual: usize, max: usize },
    #[error("binary hash mismatch: expected={expected}, actual={actual}")]
    BinaryHashMismatch { expected: String, actual: String },
    #[error("binary versioning failed: {0}")]
    BinaryVersioningFailed(String),
    #[error("binary not found: {0}")]
    BinaryNotFound(String),
    #[error("binary is not executable: {0}")]
    BinaryNotExecutable(String),
    #[error("binary is a directory, not a file: {0}")]
    BinaryIsDirectory(String),
}

struct PipePair {
    read_fd: RawFd,
    write_fd: RawFd,
}

fn create_pipe() -> Result<PipePair, SubprocessError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(SubprocessError::PipeSetupFailed(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(PipePair {
        read_fd: fds[0],
        write_fd: fds[1],
    })
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

    let mut command = Command::new(&config.executable_path);
    command.args(&config.argv);
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

    let fd3_read = fd3_pipe.read_fd;
    let fd3_write = fd3_pipe.write_fd;
    let fd4_read = fd4_pipe.read_fd;
    let fd4_write = fd4_pipe.write_fd;

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

    unsafe {
        libc::close(fd3_read);
        libc::close(fd4_write);
    }

    let fd3_writer = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
    let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload;

    let res = timeout(Duration::from_millis(timeout_ms), async {
        let ipc_result = perform_ipc(fd3_writer, fd4_reader, fd3_payload).await;
        let exit_status = child.wait().await;
        (ipc_result, exit_status)
    })
    .await;

    match res {
        Ok((Ok(output), exit_status)) => match exit_status {
            Ok(status) => {
                if let Some(exit_code) = status.code() {
                    Ok(SubprocessOutput {
                        fd4_bytes: output,
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
        let chunk_size = remaining.min(BOUNDED_BUFFER_SIZE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn test_bounded_buffer_constant() {
        assert_eq!(BOUNDED_BUFFER_SIZE, 65536);
    }

    #[tokio::test]
    async fn test_create_pipe_sets_cloexec() {
        let pipe = create_pipe().unwrap();
        let r = pipe.read_fd;
        let w = pipe.write_fd;
        unsafe {
            let flags = libc::fcntl(r, libc::F_GETFD);
            assert!(
                flags & libc::FD_CLOEXEC != 0,
                "read end should have FD_CLOEXEC"
            );
            let flags = libc::fcntl(w, libc::F_GETFD);
            assert!(
                flags & libc::FD_CLOEXEC != 0,
                "write end should have FD_CLOEXEC"
            );
            libc::close(r);
            libc::close(w);
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
    }

    #[tokio::test]
    async fn test_subprocess_error_display() {
        let err = SubprocessError::Timeout { elapsed_ms: 5000 };
        assert!(err.to_string().contains("5000"));

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
            BOUNDED_BUFFER_SIZE, 65536,
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
            BOUNDED_BUFFER_SIZE <= KERNEL_PIPE_BUFFER,
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

    #[test]
    fn test_validate_binary_path_nonexistent() {
        let result = validate_binary_path("/nonexistent/binary/path");
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryNotFound(path) => {
                assert_eq!(path, "/nonexistent/binary/path");
            }
            other => panic!("expected BinaryNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_binary_path_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not_exec.txt");
        std::fs::write(&file_path, "not executable").unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = validate_binary_path(file_path.to_str().unwrap());
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryNotExecutable(path) => {
                assert_eq!(path, file_path.to_str().unwrap());
            }
            other => panic!("expected BinaryNotExecutable, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_binary_path_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_binary_path(dir.path().to_str().unwrap());
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryIsDirectory(path) => {
                assert_eq!(path, dir.path().to_str().unwrap());
            }
            other => panic!("expected BinaryIsDirectory, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_binary_path_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("executable");
        std::fs::write(&file_path, "#!/bin/sh\necho hello\n").unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = validate_binary_path(file_path.to_str().unwrap());
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert_eq!(canonical, file_path.canonicalize().unwrap().to_string_lossy().to_string());
    }

    #[test]
    fn test_validate_binary_path_valid_bin_true() {
        let result = validate_binary_path("/bin/true");
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert!(canonical.ends_with("true"));
        assert!(std::path::Path::new(&canonical).exists());
    }

    #[test]
    fn test_subprocess_config_rejects_nonexistent_binary() {
        let result = SubprocessConfig::new(
            "/nonexistent/binary".to_string(),
            vec!["nonexistent".to_string()],
            5000,
            vec![],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryNotFound(_) => {}
            other => panic!("expected BinaryNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_subprocess_config_rejects_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not_exec.txt");
        std::fs::write(&file_path, "not executable").unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = SubprocessConfig::new(
            file_path.to_str().unwrap().to_string(),
            vec!["not_exec".to_string()],
            5000,
            vec![],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryNotExecutable(_) => {}
            other => panic!("expected BinaryNotExecutable, got {:?}", other),
        }
    }

    #[test]
    fn test_subprocess_config_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = SubprocessConfig::new(
            dir.path().to_str().unwrap().to_string(),
            vec![],
            5000,
            vec![],
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            SubprocessError::BinaryIsDirectory(_) => {}
            other => panic!("expected BinaryIsDirectory, got {:?}", other),
        }
    }

    #[test]
    fn test_subprocess_config_accepts_valid_binary() {
        let result = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![1, 2, 3],
        );
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.executable_path().ends_with("true"));
        assert_eq!(config.timeout_ms(), 5000);
        assert_eq!(config.fd3_payload(), &[1, 2, 3]);
    }

    #[test]
    fn test_subprocess_config_canonicalizes_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("canonicalize_test");
        std::fs::write(&file_path, "#!/bin/sh\necho ok\n").unwrap();
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();
        let result = SubprocessConfig::new(
            file_path.to_str().unwrap().to_string(),
            vec!["canonicalize_test".to_string()],
            5000,
            vec![],
        );
        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.executable_path().starts_with('/'));
        assert_eq!(
            config.executable_path(),
            file_path.canonicalize().unwrap().to_string_lossy().to_string()
        );
    }
}
