//! Async Process Supervisor Port
//!
//! Defines the interface for subprocess lifecycle management.
//! Implementors must be Send + Sync.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use vo_types::InstanceId;

// =============================================================================
// SpawnRecord - Spawn data including lifecycle state fields
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
    /// The command to execute.
    pub command: String,
    /// Current phase of the lifecycle.
    pub spawn_phase: SpawnPhase,
    /// Number of health checks performed.
    pub health_checks: u32,
    /// Number of spawn attempts (for backoff calculation).
    pub spawn_attempts: u32,
    /// Last error encountered.
    pub last_error: Option<ProcessSupervisorError>,
}

impl SpawnRecord {
    /// Creates a new `SpawnRecord` in the Spawn phase.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        command: String,
        spawn_id: Option<vo_types::SpawnId>,
    ) -> Self {
        Self {
            spawn_id,
            instance_id,
            command,
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
            command: self.command.clone(),
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: self.spawn_attempts.saturating_add(1),
            last_error: None,
        }
    }
}

// =============================================================================
// SpawnPhase - Lifecycle phases for spawned subprocesses
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
// ProcessSupervisorError - All error variants for ProcessSupervisor
// =============================================================================

/// `ProcessSupervisorError` - All error variants for `ProcessSupervisor`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessSupervisorError {
    /// Storage operation failed - transient, retryable
    StorageError(String),

    /// Spawn key corrupt or malformed - fatal, requires manual intervention
    CorruptSpawn(String),

    /// Atomicity violation: delete succeeded but dispatch failed
    AtomicityViolation(String),

    /// Instance actor not found - transient if actor is restarting
    InstanceNotFound(InstanceId),

    /// Dispatch failed due to actor mailbox full
    MailboxFull(InstanceId),

    /// Configuration error - fatal
    InvalidConfig(String),

    /// Supervisor already running
    AlreadyRunning,

    /// Supervisor shutdown timeout
    ShutdownTimeout(Duration),

    /// Dispatch error
    DispatchError(String),

    /// Process spawn failed
    SpawnFailed { command: String, error: String },

    /// Health check failed
    HealthCheckFailed {
        instance_id: InstanceId,
        check_number: u32,
        error: String,
    },

    /// Zombie process detected
    ZombieDetected { instance_id: InstanceId, pid: u32 },

    /// Process exited unexpectedly
    ProcessExited {
        instance_id: InstanceId,
        pid: u32,
        exit_code: i32,
    },

    /// Supervisor is not running
    NotRunning,

    /// Already shutdown
    AlreadyShutdown,
}

impl ProcessSupervisorError {
    /// Returns true if this error is transient and retryable.
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

    /// Returns true if this error is fatal and requires manual intervention.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptSpawn(_) | Self::InvalidConfig(_) | Self::ZombieDetected { .. }
        )
    }
}

impl std::fmt::Display for ProcessSupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(s) => write!(f, "Storage error: {s}"),
            Self::CorruptSpawn(s) => write!(f, "Corrupt spawn: {s}"),
            Self::AtomicityViolation(s) => write!(f, "Atomicity violation: {s}"),
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {id}"),
            Self::MailboxFull(id) => write!(f, "Mailbox full: {id}"),
            Self::InvalidConfig(s) => write!(f, "Invalid config: {s}"),
            Self::AlreadyRunning => write!(f, "Already running"),
            Self::ShutdownTimeout(d) => write!(f, "Shutdown timeout: {d:?}"),
            Self::DispatchError(s) => write!(f, "Dispatch error: {s}"),
            Self::SpawnFailed { command, error } => {
                write!(f, "Spawn failed for '{command}': {error}")
            }
            Self::HealthCheckFailed {
                instance_id,
                check_number,
                error,
            } => {
                write!(
                    f,
                    "Health check {check_number} failed for {instance_id}: {error}"
                )
            }
            Self::ZombieDetected { instance_id, pid } => {
                write!(f, "Zombie detected for {instance_id}: pid={pid}")
            }
            Self::ProcessExited {
                instance_id,
                pid,
                exit_code,
            } => {
                write!(
                    f,
                    "Process exited for {instance_id}: pid={pid}, code={exit_code}"
                )
            }
            Self::NotRunning => write!(f, "Supervisor not running"),
            Self::AlreadyShutdown => write!(f, "Supervisor already shutdown"),
        }
    }
}

