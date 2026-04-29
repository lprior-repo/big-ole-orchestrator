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
}
