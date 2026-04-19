use std::path::PathBuf;
use std::time::Instant;

use super::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReloadEvent {
    Reloaded {
        path: PathBuf,
        duration_ms: u64,
    },
    ReloadFailed {
        path: PathBuf,
        error: Error,
        duration_ms: u64,
    },
}

impl ReloadEvent {
    pub fn reload_succeeded(path: PathBuf, start: Instant) -> Self {
        let duration_ms = start.elapsed().as_millis() as u64;
        Self::Reloaded {
            path,
            duration_ms,
        }
    }

    pub fn reload_failed(path: PathBuf, error: Error, start: Instant) -> Self {
        let duration_ms = start.elapsed().as_millis() as u64;
        Self::ReloadFailed {
            path,
            error,
            duration_ms,
        }
    }
}