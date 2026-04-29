use std::path::{Path, PathBuf};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpawnError {
    #[error("process spawn failed: {0}")]
    ProcessSpawnFailed(String),
    #[error("process not found: {0}")]
    ProcessNotFound(u32),
    #[error("failed to terminate process: {0}")]
    TerminateFailed(String),
}

#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub pid: u32,
    pub executable: PathBuf,
    pub args: Vec<String>,
}

impl ProcessHandle {
    #[must_use]
    pub fn new(pid: u32, executable: PathBuf, args: Vec<String>) -> Self {
        Self {
            pid,
            executable,
            args,
        }
    }
}

#[async_trait]
pub trait SpawnStorage: Send + Sync {
    async fn spawn_process(
        &self,
        executable: &Path,
        args: &[String],
    ) -> Result<ProcessHandle, SpawnError>;

    async fn is_zombie(&self, pid: u32) -> Result<bool, SpawnError>;

    async fn terminate(&self, pid: u32) -> Result<(), SpawnError>;
}