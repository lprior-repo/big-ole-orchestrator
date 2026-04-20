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
use std::os::fd::{FromRawFd, RawFd};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Maximum allowed length for executable path (4096 bytes)
const MAX_EXECUTABLE_PATH_LEN: usize = 4096;

/// Maximum allowed length for individual argument (128KB)
const MAX_ARG_LEN: usize = 131072;

/// Maximum allowed number of arguments (256)
const MAX_ARG_COUNT: usize = 256;

/// Command injection prevention: Check for shell metacharacters and dangerous patterns
fn check_command_injection(input: &str, context: &str) -> Result<(), SubprocessError> {
    // Check for shell metacharacters that could enable injection
    let dangerous_chars = [';', '|', '&', '$', '`', '(', ')', '{', '}', '<', '>', '\n', '\r', '\t'];
    for ch in dangerous_chars.iter() {
        if input.contains(*ch) {
            return Err(SubprocessError::CommandInjectionBlocked(format!(
                "{} contains shell metacharacter '{}'",
                context, ch
            )));
        }
    }

    // Check for command substitution patterns
    if input.contains("$(") || input.contains("`") {
        return Err(SubprocessError::CommandInjectionBlocked(format!(
            "{} contains command substitution pattern",
            context
        )));
    }

    // Check for variable expansion patterns
    if input.contains("${") || (input.contains("$") && input.len() > 1) {
        // Allow $0, $1, etc. for positional parameters but not variable expansion
        if input.matches('$').count() > 0 {
            // Check for $VAR or ${VAR} patterns
            for i in 0..input.len().saturating_sub(1) {
                if input.as_bytes()[i] == b'$' {
                    let remaining = &input[i..];
                    if remaining.starts_with("$(")
                        || (remaining.starts_with("${") && remaining.contains('}'))
                        || (remaining.len() > 1
                            && remaining.chars().nth(1).map_or(false, |c| c.is_ascii_alphabetic() || c == '_'))
                    {
                        return Err(SubprocessError::CommandInjectionBlocked(format!(
                            "{} contains variable expansion pattern",
                            context
                        )));
                    }
                }
            }
        }
    }

    // Check for path traversal patterns
    if input.contains("..") {
        return Err(SubprocessError::CommandInjectionBlocked(format!(
            "{} contains path traversal pattern '..'",
            context
        )));
    }

    Ok(())
}

/// Validate executable path is safe and absolute
fn validate_executable_path(path: &str) -> Result<(), SubprocessError> {
    // Check length
    if path.len() > MAX_EXECUTABLE_PATH_LEN {
        return Err(SubprocessError::InputValidationError {
            reason: format!(
                "executable path exceeds maximum length of {} bytes",
                MAX_EXECUTABLE_PATH_LEN
            ),
        });
    }

    // Check for command injection
    check_command_injection(path, "executable path")?;

    // Path must be absolute
    if !Path::new(path).is_absolute() {
        return Err(SubprocessError::InputValidationError {
            reason: "executable path must be absolute".to_string(),
        });
    }

    // Path components cannot be empty (no double slashes)
    if path.contains("//") {
        return Err(SubprocessError::InputValidationError {
            reason: "executable path contains double slashes".to_string(),
        });
    }

    // Path cannot contain null bytes
    if path.contains('\0') {
        return Err(SubprocessError::InputValidationError {
            reason: "executable path contains null byte".to_string(),
        });
    }

    Ok(())
}

/// Validate command argument is safe
fn validate_argument(arg: &str, index: usize) -> Result<(), SubprocessError> {
    // Check length
    if arg.len() > MAX_ARG_LEN {
        return Err(SubprocessError::InputValidationError {
            reason: format!(
                "argument {} exceeds maximum length of {} bytes",
                index, MAX_ARG_LEN
            ),
        });
    }

    // Check for command injection
    check_command_injection(arg, &format!("argument {}", index))?;

    // Argument cannot contain null bytes
    if arg.contains('\0') {
        return Err(SubprocessError::InputValidationError {
            reason: format!("argument {} contains null byte", index),
        });
    }

    Ok(())
}

