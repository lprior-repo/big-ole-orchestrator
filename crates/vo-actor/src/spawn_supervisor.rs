//! Async spawn supervisor for subprocess lifecycle management
//!
//! Provides async spawn supervisor that manages subprocess lifecycle:
//! spawn → health-check → ready → running → shutdown
//!
//! Features:
//! - Zombie detection and reaping
//! - Exponential backoff respawn
//! - Health checks during startup
//! - Follows Data→Calc→Actions pattern
//! - Fully async using tokio and async_trait

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

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
    /// The command to execute.
    pub command: String,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnSupervisorError {
    /// Storage operation failed - transient, retryable
    StorageError(String),

    /// Spawn key corrupt or malformed - fatal, requires manual intervention
    CorruptSpawn(String),

    /// Atomicity violation: delete succeeded but dispatch failed
    /// Spawn may be lost; requires reconciliation
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

impl SpawnSupervisorError {
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
            Self::CorruptSpawn(_)
                | Self::InvalidConfig(_)
                | Self::ZombieDetected { .. }
        )
    }
}

impl std::fmt::Display for SpawnSupervisorError {
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

impl std::error::Error for SpawnSupervisorError {}

// =============================================================================
// SpawnSupervisorMetrics - Metrics for SpawnSupervisor
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

/// Metrics for `SpawnSupervisor`
#[derive(Debug, Default)]
pub struct SpawnSupervisorMetrics {
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
#[async_trait::async_trait]
pub trait SpawnStorage: Send + Sync {
    /// Gets a spawn record by instance ID.
    async fn get_spawn_record(&self, instance_id: &InstanceId) -> Option<SpawnRecord>;

    /// Saves a spawn record.
    async fn save_spawn_record(&self, record: &SpawnRecord) -> Result<(), SpawnSupervisorError>;

    /// Deletes a spawn record.
    async fn delete_spawn_record(&self, instance_id: &InstanceId) -> Result<(), SpawnSupervisorError>;

    /// Scans for spawns in the given phase.
    async fn scan_spawns_by_phase(&self, phase: SpawnPhase, max: u32) -> Vec<SpawnRecord>;

    /// Updates spawn phase for a record.
    async fn transition_phase(
        &self,
        instance_id: &InstanceId,
        new_phase: SpawnPhase,
    ) -> Result<(), SpawnSupervisorError>;
}

/// Async process trait for spawning and managing subprocesses
#[async_trait::async_trait]
pub trait ProcessManager: Send + Sync {
    /// Spawns a new process.
    async fn spawn_process(&self, command: &str) -> Result<ProcessHandle, SpawnSupervisorError>;

    /// Checks if a process is healthy.
    async fn check_health(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    /// Checks if a process is a zombie.
    async fn is_zombie(&self, pid: u32) -> Result<bool, SpawnSupervisorError>;

    /// Terminates a process gracefully.
    async fn terminate(&self, pid: u32) -> Result<(), SpawnSupervisorError>;

    /// Waits for a process to exit.
    async fn wait(&self, pid: u32) -> Result<i32, SpawnSupervisorError>;
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
#[async_trait::async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a spawn work item for the given instance.
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        command: String,
    ) -> Result<(), SpawnSupervisorError>;

