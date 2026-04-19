use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum Error {
    #[error("Config file not found: {0}")]
    ConfigFileNotFound(PathBuf),

    #[error("Failed to read config file: {0}")]
    ReadError(PathBuf),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Watcher error: {0}")]
    WatcherError(String),

    #[error("Channel closed unexpectedly")]
    ChannelClosed,

    #[error("Swap failed: no valid config to swap to")]
    SwapFailed,

    #[error("Invalid glob pattern: {0}")]
    InvalidGlobPattern(String),

    #[error("Debounce error: {0}")]
    DebounceError(String),

    #[error("Event queue closed unexpectedly")]
    EventQueueClosed,
}