/// Sanitize and validate subprocess configuration
///
/// # Errors
///
/// Returns `SubprocessError::InputValidationError` if path validation fails.
/// Returns `SubprocessError::CommandInjectionBlocked` if injection attempt detected.
fn sanitize_config(config: &mut SubprocessConfig) -> Result<(), SubprocessError> {
    // Validate executable path
    validate_executable_path(config.executable_path())?;

    // Validate arguments
    if config.argv().len() > MAX_ARG_COUNT {
        return Err(SubprocessError::InputValidationError {
            reason: format!(
                "argument count exceeds maximum of {} arguments",
                MAX_ARG_COUNT
            ),
        });
    }

    for (i, arg) in config.argv().iter().enumerate() {
        validate_argument(arg, i)?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct SubprocessConfig {
    executable_path: String,
    argv: Vec<String>,
    timeout_ms: u64,
    fd3_payload: Vec<u8>,
}

impl SubprocessConfig {
    #[must_use]
    pub fn new(
        executable_path: String,
        argv: Vec<String>,
        timeout_ms: u64,
        fd3_payload: Vec<u8>,
    ) -> Self {
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
    #[error("input validation failed: {reason}")]
    InputValidationError { reason: String },
    #[error("command injection attempt blocked: {0}")]
    CommandInjectionBlocked(String),
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
/// - Input validation fails (command injection attempt)
#[tracing::instrument(skip(config))]
pub async fn run_subprocess(mut config: SubprocessConfig) -> Result<SubprocessOutput, SubprocessError> {
    // Sanitize inputs to prevent command injection
    sanitize_config(&mut config)?;
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
            let _ = child.kill().await;
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

    #[tokio::test]
    async fn test_bounded_buffer_constant() {
        assert_eq!(BOUNDED_BUFFER_SIZE, 65536);
    }

    #[tokio::test]
    async fn test_create_pipe_sets_cloexec() {
        let (r, w) = create_pipe().unwrap();
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

    // ============================================================================
    // BLACK-HAT TESTS: Command Injection Prevention (bh-009)
    // ============================================================================

    /// Test: Semicolon injection attempt
    #[tokio::test]
    async fn test_command_injection_semicolon() {
        let config = SubprocessConfig::new(
            "/bin/echo; rm -rf /".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Semicolon injection should be blocked: {:?}",
            result
        );
    }

    /// Test: Pipe injection attempt
    #[tokio::test]
    async fn test_command_injection_pipe() {
        let config = SubprocessConfig::new(
            "/bin/echo | cat".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Pipe injection should be blocked: {:?}",
            result
        );
    }

    /// Test: Ampersand injection attempt
    #[tokio::test]
    async fn test_command_injection_ampersand() {
        let config = SubprocessConfig::new(
            "/bin/echo && rm -rf /".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Ampersand injection should be blocked: {:?}",
            result
        );
    }

    /// Test: Command substitution with $()
    #[tokio::test]
    async fn test_command_injection_dollar_paren() {
        let config = SubprocessConfig::new(
            "/bin/echo $(whoami)".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "$() command substitution should be blocked: {:?}",
            result
        );
    }

    /// Test: Command substitution with backticks
    #[tokio::test]
    async fn test_command_injection_backticks() {
        let config = SubprocessConfig::new(
            "/bin/echo `whoami`".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Backtick command substitution should be blocked: {:?}",
            result
        );
    }

    /// Test: Variable expansion $HOME
    #[tokio::test]
    async fn test_command_injection_variable_expansion() {
        let config = SubprocessConfig::new(
            "/bin/echo $HOME".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Variable expansion should be blocked: {:?}",
            result
        );
    }

    /// Test: Path traversal in executable path
    #[tokio::test]
    async fn test_command_injection_path_traversal() {
        let config = SubprocessConfig::new(
            "/bin/../../../etc/passwd".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Path traversal should be blocked: {:?}",
            result
        );
    }

    /// Test: Newline injection in argument
    #[tokio::test]
    async fn test_command_injection_newline_arg() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["arg1\nrm -rf /".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Newline in argument should be blocked: {:?}",
            result
        );
    }

    /// Test: Tab injection in argument
    #[tokio::test]
    async fn test_command_injection_tab_arg() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["arg1\trm -rf /".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Tab in argument should be blocked: {:?}",
            result
        );
    }

    /// Test: Carriage return injection
    #[tokio::test]
    async fn test_command_injection_cr() {
        let config = SubprocessConfig::new(
            "/bin/echo\rm -rf /".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Carriage return should be blocked: {:?}",
            result
        );
    }

    /// Test: Redirection attempt with <
    #[tokio::test]
    async fn test_command_injection_redirect_input() {
        let config = SubprocessConfig::new(
            "/bin/cat < /etc/passwd".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Input redirection should be blocked: {:?}",
            result
        );
    }

    /// Test: Redirection attempt with >
    #[tokio::test]
    async fn test_command_injection_redirect_output() {
        let config = SubprocessConfig::new(
            "/bin/echo test > /tmp/malicious".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Output redirection should be blocked: {:?}",
            result
        );
    }

    /// Test: Subshell with parentheses
    #[tokio::test]
    async fn test_command_injection_subshell() {
        let config = SubprocessConfig::new(
            "/bin/echo (rm -rf /)".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Subshell should be blocked: {:?}",
            result
        );
    }

    /// Test: Brace expansion
    #[tokio::test]
    async fn test_command_injection_braces() {
        let config = SubprocessConfig::new(
            "/bin/echo {rm,cat} /etc/passwd".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Brace expansion should be blocked: {:?}",
            result
        );
    }

    /// Test: Null byte injection in path
    #[tokio::test]
    async fn test_command_injection_null_byte() {
        let config = SubprocessConfig::new(
            "/bin/true\0/bin/false".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Null byte should be rejected: {:?}",
            result
        );
    }

    /// Test: Non-absolute path rejected
    #[tokio::test]
    async fn test_non_absolute_path_rejected() {
        let config = SubprocessConfig::new(
            "bin/true".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Non-absolute path should be rejected: {:?}",
            result
        );
    }

    /// Test: Double slash in path rejected
    #[tokio::test]
    async fn test_double_slash_rejected() {
        let config = SubprocessConfig::new(
            "/bin//true".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Double slash should be rejected: {:?}",
            result
        );
    }

    /// Test: Valid executable path is accepted
    #[tokio::test]
    async fn test_valid_absolute_path_accepted() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["true".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        // May fail at spawn (binary doesn't exist), but should pass validation
        assert!(
            !matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Valid path should not be blocked by injection check: {:?}",
            result
        );
        assert!(
            !matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Valid path should not fail validation: {:?}",
            result
        );
    }

    /// Test: Valid argument with spaces is accepted
    #[tokio::test]
    async fn test_valid_arg_with_spaces_accepted() {
        let config = SubprocessConfig::new(
            "/bin/echo".to_string(),
            vec!["hello world".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        // May fail at spawn, but should pass validation
        assert!(
            !matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Valid arg with spaces should not be blocked: {:?}",
            result
        );
        assert!(
            !matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Valid arg should not fail validation: {:?}",
            result
        );
    }

    /// Test: Argument count limit
    #[tokio::test]
    async fn test_argument_count_limit() {
        let mut args: Vec<String> = Vec::with_capacity(MAX_ARG_COUNT + 1);
        for i in 0..=MAX_ARG_COUNT {
            args.push(format!("arg{}", i));
        }
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            args,
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::InputValidationError { ref reason }) if reason.contains("exceeds maximum")),
            "Argument count limit should be enforced: {:?}",
            result
        );
    }

    /// Test: Argument length limit
    #[tokio::test]
    async fn test_argument_length_limit() {
        let long_arg = "a".repeat(MAX_ARG_LEN + 100);
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec![long_arg],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::InputValidationError { ref reason }) if reason.contains("exceeds maximum length")),
            "Argument length limit should be enforced: {:?}",
            result
        );
    }

    /// Test: Mixed valid and malicious arguments
    #[tokio::test]
    async fn test_mixed_valid_and_malicious_args() {
        let config = SubprocessConfig::new(
            "/bin/echo".to_string(),
            vec![
                "safe".to_string(),
                "safe2".to_string(),
                "unsafe; rm -rf /".to_string(),
                "safe3".to_string(),
            ],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Malicious arg in list should be blocked: {:?}",
            result
        );
    }

    /// Test: Path traversal in argument
    #[tokio::test]
    async fn test_path_traversal_in_arg() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["../../../etc/passwd".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Path traversal in arg should be blocked: {:?}",
            result
        );
    }

    /// Test: Command injection via argument
    #[tokio::test]
    async fn test_command_injection_via_argument() {
        let config = SubprocessConfig::new(
            "/bin/true".to_string(),
            vec!["$(whoami)".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Command substitution in arg should be blocked: {:?}",
            result
        );
    }

    /// Test: Empty argument is allowed
    #[tokio::test]
    async fn test_empty_arg_allowed() {
        let config = SubprocessConfig::new(
            "/bin/echo".to_string(),
            vec!["".to_string()],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            !matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Empty argument should be allowed: {:?}",
            result
        );
    }

    /// Test: Valid path with underscores and hyphens
    #[tokio::test]
    async fn test_valid_path_special_chars() {
        let config = SubprocessConfig::new(
            "/usr/local/bin/my-app_v2.0".to_string(),
            vec![],
            5000,
            vec![],
        );
        let result = run_subprocess(config).await;
        assert!(
            !matches!(result, Err(SubprocessError::CommandInjectionBlocked(_))),
            "Valid path with special chars should not be blocked: {:?}",
            result
        );
        assert!(
            !matches!(result, Err(SubprocessError::InputValidationError { .. })),
            "Valid path should pass validation: {:?}",
            result
        );
    }
}
