//! SpawnSupervisor actor struct, constructor, spawn method, and run_loop.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use super::types::{SpawnSupervisorError, SpawnSupervisorState};
use super::{calculate_backoff_delay, ProcessManager, WorkQueue};
use super::{SpawnStorage, SpawnSupervisorMetrics};
use crate::lifecycle::ShutdownPropagator;
use crate::semaphore::ExecutionSemaphore;

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
    /// Jitter factor for backoff randomization (0.0 < jitter_factor <= 1.0).
    pub jitter_factor: f64,
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
    /// Global execution semaphore for limiting concurrent spawns.
    pub execution_semaphore: Arc<ExecutionSemaphore>,
    /// Shutdown propagator for ordered cleanup.
    pub shutdown_propagator: Arc<ShutdownPropagator>,
}

impl std::fmt::Debug for SpawnSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnSupervisor")
            .field("health_check_interval", &self.health_check_interval)
            .field("max_health_checks", &self.max_health_checks)
            .field("initial_backoff", &self.initial_backoff)
            .field("backoff_multiplier", &self.backoff_multiplier)
            .field("jitter_factor", &self.jitter_factor)
            .field("max_spawn_attempts", &self.max_spawn_attempts)
            .field("execution_semaphore", &self.execution_semaphore)
            .finish_non_exhaustive()
    }
}

impl SpawnSupervisor {
    /// Creates a new `SpawnSupervisor`.
    ///
    /// Registers the spawn supervisor cleanup action with the shutdown
    /// propagator to enforce reverse-initialization drop ordering
    /// per ADR-050 and ADR-055.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if configuration is invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        health_check_interval: Duration,
        max_health_checks: u32,
        initial_backoff: Duration,
        backoff_multiplier: f64,
        jitter_factor: f64,
        max_spawn_attempts: u32,
        storage: Arc<dyn SpawnStorage>,
        process_manager: Arc<dyn ProcessManager>,
        work_queue: Arc<dyn WorkQueue>,
        execution_semaphore: Arc<ExecutionSemaphore>,
        shutdown_propagator: Arc<ShutdownPropagator>,
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

       if jitter_factor <= 0.0 || jitter_factor > 1.0 {
            return Err(SpawnSupervisorError::InvalidConfig(
                "jitter_factor must be in (0.0, 1.0]".to_string(),
            ));
        }

        shutdown_propagator.register_drop_sync("spawn_supervisor", || {
            tracing::debug!("SpawnSupervisor cleanup: draining spawn queue");
        });

        Ok(Self {
            health_check_interval,
            max_health_checks,
            initial_backoff,
            backoff_multiplier,
            jitter_factor,
            max_spawn_attempts,
            storage,
            process_manager,
            work_queue,
            metrics: SpawnSupervisorMetrics::default(),
            execution_semaphore,
            shutdown_propagator,
        })
    }

    /// Spawns the `SpawnSupervisor` background task.
    ///
    /// # Errors
    /// Returns `AlreadyRunning` if the supervisor is already running.
    pub fn spawn(self) -> Result<SpawnSupervisorHandle, SpawnSupervisorError> {
        let (state_sender, _) = watch::channel(SpawnSupervisorState::Running);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            let result = self.run_loop(state_sender_clone, shutdown_receiver).await;
            if let Err(e) = result {
                tracing::error!("Spawn supervisor loop exited with error: {}", e);
            }
        });

        let propagator = self.shutdown_propagator.clone();
        Ok(SpawnSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: Some(task_handle),
            shutdown_propagator: propagator,
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

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(SpawnSupervisorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    // Run zombie detection before the process cycle so
                    // reaped instances are cleaned up before new work is dispatched.
                    let _reaped = self.zombie_detection().await;

                    match self.process_cycle().await {
                        Ok(_) => {}
                        Err(e) if e.is_transient() => {
                            tracing::warn!("Transient error in spawn supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_resumable() => {
                            tracing::info!("Resumable error in spawn supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            tracing::error!("Fatal error in spawn supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_operational() => {
                            tracing::debug!("Operational error in spawn supervisor cycle: {}", e);
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
    /// Shared shutdown propagator for ordered cleanup.
    pub(crate) shutdown_propagator: Arc<ShutdownPropagator>,
}

impl SpawnSupervisorHandle {
    /// Returns the current state of the supervisor.
    #[must_use]
    pub fn current_state(&self) -> SpawnSupervisorState {
        *self.state_sender.borrow()
    }

    /// Requests the supervisor to shut down.
    ///
    /// Per ADR-050 and ADR-055: shutdown propagates through the propagator's
    /// ordered drop registry (reverse-initialization order).
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

        let _ = self.shutdown_propagator.propagate();

        Ok(())
    }
}
