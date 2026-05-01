use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    #[error("timeout must be greater than 0ms, got {timeout_ms}")]
    TimeoutMustBePositive { timeout_ms: u64 },
    #[error("program path does not exist: {path:?}")]
    ProgramMissing { path: PathBuf },
    #[error("program path is not executable: {path:?}")]
    ProgramNotExecutable { path: PathBuf },
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("failed to create subprocess pipes: {detail}")]
    PipeSetupFailed { detail: String },
    #[error("failed to spawn subprocess: {detail}")]
    SpawnFailed { detail: String },
    #[error("failed to wait for subprocess: {detail}")]
    WaitFailed { detail: String },
    #[error("failed to read fd4 payload: {detail}")]
    Fd4ReadFailed { detail: String },
    #[error("failed to write fd3 payload: {detail}")]
    Fd3WriteFailed { detail: String },
    #[error("failed to capture stderr: {detail}")]
    StderrReadFailed { detail: String },
    #[error("failed to capture stdout: {detail}")]
    StdoutReadFailed { detail: String },
    #[error("failed to signal subprocess: {detail}")]
    SignalFailed { detail: String },
    #[error("subprocess timed out after {elapsed_ms}ms")]
    Timeout {
        elapsed_ms: u64,
        stdout_bytes: Vec<u8>,
        stdout_truncated: bool,
        stderr_bytes: Vec<u8>,
        stderr_truncated: bool,
    },
    #[error("subprocess exited with code {exit_code}")]
    ProcessFailed {
        exit_code: i32,
        stdout_bytes: Vec<u8>,
        stdout_truncated: bool,
        stderr_bytes: Vec<u8>,
        stderr_truncated: bool,
    },
    #[error("backpressure timeout: child process fell behind by {wait_seconds}s")]
    BackpressureTimeout { wait_seconds: u64 },
    #[error("Payload too large: {0} bytes")]
    PayloadTooLarge(u32),
    #[error("input exceeds cap: {size} bytes (max {cap})")]
    InputExceedsCap { size: u32, cap: u32 },
    #[error("Incomplete read: expected {expected} bytes, got {actual}")]
    IncompleteRead { expected: usize, actual: usize },
    #[error("Invalid JSON or UTF-8: {0}")]
    InvalidJson(String),
    #[error("Invalid postcard encoding: {0}")]
    InvalidPostcard(String),
    #[error("Version mismatch: expected 1, got {0}")]
    VersionMismatch(u8),
    #[error("Schema violation: {0}")]
    SchemaViolation(String),
    #[error("Identity mismatch: expected {expected_instance}:{expected_node}, got {actual_instance}:{actual_node}")]
    IdentityMismatch {
        expected_instance: String,
        expected_node: String,
        actual_instance: String,
        actual_node: String,
    },
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("IPC reader already consumed")]
    AlreadyConsumed,
    #[error("handshake timed out waiting for child response on fd4")]
    HandshakeTimeout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn config_error_timeout_display() {
        let err = ConfigError::TimeoutMustBePositive { timeout_ms: 42 };
        assert_eq!(err.to_string(), "timeout must be greater than 0ms, got 42");
    }

    #[test]
    fn config_error_missing_display() {
        let err = ConfigError::ProgramMissing {
            path: PathBuf::from("/bin/foo"),
        };
        assert!(err.to_string().contains("/bin/foo"));
    }

    #[test]
    fn config_error_not_executable_display() {
        let err = ConfigError::ProgramNotExecutable {
            path: PathBuf::from("/tmp/bar"),
        };
        assert!(err.to_string().contains("/tmp/bar"));
    }

    #[test]
    fn config_error_clone_eq() {
        let err1 = ConfigError::TimeoutMustBePositive { timeout_ms: 1 };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn config_error_debug() {
        let err = ConfigError::TimeoutMustBePositive { timeout_ms: 5 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("TimeoutMustBePositive"));
    }

    #[test]
    fn ipc_error_from_config_error() {
        let cfg_err = ConfigError::TimeoutMustBePositive { timeout_ms: 0 };
        let ipc_err: IpcError = cfg_err.into();
        assert!(matches!(ipc_err, IpcError::Config(_)));
    }

    #[test]
    fn ipc_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let ipc_err: IpcError = io_err.into();
        assert!(matches!(ipc_err, IpcError::IoError(_)));
        assert!(ipc_err.to_string().contains("pipe broke"));
    }

    #[test]
    fn ipc_error_payload_too_large() {
        let err = IpcError::PayloadTooLarge(999);
        assert_eq!(err.to_string(), "Payload too large: 999 bytes");
    }

    #[test]
    fn ipc_error_incomplete_read() {
        let err = IpcError::IncompleteRead {
            expected: 100,
            actual: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }

    #[test]
    fn ipc_error_invalid_json() {
        let err = IpcError::InvalidJson("bad json".into());
        assert!(err.to_string().contains("bad json"));
    }

    #[test]
    fn ipc_error_version_mismatch() {
        let err = IpcError::VersionMismatch(3);
        assert!(err.to_string().contains("3"));
    }

    #[test]
    fn ipc_error_schema_violation() {
        let err = IpcError::SchemaViolation("bad field".into());
        assert!(err.to_string().contains("bad field"));
    }

    #[test]
    fn ipc_error_identity_mismatch() {
        let err = IpcError::IdentityMismatch {
            expected_instance: "i1".into(),
            expected_node: "n1".into(),
            actual_instance: "i2".into(),
            actual_node: "n2".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("i1") && msg.contains("n1"));
        assert!(msg.contains("i2") && msg.contains("n2"));
    }

    #[test]
    fn ipc_error_timeout_fields() {
        let err = IpcError::Timeout {
            elapsed_ms: 5000,
            stderr_bytes: b"oops".to_vec(),
            stderr_truncated: true,
        };
        assert!(err.to_string().contains("5000ms"));
    }

    #[test]
    fn ipc_error_process_failed_fields() {
        let err = IpcError::ProcessFailed {
            exit_code: 1,
            stderr_bytes: vec![],
            stderr_truncated: false,
        };
        assert!(err.to_string().contains("exited with code 1"));
    }

    #[test]
    fn ipc_error_debug() {
        let err = IpcError::PipeSetupFailed {
            detail: "test".into(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("PipeSetupFailed"));
    }

    #[test]
    fn ipc_error_fd3_write_failed_display() {
        let err = IpcError::Fd3WriteFailed {
            detail: "broken pipe".to_string(),
        };
        assert_eq!(err.to_string(), "failed to write fd3 payload: broken pipe");
    }

    #[test]
    fn ipc_error_fd4_read_failed_display() {
        let err = IpcError::Fd4ReadFailed {
            detail: "eof".to_string(),
        };
        assert_eq!(err.to_string(), "failed to read fd4 payload: eof");
    }

    #[test]
    fn ipc_error_spawn_failed_display() {
        let err = IpcError::SpawnFailed {
            detail: "enoent".to_string(),
        };
        assert_eq!(err.to_string(), "failed to spawn subprocess: enoent");
    }

    #[test]
    fn ipc_error_signal_failed_display() {
        let err = IpcError::SignalFailed {
            detail: "eperm".to_string(),
        };
        assert_eq!(err.to_string(), "failed to signal subprocess: eperm");
    }

    #[test]
    fn ipc_error_wait_failed_display() {
        let err = IpcError::WaitFailed {
            detail: "echild".to_string(),
        };
        assert_eq!(err.to_string(), "failed to wait for subprocess: echild");
    }

    #[test]
    fn ipc_error_stderr_read_failed_display() {
        let err = IpcError::StderrReadFailed {
            detail: "read error".to_string(),
        };
        assert_eq!(err.to_string(), "failed to capture stderr: read error");
    }

    #[test]
    fn ipc_error_config_from_display() {
        let cfg = ConfigError::ProgramMissing {
            path: PathBuf::from("/nope"),
        };
        let ipc: IpcError = cfg.into();
        assert!(ipc.to_string().contains("/nope"));
    }

    #[test]
    fn ipc_error_timeout_debug() {
        let err = IpcError::Timeout {
            elapsed_ms: 999,
            stderr_bytes: b"err".to_vec(),
            stderr_truncated: false,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Timeout"));
    }

    #[test]
    fn ipc_error_process_failed_debug() {
        let err = IpcError::ProcessFailed {
            exit_code: 1,
            stderr_bytes: vec![],
            stderr_truncated: false,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("ProcessFailed"));
    }
}
