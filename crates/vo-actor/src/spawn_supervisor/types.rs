//! Spawn data types: `SpawnRecord`, `SpawnPhase`, `SpawnSupervisorError`, `SpawnSupervisorState`, `CycleResult`

use std::path::PathBuf;
use std::time::Duration;

use vo_types::InstanceId;

// =============================================================================
// `SpawnRecord` - Spawn data including lifecycle state fields
// =============================================================================

/// `SpawnRecord` - Spawn data including lifecycle state fields
///
/// Per the spawn supervisor design:
/// - `spawn_phase`: Current phase of the lifecycle
/// - `health_checks`: Number of health checks performed
/// - `spawn_attempts`: Number of spawn attempts (for backoff calculation)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRecord {
    /// Optional spawn ID for multiple spawns per instance.
    pub spawn_id: Option<vo_types::SpawnId>,
    /// The instance ID this spawn belongs to.
    pub instance_id: InstanceId,
    /// The executable path.
    pub executable: PathBuf,
    /// Arguments to pass to the executable.
    pub args: Vec<String>,
    /// Current phase of the lifecycle.
    pub spawn_phase: SpawnPhase,
    /// Number of health checks performed.
    pub health_checks: u32,
    /// Number of spawn attempts (for backoff calculation).
    pub spawn_attempts: u32,
    /// Last error encountered.
    pub last_error: Option<SpawnSupervisorError>,
}

impl SpawnRecord {
    /// Creates a new `SpawnRecord` in the Spawn phase.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        executable: PathBuf,
        args: Vec<String>,
        spawn_id: Option<vo_types::SpawnId>,
    ) -> Self {
        Self {
            spawn_id,
            instance_id,
            executable,
            args,
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: None,
        }
    }

    /// Transition to health-check phase.
    #[must_use]
    pub fn transition_to_health_check(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::HealthCheck,
            ..self.clone()
        }
    }

    /// Transition to running phase.
    #[must_use]
    pub fn transition_to_running(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::Running,
            ..self.clone()
        }
    }

    /// Transition to shutdown phase.
    #[must_use]
    pub fn transition_to_shutdown(&self) -> Self {
        Self {
            spawn_phase: SpawnPhase::Shutdown,
            ..self.clone()
        }
    }

    /// Create a new spawn record after respawn.
    #[must_use]
    pub fn respawn(&self, new_spawn_id: Option<vo_types::SpawnId>) -> Self {
        Self {
            spawn_id: new_spawn_id,
            instance_id: self.instance_id.clone(),
            executable: self.executable.clone(),
            args: self.args.clone(),
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: self.spawn_attempts.saturating_add(1),
            last_error: None,
        }
    }
}

// =============================================================================
// `SpawnPhase` - Lifecycle phases for spawned subprocesses
// =============================================================================

/// `SpawnPhase` - Lifecycle phases for spawned subprocesses
///
/// Per the spawn supervisor design:
/// Spawn → HealthCheck → Running → Shutdown
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnPhase {
    /// Initial spawn phase - process is being started
    Spawn,
    /// Health check phase - verifying process is healthy
    HealthCheck,
    /// Running phase - process is healthy and running
    Running,
    /// Shutdown phase - process is being terminated
    Shutdown,
    /// Terminated phase - process has exited
    Terminated,
    /// Failed phase - process failed and will respawn
    Failed,
}

impl std::fmt::Display for SpawnPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn => write!(f, "spawn"),
            Self::HealthCheck => write!(f, "health-check"),
            Self::Running => write!(f, "running"),
            Self::Shutdown => write!(f, "shutdown"),
            Self::Terminated => write!(f, "terminated"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

// =============================================================================
// `SpawnSupervisorError` - All error variants for `SpawnSupervisor`
// =============================================================================

/// `SpawnSupervisorError` - All error variants for `SpawnSupervisor`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpawnSupervisorError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Corrupt spawn: {0}")]
    CorruptSpawn(String),
    #[error("Atomicity violation: {0}")]
    AtomicityViolation(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("Mailbox full: {0}")]
    MailboxFull(InstanceId),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Already running")]
    AlreadyRunning,
    #[error("Shutdown timeout: {0:?}")]
    ShutdownTimeout(Duration),
    #[error("Dispatch error: {0}")]
    DispatchError(String),
    #[error("Spawn failed for '{executable}': {error}")]
    SpawnFailed { executable: PathBuf, error: String },
    #[error("Health check {check_number} failed for {instance_id}: {error}")]
    HealthCheckFailed {
        instance_id: InstanceId,
        check_number: u32,
        error: String,
    },
    #[error("Zombie detected for {instance_id}: pid={pid}")]
    ZombieDetected { instance_id: InstanceId, pid: u32 },
    #[error("Process exited for {instance_id}: pid={pid}, code={exit_code}")]
    ProcessExited {
        instance_id: InstanceId,
        pid: u32,
        exit_code: i32,
    },
    #[error("Supervisor not running")]
    NotRunning,
    #[error("Supervisor already shutdown")]
    AlreadyShutdown,
}

impl SpawnSupervisorError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::StorageError(_)
                | Self::InstanceNotFound(_)
                | Self::MailboxFull(_)
                | Self::DispatchError(_)
        )
    }

    #[must_use]
    pub fn is_resumable(&self) -> bool {
        matches!(
            self,
            Self::HealthCheckFailed { .. } | Self::ProcessExited { .. } | Self::SpawnFailed { .. }
        )
    }

    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptSpawn(_) | Self::InvalidConfig(_) | Self::ZombieDetected { .. }
        )
    }

    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::AlreadyRunning
                | Self::AlreadyShutdown
                | Self::NotRunning
                | Self::ShutdownTimeout(_)
                | Self::AtomicityViolation(_)
        )
    }
}

// =============================================================================
// `SpawnSupervisorState` - Runtime state of the supervisor
// =============================================================================

/// Runtime state of the spawn supervisor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnSupervisorState {
    /// Supervisor is stopped
    Stopped,
    /// Supervisor is running
    Running,
    /// Supervisor is shutting down
    ShuttingDown,
    /// Supervisor has shut down
    ShutDown,
}

// =============================================================================
// `CycleResult` - Result of a process_cycle call
// =============================================================================

/// Result of a `process_cycle` call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleResult {
    /// Number of spawns processed.
    pub spawns_processed: u32,
    /// Number of health checks performed.
    pub health_checks: u32,
    /// Number of errors.
    pub errors: u32,
    /// Number of respawns.
    pub respawns: u32,
}