    /// Enqueues a resume work item for the given instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SpawnSupervisorError>;
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
// `SpawnSupervisor` - Async actor that manages spawn lifecycle
// =============================================================================

/// `SpawnSupervisor` - Async actor that manages spawn lifecycle
pub struct SpawnSupervisor {
    /// Interval between health checks.
    pub health_check_interval: Duration,
    /// Maximum number of health checks before considering process healthy.
    pub max_health_checks: u32,
    /// Initial backoff duration for respawn.
    pub initial_backoff: Duration,
    /// Backoff multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Maximum respawn attempts.
    pub max_spawn_attempts: u32,
    /// Storage for spawn records.
    pub storage: Arc<dyn SpawnStorage>,
    /// Process manager for spawning processes.
    pub process_manager: Arc<dyn ProcessManager>,
    /// Work queue for dispatching.
    pub work_queue: Arc<dyn WorkQueue>,
    /// Metrics.
    pub metrics: SpawnSupervisorMetrics,
}

impl std::fmt::Debug for SpawnSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnSupervisor")
            .field("health_check_interval", &self.health_check_interval)
            .field("max_health_checks", &self.max_health_checks)
            .field("initial_backoff", &self.initial_backoff)
            .field("backoff_multiplier", &self.backoff_multiplier)
            .field("max_spawn_attempts", &self.max_spawn_attempts)
            .finish_non_exhaustive()
    }
}

impl SpawnSupervisor {
    /// Creates a new `SpawnSupervisor`.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if configuration is invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        health_check_interval: Duration,
        max_health_checks: u32,
        initial_backoff: Duration,
        backoff_multiplier: f64,
        max_spawn_attempts: u32,
        storage: Arc<dyn SpawnStorage>,
        process_manager: Arc<dyn ProcessManager>,
        work_queue: Arc<dyn WorkQueue>,
    ) -> Result<Self, SpawnSupervisorError> {
        if health_check_interval.is_zero() {
            return Err(SpawnSupervisorError::InvalidConfig(
                "health_check_interval must be > 0".to_string(),
            ));
        }

        if max_health_checks == 0 {
            return Err(SpawnSupervisorError::InvalidConfig(
                "max_health_checks must be > 0".to_string(),
            ));
        }

        if initial_backoff.is_zero() {
            return Err(SpawnSupervisorError::InvalidConfig(
                "initial_backoff must be > 0".to_string(),
            ));
        }

        if backoff_multiplier < 1.0 {
            return Err(SpawnSupervisorError::InvalidConfig(
                "backoff_multiplier must be >= 1.0".to_string(),
            ));
        }

        Ok(Self {
            health_check_interval,
            max_health_checks,
            initial_backoff,
            backoff_multiplier,
            max_spawn_attempts,
            storage,
            process_manager,
            work_queue,
            metrics: SpawnSupervisorMetrics::default(),
        })
    }