impl std::error::Error for ProcessSupervisorError {}

// =============================================================================
// ProcessSupervisorMetrics - Metrics for ProcessSupervisor
// =============================================================================

/// Simple counter for metrics using AtomicU64
#[derive(Debug, Default)]
pub struct Counter {
    value: std::sync::atomic::AtomicU64,
}

impl Counter {
    /// Creates a new Counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current value.
    pub fn get(&self) -> u64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Increments the counter.
    pub fn incr(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Metrics for `ProcessSupervisor`
#[derive(Debug, Default)]
pub struct ProcessSupervisorMetrics {
    /// Number of successful spawns.
    pub spawns_successful: Counter,
    /// Number of spawns that failed.
    pub spawns_failed: Counter,
    /// Number of health checks performed.
    pub health_checks_performed: Counter,
    /// Number of health checks that failed.
    pub health_checks_failed: Counter,
    /// Number of zombie processes detected.
    pub zombies_detected: Counter,
    /// Number of respawns.
    pub respawns: Counter,
    /// Number of dispatch errors.
    pub dispatch_errors: Counter,
}

// =============================================================================
// Async Traits - Storage and Process abstractions
// =============================================================================

/// Async storage trait for spawn operations
#[async_trait]
pub trait SpawnStorage: Send + Sync {
    /// Gets a spawn record by instance ID.
    async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord>;

    /// Saves a spawn record.
    async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), ProcessSupervisorError>;

    /// Deletes a spawn record.
    async fn delete_spawn_record(
        &self,
        instance_id: &InstanceId,
    ) -> Result<(), ProcessSupervisorError>;

    /// Scans for spawns in the given phase.
    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, max: u32) -> Vec<SpawnRecord>;

    /// Updates spawn phase for a record.
    async fn transition_phase(
        &self,
        instance_id: &InstanceId,
        new_phase: SpawnPhase,
    ) -> Result<(), ProcessSupervisorError>;
}

/// Async process trait for spawning and managing subprocesses
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Spawns a new process.
    async fn spawn_process(&self, command: &str) -> Result<ProcessHandle, ProcessSupervisorError>;

    /// Checks if a process is healthy.
    async fn check_health(&self, pid: u32) -> Result<bool, ProcessSupervisorError>;

    /// Checks if a process is a zombie.
    async fn is_zombie(&self, pid: u32) -> Result<bool, ProcessSupervisorError>;

    /// Terminates a process gracefully.
    async fn terminate(&self, pid: u32) -> Result<(), ProcessSupervisorError>;

    /// Waits for a process to exit.
    async fn wait(&self, pid: u32) -> Result<i32, ProcessSupervisorError>;
}

/// Process handle for managing a spawned process
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    /// Process ID.
    pub pid: u32,
    /// Command being executed.
    pub command: String,
}

impl ProcessHandle {
    /// Creates a new `ProcessHandle`.
    #[must_use]
    pub fn new(pid: u32, command: String) -> Self {
        Self { pid, command }
    }
}

/// Async work queue trait for dispatching work
#[async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a spawn work item for the given instance.
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        command: String,
    ) -> Result<(), ProcessSupervisorError>;

    /// Enqueues a resume work item for the given instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ProcessSupervisorError>;
}

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// `calculate_backoff_delay` - Calculate exponential backoff delay
///
/// Formula: `initial_backoff * backoff_multiplier^(attempt - 1)`
///
/// This function is a pure calculation with no side effects.
#[inline]
#[must_use]
pub fn calculate_backoff_delay(
    initial_backoff_ms: u64,
    backoff_multiplier: f64,
    attempt: u32,
) -> u64 {
    let exponent = attempt.saturating_sub(1) as f64;
    let multiplier_pow = backoff_multiplier.powf(exponent);
    #[allow(clippy::cast_possible_truncation)]
    let result = (initial_backoff_ms as f64 * multiplier_pow) as u64;
    result
}

