//! Async traits: SpawnStorage, ProcessManager, WorkQueue

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::types::{SpawnPhase, SpawnRecord, SpawnSupervisorError};
use vo_types::InstanceId;

// =============================================================================
// Async Traits - Storage and Process abstractions
// =============================================================================

/// Async storage trait for spawn operations
#[async_trait]
pub trait SpawnStorage: Send + Sync {
    /// Gets a spawn record by instance ID.
    async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord>;

    /// Saves a spawn record.
    async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), SpawnSupervisorError>;

    /// Deletes a spawn record.
    async fn delete_spawn_record(
        &self,
        instance_id: &InstanceId,
    ) -> Result<(), SpawnSupervisorError>;

    /// Scans for spawns in the given phase.
    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, max: u32) -> Vec<SpawnRecord>;

    /// Updates spawn phase for a record.
    async fn transition_phase(
        &self,
        instance_id: &InstanceId,
        new_phase: SpawnPhase,
    ) -> Result<(), SpawnSupervisorError>;
}

/// Process handle for managing a spawned process
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    /// Process ID.
    pub pid: u32,
    /// Executable path.
    pub executable: PathBuf,
    /// Arguments passed to the executable.
    pub args: Vec<String>,
}

impl ProcessHandle {
    /// Creates a new `ProcessHandle`.
    #[must_use]
    pub fn new(pid: u32, executable: PathBuf, args: Vec<String>) -> Self {
        Self {
            pid,
            executable,
            args,
        }
    }
}

/// Async process trait for spawning and managing subprocesses
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Spawns a new process.
    async fn spawn_process(
        &self,
        executable: &Path,
        args: &[String],
    ) -> Result<ProcessHandle, SpawnSupervisorError>;

    /// Checks if a process is healthy.
    async fn check_health(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    /// Checks if a process is a zombie.
    async fn is_zombie(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    /// Terminates a process gracefully.
    async fn terminate(&self, pid: u32) -> Result<(), SpawnSupervisorError>;

    /// Waits for a process to exit.
    async fn wait(&self, pid: u32) -> Result<i32, SpawnSupervisorError>;
}

/// Async work queue trait for dispatching work
#[async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a spawn work item for the given instance.
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        executable: PathBuf,
        args: Vec<String>,
    ) -> Result<(), SpawnSupervisorError>;

    /// Enqueues a resume work item for the given instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError>;
}