    /// Spawns the `SpawnSupervisor` background task.
    ///
    /// # Errors
    /// Returns `AlreadyRunning` if the supervisor is already running.
    pub fn spawn(
        self,
    ) -> Result<SpawnSupervisorHandle, SpawnSupervisorError> {
        let (state_sender, _) = watch::channel(SpawnSupervisorState::Stopped);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            let result = self.run_loop(state_sender_clone, shutdown_receiver).await;
            if let Err(e) = result {
                tracing::error!("Spawn supervisor loop exited with error: {}", e);
            }
        });

        Ok(SpawnSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: Some(task_handle),
        })
    }

    /// The main loop implementation.
    #[tracing::instrument(skip_all)]
    async fn run_loop(
        self,
        state_sender: watch::Sender<SpawnSupervisorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) -> Result<(), SpawnSupervisorError> {
        let mut scan_interval = interval(self.health_check_interval);
        scan_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let _ = state_sender.send(SpawnSupervisorState::Running);

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(SpawnSupervisorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    match self.process_cycle().await {
                        Ok(_) => {}
                        Err(e) if e.is_transient() => {
                            tracing::warn!("Transient error in spawn supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            tracing::error!("Fatal error in spawn supervisor cycle: {}", e);
                        }
                        Err(e) => {
                            tracing::error!("Unknown error in spawn supervisor cycle: {}", e);
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(SpawnSupervisorState::ShutDown);
        Ok(())
    }

    /// Processes one spawn cycle.
    ///
    /// Scans storage for spawns in spawn/health-check phases, spawns/health-checks them,
    /// and transitions to running phase when healthy.
    ///
    /// # Errors
    /// Returns an error if storage or process operations fail.
    pub async fn process_cycle(&self) -> Result<CycleResult, SpawnSupervisorError> {
        let mut spawns_processed = 0u32;
        let mut health_checks = 0u32;
        let mut errors = 0u32;
        let mut respawns = 0u32;

        let spawn_records = self.storage.scan_spawns_by_phase(SpawnPhase::Spawn, 100).await;

        for record in spawn_records {
            spawns_processed += 1;

            if record.spawn_attempts > self.max_spawn_attempts {
                self.metrics.spawns_failed.incr();
                errors += 1;
                tracing::warn!(
                    instance_id = %record.instance_id,
                    attempts = record.spawn_attempts,
                    max_attempts = self.max_spawn_attempts,
                    "Max spawn attempts exceeded"
                );
                continue;
            }

            let backoff_delay = self.calculate_backoff_delay(record.spawn_attempts);

            match self.spawn_process(&record).await {
                Ok(process_handle) => {
                    let mut new_record = record.transition_to_health_check();
                    new_record.last_error = None;

                    if let Err(e) = self.storage.save_spawn_record(&new_record).await {
                        self.metrics.dispatch_errors.incr();
                        errors += 1;
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %e,
                            "Failed to save spawn record"
                        );
                        continue;
                    }

                    match self.perform_health_checks(&record.instance_id, &process_handle).await {
                        Ok(()) => {
                            let mut running_record = new_record.transition_to_running();
                            running_record.spawn_id =
                                Some(vo_types::SpawnId::new(process_handle.pid.to_string()));

                            if let Err(e) = self.storage.save_spawn_record(&running_record).await {
                                self.metrics.dispatch_errors.incr();
                                errors += 1;
                                tracing::error!(
                                    instance_id = %record.instance_id,
                                    error = %e,
                                    "Failed to save running spawn record"
                                );
                                continue;
                            }

                            self.metrics.spawns_successful.incr();
                        }
                        Err(e) => {
                            self.metrics.health_checks_failed.incr();
                            errors += 1;
                            tracing::error!(
                                instance_id = %record.instance_id,
                                error = %e,
                                "Health check failed"
                            );

                            if record.spawn_attempts < self.max_spawn_attempts {
                                respawns += 1;
                                self.metrics.respawns.incr();

                                tracing::info!(
                                    instance_id = %record.instance_id,
                                    backoff_ms = backoff_delay.as_millis(),
                                    "Scheduling respawn with backoff"
                                );

                                let _ = backoff_delay;
                            }
                        }
                    }
                }
                Err(e) => {
                    self.metrics.spawns_failed.incr();
                    errors += 1;

                    let mut updated_record = record.clone();
                    updated_record.last_error = Some(e.clone());

                    if let Err(save_err) = self.storage.save_spawn_record(&updated_record).await {
                        self.metrics.dispatch_errors.incr();
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %save_err,
                            "Failed to save error record"
                        );
                    }
                }
            }
        }

        let health_check_records = self
            .storage
            .scan_spawns_by_phase(SpawnPhase::HealthCheck, 100)
            .await;

        for record in health_check_records {
            spawns_processed += 1;
            health_checks += 1;

            match self.perform_health_checks(
                &record.instance_id,
                &ProcessHandle {
                    pid: 0,
                    command: record.command.clone(),
                },
            ).await {
                Ok(()) => {
                    let running_record = record.transition_to_running();

                    if let Err(e) = self.storage.save_spawn_record(&running_record).await {
                        self.metrics.dispatch_errors.incr();
                        errors += 1;
                        tracing::error!(
                            instance_id = %record.instance_id,
                            error = %e,
                            "Failed to save running spawn record"
                        );
                        continue;
                    }

                    self.metrics.spawns_successful.incr();
                }
                Err(e) => {
                    self.metrics.health_checks_failed.incr();
                    errors += 1;
                    tracing::error!(
                        instance_id = %record.instance_id,
                        error = %e,
                        "Health check failed"
                    );
                }
            }
        }

        Ok(CycleResult {
            spawns_processed,
            health_checks,
            errors,
            respawns,
        })
    }

    /// Spawns a process for a spawn record.
    async fn spawn_process(&self, record: &SpawnRecord) -> Result<ProcessHandle, SpawnSupervisorError> {
        self.process_manager.spawn_process(&record.command).await
    }

    /// Performs health checks on a process.
    async fn perform_health_checks(
        &self,
        instance_id: &InstanceId,
        process: &ProcessHandle,
    ) -> Result<(), SpawnSupervisorError> {
        for i in 1..=self.max_health_checks {
            self.metrics.health_checks_performed.incr();

            tokio::time::sleep(Duration::from_millis(100)).await;

            match self.process_manager.check_health(process.pid).await {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    if i < self.max_health_checks {
                        continue;
                    }
                }
                Err(e) => {
                    return Err(SpawnSupervisorError::HealthCheckFailed {
                        instance_id: instance_id.clone(),
                        check_number: i,
                        error: e.to_string(),
                    });
                }
            }
        }

        Err(SpawnSupervisorError::HealthCheckFailed {
            instance_id: instance_id.clone(),
            check_number: self.max_health_checks,
            error: "Max health checks exceeded".to_string(),
        })
    }

    /// Calculates backoff delay using exponential backoff.
    ///
    /// Formula: `initial_backoff * backoff_multiplier^(attempt - 1)`
    #[must_use]
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1) as f64;
        let multiplier_pow = self.backoff_multiplier.powf(exponent);
        #[allow(clippy::cast_possible_truncation)]
        let delay_ms = (self.initial_backoff.as_millis() as f64 * multiplier_pow) as u64;
        Duration::from_millis(delay_ms)
    }
}

