//! Lock Manager Supervisor Module
//!
//! Provides lifecycle management for the distributed lock manager including:
//! - Startup initialization
//! - Graceful shutdown with operation draining
//! - Health monitoring with periodic checks
//! - Degraded state reporting
//!
//! # State Machine
//!
//! ```text
//!  Starting → Running → Stopping → Stopped
//!     │          │
//!     └──────────┴──→ Degraded (health check failure)
//! ```
//!
//! # ADR-046 Contract
//!
//! This implementation follows the async supervisor contract for lifecycle management.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use crate::port::LockManager;
use crate::{LockEntry, LockId, OwnerId};

// =============================================================================
// LockManagerSupervisorState - Runtime state of the supervisor
// =============================================================================

/// `LockManagerSupervisorState` - Runtime state of the lock manager supervisor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockManagerSupervisorState {
    /// Supervisor is starting up and initializing components.
    Starting,
    /// Supervisor is running and healthy.
    Running,
    /// Supervisor is running but in degraded mode (health check failed).
    Degraded,
    /// Supervisor is initiating graceful shutdown.
    Stopping,
    /// Supervisor has completed shutdown.
    Stopped,
}

impl LockManagerSupervisorState {
    /// Returns true if this is a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped)
    }

    /// Returns true if the supervisor is active (can process operations).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Degraded)
    }
}

impl std::fmt::Display for LockManagerSupervisorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Degraded => write!(f, "degraded"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

// =============================================================================
// LockManagerSupervisorError - Error types for LockManagerSupervisor
// =============================================================================

/// `LockManagerSupervisorError` - Error variants for `LockManagerSupervisor`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockManagerSupervisorError {
    #[error("Lock manager not initialized")]
    NotInitialized,

    #[error("Supervisor already running")]
    AlreadyRunning,

    #[error("Supervisor not running")]
    NotRunning,

    #[error("Shutdown timeout after {0:?}")]
    ShutdownTimeout(Duration),

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Lock operation failed: {0}")]
    LockOperationFailed(String),

    #[error("Component initialization failed: {0}")]
    InitFailed(String),

    #[error("Drain timeout with {pending} operations pending")]
    DrainTimeout { pending: usize },
}

impl LockManagerSupervisorError {
    /// Returns true if this is a transient error that may resolve on retry.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::LockOperationFailed(_) | Self::HealthCheckFailed(_))
    }

    /// Returns true if this is a fatal error requiring intervention.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::InitFailed(_) | Self::NotInitialized)
    }

    /// Returns true if this is an operational error.
    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::AlreadyRunning | Self::NotRunning | Self::ShutdownTimeout(_) | Self::DrainTimeout { .. }
        )
    }
}

// =============================================================================
// LockManagerSupervisorMetrics - Metrics for LockManagerSupervisor
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

/// Metrics for `LockManagerSupervisor`
#[derive(Debug, Default)]
pub struct LockManagerSupervisorMetrics {
    /// Number of successful health checks.
    pub health_checks_ok: Counter,
    /// Number of failed health checks.
    pub health_checks_failed: Counter,
    /// Number of times supervisor entered degraded state.
    pub degraded_enter: Counter,
    /// Number of times supervisor recovered to healthy state.
    pub degraded_recover: Counter,
    /// Number of locks acquired.
    pub locks_acquired: Counter,
    /// Number of locks released.
    pub locks_released: Counter,
    /// Number of lock operations that timed out.
    pub lock_timeouts: Counter,
    /// Number of operations drained during shutdown.
    pub operations_drained: Counter,
}

/// Health status of a lock manager component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is degraded but operational.
    Degraded,
    /// Component is unhealthy.
    Unhealthy,
}

/// Result of a health check operation.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Overall health status.
    pub status: HealthStatus,
    /// Number of locks currently held.
    pub locks_held: usize,
    /// Number of pending operations.
    pub pending_operations: usize,
    /// Description of any issues found.
    pub issues: Vec<String>,
}

// =============================================================================
// LockManagerSupervisor - Main supervisor actor
// =============================================================================

