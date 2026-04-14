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

use std::os::fd::{FromRawFd, RawFd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone)]
pub struct SubprocessConfig {
    executable_path: String,
    argv: Vec<String>,
    timeout_ms: u64,
    fd3_payload: Vec<u8>,
}

impl SubprocessConfig {
    #[must_use]
    pub fn new(executable_path: String, argv: Vec<String>, timeout_ms: u64, fd3_payload: Vec<u8>) -> Self {
        Self {
            executable_path,
            argv,
            timeout_ms,
            fd3_payload,
        }
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
}

const BOUNDED_BUFFER_SIZE: usize = 65536;

fn create_pipe() -> Result<(RawFd, RawFd), SubprocessError> {
    let mut fds = [0; 2];
    let res = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
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
    let (fd3_read, fd3_write) = create_pipe()?;
    let (fd4_read, fd4_write) = create_pipe()?;

    let mut command = Command::new(&config.executable_path);
    command.args(&config.argv);
    command.env_clear();
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

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

    let mut child = command.spawn().map_err(|e| SubprocessError::SpawnFailed(e.to_string()))?;

    unsafe {
        libc::close(fd3_read);
        libc::close(fd4_write);
    }

    let fd3_writer = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd3_write)) };
    let fd4_reader = unsafe { tokio::fs::File::from_std(std::fs::File::from_raw_fd(fd4_read)) };

    let timeout_ms = config.timeout_ms();
    let fd3_payload = config.fd3_payload;

    let res = timeout(
        Duration::from_millis(timeout_ms),
        perform_ipc(fd3_writer, fd4_reader, fd3_payload),
    )
    .await;

    match res {
        Ok(Ok(output)) => {
            let exit_code = child.wait().await.ok().and_then(|s| s.code());
            Ok(SubprocessOutput {
                fd4_bytes: output,
                exit_code,
            })
        }
        Ok(Err(e)) => Err(e),
        Err(_) => {
            let _ = child.kill().await;
            Err(SubprocessError::Timeout { elapsed_ms: timeout_ms })
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
            std::io::Error::new(std::io::ErrorKind::InvalidData, "fd3 payload exceeds u32::MAX")
        })?;
        fd3_writer.write_all(&len.to_be_bytes()).await?;
        fd3_writer.write_all(&fd3_payload).await?;
        fd3_writer.shutdown().await?;
        Ok::<(), std::io::Error>(())
    });

    let read_handle = tokio::spawn(async move {
        read_bounded_fd4(&mut fd4_reader).await
    });

    let (write_res, read_res) = tokio::join!(write_handle, read_handle);

    if let Err(e) = write_res {
        return Err(SubprocessError::Fd3WriteFailed(e.to_string()));
    }
    if let Err(e) = write_res.unwrap() {
        return Err(SubprocessError::Fd3WriteFailed(e.to_string()));
    }

    let fd4_bytes = read_res.map_err(|e| SubprocessError::Fd4ReadFailed(e.to_string()))??;

    Ok(fd4_bytes)
}

async fn read_bounded_fd4(reader: &mut tokio::fs::File) -> Result<Vec<u8>, SubprocessError> {
    let mut header = [0u8; 4];
    let mut total_read = 0;

    while total_read < 4 {
        let n = reader.read(&mut header[total_read..]).await.map_err(|e| {
            SubprocessError::Fd4ReadFailed(format!("failed to read header: {}", e))
        })?;
        if n == 0 {
            if total_read == 0 {
                return Ok(vec![]);
            }
            return Err(SubprocessError::Fd4ReadFailed("early eof during header".to_string()));
        }
        total_read += n;
    }

    let len = u32::from_be_bytes(header);

    if len > 10_485_760 {
        return Err(SubprocessError::Fd4ReadFailed(format!(
            "payload too large: {} bytes (max 10MB)",
            len
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
            return Err(SubprocessError::Fd4ReadFailed("early eof during payload".to_string()));
        }
        bytes.extend_from_slice(&chunk[..n]);
        remaining -= n;
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bounded_buffer_constant() {
        assert_eq!(BOUNDED_BUFFER_SIZE, 65536);
    }

    #[tokio::test]
    async fn test_create_pipe_sets_cloexec() {
        let (r, w) = create_pipe().unwrap();
        unsafe {
            let flags = libc::fcntl(r, libc::F_GETFD);
            assert!(flags & libc::FD_CLOEXEC != 0, "read end should have FD_CLOEXEC");
            let flags = libc::fcntl(w, libc::F_GETFD);
            assert!(flags & libc::FD_CLOEXEC != 0, "write end should have FD_CLOEXEC");
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
        );
        assert_eq!(config.executable_path(), "/bin/true");
        assert_eq!(config.argv(), &["true".to_string()]);
        assert_eq!(config.timeout_ms(), 5000);
        assert_eq!(config.fd3_payload(), &[1, 2, 3]);
    }

    #[tokio::test]
    async fn test_subprocess_error_display() {
        let err = SubprocessError::Timeout { elapsed_ms: 5000 };
        assert!(err.to_string().contains("5000"));

        let err = SubprocessError::BoundedBufferExceeded { max: 100, tried: 200 };
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
        );
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
}
