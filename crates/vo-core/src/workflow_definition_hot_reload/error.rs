use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("workflow binary not found: {0}")]
    BinaryNotFound(PathBuf),

    #[error("failed to spawn workflow binary '{binary}': {source}")]
    SpawnFailed {
        binary: String,
        source: std::io::Error,
    },

    #[error("workflow binary '{binary}' exited with code {code}: {stderr}")]
    BinaryFailed {
        binary: String,
        code: i32,
        stderr: String,
    },

    #[error("workflow binary '{binary}' timed out after {timeout_secs}s")]
    BinaryTimeout {
        binary: String,
        timeout_secs: u64,
    },

    #[error("--graph produced no output for '{binary}'")]
    NoGraphOutput { binary: String },

    #[error("failed to parse --graph output from '{binary}': {source}")]
    ParseFailed {
        binary: String,
        source: serde_json::Error,
    },

    #[error("workflow definition validation failed for '{workflow}': {reason}")]
    ValidationFailed {
        workflow: String,
        reason: String,
    },

    #[error("workflow '{workflow}' not found in registry")]
    WorkflowNotFound { workflow: String },

    #[error("watcher error: {0}")]
    WatcherError(String),

    #[error("debounce error: {0}")]
    DebounceError(String),

    #[error("workflow definitions directory not found: {0}")]
    WorkflowsDirNotFound(PathBuf),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<notify::Error> for Error {
    fn from(e: notify::Error) -> Self {
        Error::WatcherError(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::ParseFailed {
            binary: "unknown".to_string(),
            source: e,
        }
    }
}