/// `is_zombie_state` - Check if spawn record indicates zombie state
///
/// Returns true if spawn is in failed phase with high attempt count.
#[inline]
#[must_use]
pub fn is_zombie_state(record: &SpawnRecord) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts > 3
}

/// `should_respawn` - Check if spawn should be respawned
///
/// Returns true if spawn is in failed phase and attempts are within limit.
#[inline]
#[must_use]
pub fn should_respawn(record: &SpawnRecord, max_attempts: u32) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts < max_attempts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance_id() -> InstanceId {
        let ulid = ulid::Ulid::new();
        InstanceId::from_bytes(ulid.to_bytes())
    }

    #[test]
    fn calculate_backoff_delay_returns_initial_for_first_attempt() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 1), 1000);
    }

    #[test]
    fn calculate_backoff_delay_applies_multiplier() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 2), 2000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 3), 4000);
    }

    #[test]
    fn calculate_backoff_delay_with_multiplier_1_0() {
        assert_eq!(calculate_backoff_delay(1000, 1.0, 1), 1000);
        assert_eq!(calculate_backoff_delay(1000, 1.0, 2), 1000);
        assert_eq!(calculate_backoff_delay(1000, 1.0, 10), 1000);
    }

    #[test]
    fn is_zombie_state_returns_true_for_failed_high_attempts() {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 5,
            last_error: None,
        };

        assert!(is_zombie_state(&record));
    }

    #[test]
    fn is_zombie_state_returns_false_for_low_attempts() {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 2,
            last_error: None,
        };

        assert!(!is_zombie_state(&record));
    }

    #[test]
    fn should_respawn_returns_true_within_limit() {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 2,
            last_error: None,
        };

        assert!(should_respawn(&record, 5));
    }

    #[test]
    fn should_respawn_returns_false_at_limit() {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 5,
            last_error: None,
        };

        assert!(!should_respawn(&record, 5));
    }

    #[test]
    fn spawn_record_transitions_correctly() {
        let record = SpawnRecord::new(test_instance_id(), "test".to_string(), None);

        assert_eq!(record.spawn_phase, SpawnPhase::Spawn);

        let health_check_record = record.transition_to_health_check();
        assert_eq!(health_check_record.spawn_phase, SpawnPhase::HealthCheck);

        let running_record = health_check_record.transition_to_running();
        assert_eq!(running_record.spawn_phase, SpawnPhase::Running);

        let shutdown_record = running_record.transition_to_shutdown();
        assert_eq!(shutdown_record.spawn_phase, SpawnPhase::Shutdown);
    }

    #[test]
    fn spawn_record_respawn_increments_attempts() {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 3,
            last_error: None,
        };

        let respawned = record.respawn(Some(vo_types::SpawnId::new("new-123".to_string())));
        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(respawned.spawn_attempts, 4);
        assert_eq!(respawned.health_checks, 0);
    }

    #[test]
    fn process_supervisor_error_is_transient() {
        assert!(ProcessSupervisorError::StorageError("test".to_string()).is_transient());
        assert!(ProcessSupervisorError::InstanceNotFound(test_instance_id()).is_transient());
        assert!(!ProcessSupervisorError::InvalidConfig("test".to_string()).is_transient());
    }

    #[test]
    fn process_supervisor_error_is_fatal() {
        assert!(ProcessSupervisorError::CorruptSpawn("test".to_string()).is_fatal());
        assert!(ProcessSupervisorError::InvalidConfig("test".to_string()).is_fatal());
        assert!(!ProcessSupervisorError::StorageError("test".to_string()).is_fatal());
    }
}