// =============================================================================
// `SpawnSupervisorHandle` - Handle for controlling `SpawnSupervisor`
// =============================================================================

/// Handle for controlling `SpawnSupervisor`
#[derive(Debug)]
pub struct SpawnSupervisorHandle {
    pub(crate) state_sender: watch::Sender<SpawnSupervisorState>,
    pub(crate) shutdown_trigger: broadcast::Sender<()>,
    pub(crate) task_handle: Option<JoinHandle<()>>,
}

impl SpawnSupervisorHandle {
    /// Returns the current state of the supervisor.
    #[must_use]
    pub fn current_state(&self) -> SpawnSupervisorState {
        *self.state_sender.borrow()
    }

    /// Requests the supervisor to shut down.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown(mut self) -> Result<(), SpawnSupervisorError> {
        let _ = self.shutdown_trigger.send(());

        let mut receiver = self.state_sender.subscribe();
        loop {
            match receiver.changed().await {
                Ok(()) => {
                    let state = *receiver.borrow();
                    match state {
                        SpawnSupervisorState::ShutDown => break,
                        SpawnSupervisorState::ShuttingDown => continue,
                        _ => {
                            return Err(SpawnSupervisorError::AtomicityViolation(format!(
                                "Unexpected state during shutdown: {:?}",
                                state
                            )));
                        }
                    }
                }
                Err(_) => {
                    return Err(SpawnSupervisorError::AlreadyShutdown);
                }
            }
        }

        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Spawn supervisor task cancelled during shutdown");
                    } else {
                        tracing::error!("Spawn supervisor task panicked during shutdown");
                    }
                }
            }
        }

        Ok(())
    }
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

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// `calculate_backoff_delay` - Calculate exponential backoff delay
///
/// Formula: `initial_backoff * backoff_multiplier^(attempt - 1)`
///
/// This function is a pure calculation with no side effects.
///
/// # Arguments
/// * `initial_backoff_ms` - Initial backoff duration in milliseconds
/// * `backoff_multiplier` - Multiplier for exponential backoff
/// * `attempt` - Current attempt number (1-indexed)
///
/// # Returns
/// Backoff delay in milliseconds
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
///
/// # Arguments
/// * `record` - The spawn record to check
///
/// # Returns
/// `true` if the spawn appears to be a zombie
#[inline]
#[must_use]
pub fn is_zombie_state(record: &SpawnRecord) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts > 3
}

/// `should_respawn` - Check if spawn should be respawned
///
/// Returns true if spawn is in failed phase and attempts are within limit.
///
/// # Arguments
/// * `record` - The spawn record to check
/// * `max_attempts` - Maximum allowed attempts
///
/// # Returns
/// `true` if the spawn should be respawned
#[inline]
#[must_use]
pub fn should_respawn(record: &SpawnRecord, max_attempts: u32) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts < max_attempts
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn test_instance_id() -> InstanceId {
        let ulid = Ulid::new();
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
        let record = SpawnRecord::new(
            test_instance_id(),
            "test".to_string(),
            None,
        );

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
    fn spawn_supervisor_error_is_transient() {
        assert!(SpawnSupervisorError::StorageError("test".to_string()).is_transient());
        assert!(SpawnSupervisorError::InstanceNotFound(test_instance_id()).is_transient());
        assert!(!SpawnSupervisorError::InvalidConfig("test".to_string()).is_transient());
    }

    #[test]
    fn spawn_supervisor_error_is_fatal() {
        assert!(SpawnSupervisorError::CorruptSpawn("test".to_string()).is_fatal());
        assert!(SpawnSupervisorError::InvalidConfig("test".to_string()).is_fatal());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_fatal());
    }
}