/// `LockManagerSupervisor` - Actor that manages lock manager lifecycle
///
/// This supervisor is responsible for:
/// - Initializing the lock manager and its dependencies
/// - Running periodic health checks
/// - Handling graceful shutdown with operation draining
/// - Reporting degraded state when health checks fail
pub struct LockManagerSupervisor {
    /// Interval between health checks.
    health_check_interval: Duration,
    /// Timeout for graceful shutdown.
    shutdown_timeout: Duration,
    /// Timeout for draining operations during shutdown.
    drain_timeout: Duration,
    /// Maximum number of pending operations before considering drain complete.
    max_pending_for_drain: usize,
    /// The managed lock manager.
    lock_manager: Arc<dyn LockManager>,
    /// Metrics.
    metrics: LockManagerSupervisorMetrics,
    /// Running state.
    is_running: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for LockManagerSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockManagerSupervisor")
            .field("health_check_interval", &self.health_check_interval)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("drain_timeout", &self.drain_timeout)
            .finish_non_exhaustive()
    }
}

impl LockManagerSupervisor {
    /// Creates a new `LockManagerSupervisor`.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if any configuration is invalid.
    pub fn new(
        health_check_interval: Duration,
        shutdown_timeout: Duration,
        drain_timeout: Duration,
        max_pending_for_drain: usize,
        lock_manager: Arc<dyn LockManager>,
    ) -> Result<Self, LockManagerSupervisorError> {
        if health_check_interval.is_zero() {
            return Err(LockManagerSupervisorError::InitFailed(
                "health_check_interval must be > 0".to_string(),
            ));
        }

        if shutdown_timeout.is_zero() {
            return Err(LockManagerSupervisorError::InitFailed(
                "shutdown_timeout must be > 0".to_string(),
            ));
        }

        if drain_timeout.is_zero() {
            return Err(LockManagerSupervisorError::InitFailed(
                "drain_timeout must be > 0".to_string(),
            ));
        }

        if max_pending_for_drain == 0 {
            return Err(LockManagerSupervisorError::InitFailed(
                "max_pending_for_drain must be > 0".to_string(),
            ));
        }

        Ok(Self {
            health_check_interval,
            shutdown_timeout,
            drain_timeout,
            max_pending_for_drain,
            lock_manager,
            metrics: LockManagerSupervisorMetrics::default(),
            is_running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Spawns the `LockManagerSupervisor` background task.
    ///
    /// # Errors
    /// Returns `AlreadyRunning` if the supervisor is already running.
    pub fn spawn(self) -> Result<LockManagerSupervisorHandle, LockManagerSupervisorError> {
        if self
            .is_running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(LockManagerSupervisorError::AlreadyRunning);
        }

        let (state_sender, _) = watch::channel(LockManagerSupervisorState::Starting);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            let result = self.run_loop(state_sender_clone, shutdown_receiver).await;
            if let Err(e) = result {
                tracing::error!("Lock manager supervisor loop exited with error: {}", e);
            }
        });

        Ok(LockManagerSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: Some(task_handle),
        })
    }

    /// Performs a health check on the lock manager.
    async fn perform_health_check(&self) -> HealthCheckResult {
        let mut issues = Vec::new();
        let mut locks_held = 0usize;
        let mut pending_operations = 0usize;

        let response = self.lock_manager.query(crate::LockQuery {
            lock_id: None,
            owner: None,
        }).await;

        locks_held = response.locks.len();
        for lock in &response.locks {
            if lock.status == crate::LockStatus::Expired {
                issues.push(format!("Expired lock found: {:?}", lock.lock_id));
            }
        }

        let status = if issues.is_empty() {
            HealthStatus::Healthy
        } else if locks_held > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        HealthCheckResult {
            status,
            locks_held,
            pending_operations,
            issues,
        }
    }

    /// Drains pending operations during shutdown.
    async fn drain_operations(&self) -> Result<usize, LockManagerSupervisorError> {
        let start = std::time::Instant::now();
        let mut drained = 0;

        while start.elapsed() < self.drain_timeout {
            let pending = self.count_pending_operations().await;

            if pending == 0 {
                return Ok(drained);
            }

            if pending <= self.max_pending_for_drain {
                drained += pending;
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                return Err(LockManagerSupervisorError::DrainTimeout { pending });
            }
        }

        let pending = self.count_pending_operations().await;
        Err(LockManagerSupervisorError::DrainTimeout { pending })
    }

    /// Counts the number of pending operations.
    async fn count_pending_operations(&self) -> usize {
        let response = self.lock_manager
            .query(crate::LockQuery {
                lock_id: None,
                owner: None,
            })
            .await;
        response.locks.iter().filter(|l| l.status == crate::LockStatus::Pending).count()
    }

    /// The main loop implementation.
    #[tracing::instrument(skip_all)]
    async fn run_loop(
        self,
        state_sender: watch::Sender<LockManagerSupervisorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) -> Result<(), LockManagerSupervisorError> {
        let _ = state_sender.send(LockManagerSupervisorState::Starting);

        let mut health_check_interval = interval(self.health_check_interval);
        health_check_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let _ = state_sender.send(LockManagerSupervisorState::Running);
        tracing::info!("Lock manager supervisor started");

        let mut degraded = false;

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    tracing::info!("Lock manager supervisor received shutdown signal");
                    let _ = state_sender.send(LockManagerSupervisorState::Stopping);

                    match self.drain_operations().await {
                        Ok(drained) => {
                            tracing::info!(drained = drained, "Drained operations during shutdown");
                            self.metrics.operations_drained.incr();
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Drain failed or timed out during shutdown");
                        }
                    }

                    break;
                }
                _ = health_check_interval.tick() => {
                    let result = self.perform_health_check().await;

                    match result.status {
                        HealthStatus::Healthy => {
                            if degraded {
                                tracing::info!("Lock manager recovered to healthy state");
                                self.metrics.degraded_recover.incr();
                                degraded = false;
                            }
                            let _ = state_sender.send(LockManagerSupervisorState::Running);
                            self.metrics.health_checks_ok.incr();
                        }
                        HealthStatus::Degraded | HealthStatus::Unhealthy => {
                            if !degraded {
                                tracing::warn!(
                                    locks_held = result.locks_held,
                                    issues = ?result.issues,
                                    "Lock manager entered degraded state"
                                );
                                self.metrics.degraded_enter.incr();
                                degraded = true;
                            }
                            let _ = state_sender.send(LockManagerSupervisorState::Degraded);
                            self.metrics.health_checks_failed.incr();

                            for issue in &result.issues {
                                tracing::warn!(issue = %issue, "Health check issue");
                            }
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(LockManagerSupervisorState::Stopped);
        tracing::info!("Lock manager supervisor stopped");
        Ok(())
    }
}

// =============================================================================
// LockManagerSupervisorHandle - Handle for controlling LockManagerSupervisor
// =============================================================================

/// Handle for controlling `LockManagerSupervisor`
#[derive(Debug)]
pub struct LockManagerSupervisorHandle {
    state_sender: watch::Sender<LockManagerSupervisorState>,
    shutdown_trigger: broadcast::Sender<()>,
    task_handle: Option<JoinHandle<()>>,
}

impl LockManagerSupervisorHandle {
    /// Returns true if the supervisor is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state_sender.borrow().is_active()
    }

    /// Returns the current state of the supervisor.
    #[must_use]
    pub fn current_state(&self) -> LockManagerSupervisorState {
        *self.state_sender.borrow()
    }

    /// Requests the supervisor to shut down and waits for completion.
    ///
    /// # Errors
    /// Returns `ShutdownTimeout` if shutdown does not complete within the given timeout.
    pub async fn shutdown(mut self, timeout: Duration) -> Result<(), LockManagerSupervisorError> {
        let _ = self.shutdown_trigger.send(());

        let mut receiver = self.state_sender.subscribe();
        let start = std::time::Instant::now();
        loop {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(LockManagerSupervisorError::ShutdownTimeout(timeout));
            }

            match tokio::time::timeout(
                remaining,
                receiver.wait_for(|state| *state != LockManagerSupervisorState::Running
                    && *state != LockManagerSupervisorState::Degraded
                    && *state != LockManagerSupervisorState::Starting),
            ).await {
                Ok(Ok(state)) => {
                    if *state == LockManagerSupervisorState::Stopped {
                        break;
                    }
                }
                _ => {
                    return Err(LockManagerSupervisorError::ShutdownTimeout(timeout));
                }
            }
        }

        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Lock manager supervisor task cancelled during shutdown");
                    } else {
                        tracing::error!("Lock manager supervisor task panicked during shutdown");
                    }
                }
            }
        }

        Ok(())
    }
}

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// Determines if the supervisor should transition to degraded state.
#[inline]
#[must_use]
pub fn should_transition_to_degraded(result: &HealthCheckResult) -> bool {
    !result.issues.is_empty() || result.status == HealthStatus::Unhealthy
}

