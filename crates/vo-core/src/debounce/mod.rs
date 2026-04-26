#![allow(unexpected_cfgs)]

mod r#impl;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{sleep_until, Instant};

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

#[derive(Debug)]
pub struct Debouncer {
    pub duration: Duration,
    ready_rx: Receiver<Result<PathBuf, Error>>,
}

impl PartialEq for Debouncer {
    fn eq(&self, other: &Self) -> bool {
        self.duration == other.duration
    }
}


