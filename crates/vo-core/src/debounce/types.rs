use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Modify(PathBuf),
    Delete(PathBuf),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum Error {
    #[error("Invalid debounce duration configured: duration cannot be zero")]
    InvalidDebounceDuration,
    #[error("Watcher channel closed unexpectedly")]
    WatcherChannelClosed,
    #[error("Debouncer encountered an internal error")]
    DebouncerInternal,
    #[error("No tokio runtime available; debouncer requires an active async runtime")]
    NoRuntime,
}