/// Determines if the supervisor can recover from degraded state.
#[inline]
#[must_use]
pub fn can_recover_from_degraded(result: &HealthCheckResult) -> bool {
    result.issues.is_empty() && result.status == HealthStatus::Healthy
}

/// Checks if shutdown should proceed based on pending operations.
#[inline]
#[must_use]
pub fn can_proceed_with_shutdown(pending: usize, max_pending: usize) -> bool {
    pending <= max_pending
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_state_is_terminal() {
        assert!(!LockManagerSupervisorState::Starting.is_terminal());
        assert!(!LockManagerSupervisorState::Running.is_terminal());
        assert!(!LockManagerSupervisorState::Degraded.is_terminal());
        assert!(!LockManagerSupervisorState::Stopping.is_terminal());
        assert!(LockManagerSupervisorState::Stopped.is_terminal());
    }

    #[test]
    fn supervisor_state_is_active() {
        assert!(LockManagerSupervisorState::Starting.is_active());
        assert!(LockManagerSupervisorState::Running.is_active());
        assert!(LockManagerSupervisorState::Degraded.is_active());
        assert!(!LockManagerSupervisorState::Stopping.is_active());
        assert!(!LockManagerSupervisorState::Stopped.is_active());
    }

    #[test]
    fn supervisor_state_display() {
        assert_eq!(format!("{}", LockManagerSupervisorState::Starting), "starting");
        assert_eq!(format!("{}", LockManagerSupervisorState::Running), "running");
        assert_eq!(format!("{}", LockManagerSupervisorState::Degraded), "degraded");
        assert_eq!(format!("{}", LockManagerSupervisorState::Stopping), "stopping");
        assert_eq!(format!("{}", LockManagerSupervisorState::Stopped), "stopped");
    }

    #[test]
    fn error_is_transient() {
        assert!(LockManagerSupervisorError::LockOperationFailed("test".to_string()).is_transient());
        assert!(LockManagerSupervisorError::HealthCheckFailed("test".to_string()).is_transient());
        assert!(!LockManagerSupervisorError::InitFailed("test".to_string()).is_transient());
    }

    #[test]
    fn error_is_fatal() {
        assert!(LockManagerSupervisorError::InitFailed("test".to_string()).is_fatal());
        assert!(LockManagerSupervisorError::NotInitialized.is_fatal());
        assert!(!LockManagerSupervisorError::LockOperationFailed("test".to_string()).is_fatal());
    }

    #[test]
    fn error_is_operational() {
        assert!(LockManagerSupervisorError::AlreadyRunning.is_operational());
        assert!(LockManagerSupervisorError::NotRunning.is_operational());
        assert!(LockManagerSupervisorError::ShutdownTimeout(Duration::from_secs(30)).is_operational());
        assert!(LockManagerSupervisorError::DrainTimeout { pending: 5 }.is_operational());
        assert!(!LockManagerSupervisorError::LockOperationFailed("test".to_string()).is_operational());
    }

    #[test]
    fn should_transition_to_degraded_with_issues() {
        let result = HealthCheckResult {
            status: HealthStatus::Degraded,
            locks_held: 5,
            pending_operations: 0,
            issues: vec!["Expired lock found".to_string()],
        };
        assert!(should_transition_to_degraded(&result));
    }

    #[test]
    fn should_not_transition_to_degraded_when_healthy() {
        let result = HealthCheckResult {
            status: HealthStatus::Healthy,
            locks_held: 5,
            pending_operations: 0,
            issues: vec![],
        };
        assert!(!should_transition_to_degraded(&result));
    }

    #[test]
    fn test_can_recover_from_degraded() {
        let healthy_result = HealthCheckResult {
            status: HealthStatus::Healthy,
            locks_held: 0,
            pending_operations: 0,
            issues: vec![],
        };
        assert!(can_recover_from_degraded(&healthy_result));

        let degraded_result = HealthCheckResult {
            status: HealthStatus::Degraded,
            locks_held: 5,
            pending_operations: 0,
            issues: vec!["Some issue".to_string()],
        };
        assert!(!can_recover_from_degraded(&degraded_result));
    }

    #[test]
    fn can_proceed_with_shutdown_checks_pending() {
        assert!(can_proceed_with_shutdown(0, 10));
        assert!(can_proceed_with_shutdown(5, 10));
        assert!(can_proceed_with_shutdown(10, 10));
        assert!(!can_proceed_with_shutdown(11, 10));
    }

    #[tokio::test]
    async fn supervisor_creation_with_valid_config() {
        struct MockLockManager;
        #[async_trait::async_trait]
        impl LockManager for MockLockManager {
            async fn acquire(&self, _request: crate::LockRequest) -> crate::LockResponse {
                crate::LockResponse {
                    request_id: String::new(),
                    lock_id: LockId::new("test"),
                    owner: OwnerId::new("test".into()),
                    granted: true,
                    hold_token: None,
                    expires_at: None,
                    error: None,
                }
            }
            async fn release(&self, _release: crate::LockRelease) -> Result<(), crate::LockError> {
                Ok(())
            }
            async fn query(&self, _query: crate::LockQuery) -> crate::LockQueryResponse {
                crate::LockQueryResponse { locks: vec![] }
            }
            async fn promote(&self, _promote: crate::LockPromote) -> crate::LockPromoteResponse {
                crate::LockPromoteResponse {
                    request_id: String::new(),
                    lock_id: LockId::new("test"),
                    granted: false,
                    new_mode: None,
                    error: None,
                }
            }
            async fn demote(&self, _lock_id: LockId, _owner: OwnerId, _hold_token: String) -> Result<crate::LockMode, crate::LockError> {
                Ok(crate::LockMode::Shared)
            }
            async fn extend_ttl(&self, _lock_id: LockId, _owner: OwnerId, _hold_token: String, _ttl_ms: u64) -> Result<chrono::DateTime<chrono::Utc>, crate::LockError> {
                Ok(chrono::Utc::now())
            }
            async fn is_locked(&self, _lock_id: &LockId) -> bool {
                false
            }
            async fn get_holder(&self, _lock_id: &LockId) -> Option<(OwnerId, crate::LockMode)> {
                None
            }
        }

        let supervisor = LockManagerSupervisor::new(
            Duration::from_secs(1),
            Duration::from_secs(30),
            Duration::from_secs(10),
            100,
            Arc::new(MockLockManager),
        );
        assert!(supervisor.is_ok());
    }

    #[tokio::test]
    async fn supervisor_creation_rejects_zero_interval() {
        struct MockLockManager;
        #[async_trait::async_trait]
        impl LockManager for MockLockManager {
            async fn acquire(&self, _request: crate::LockRequest) -> crate::LockResponse {
                crate::LockResponse {
                    request_id: String::new(),
                    lock_id: LockId::new("test"),
                    owner: OwnerId::new("test".into()),
                    granted: true,
                    hold_token: None,
                    expires_at: None,
                    error: None,
                }
            }
            async fn release(&self, _release: crate::LockRelease) -> Result<(), crate::LockError> {
                Ok(())
            }
            async fn query(&self, _query: crate::LockQuery) -> crate::LockQueryResponse {
                crate::LockQueryResponse { locks: vec![] }
            }
            async fn promote(&self, _promote: crate::LockPromote) -> crate::LockPromoteResponse {
                crate::LockPromoteResponse {
                    request_id: String::new(),
                    lock_id: LockId::new("test"),
                    granted: false,
                    new_mode: None,
                    error: None,
                }
            }
            async fn demote(&self, _lock_id: LockId, _owner: OwnerId, _hold_token: String) -> Result<crate::LockMode, crate::LockError> {
                Ok(crate::LockMode::Shared)
            }
            async fn extend_ttl(&self, _lock_id: LockId, _owner: OwnerId, _hold_token: String, _ttl_ms: u64) -> Result<chrono::DateTime<chrono::Utc>, crate::LockError> {
                Ok(chrono::Utc::now())
            }
            async fn is_locked(&self, _lock_id: &LockId) -> bool {
                false
            }
            async fn get_holder(&self, _lock_id: &LockId) -> Option<(OwnerId, crate::LockMode)> {
                None
            }
        }

        let result = LockManagerSupervisor::new(
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(10),
            100,
            Arc::new(MockLockManager),
        );
        assert!(result.is_err());
    }
}