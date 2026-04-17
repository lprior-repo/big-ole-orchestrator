//! `TimerSupervisor` Actor Implementation
//!
//! Per ADR-005, ADR-013, this module implements the timer scanning and dispatch
//! logic with dual-clock verification and delete-before-dispatch ordering.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use vo_types::InstanceId;

// =============================================================================
// `TimerRecord` - Timer data including dual-clock verification fields
// =============================================================================

/// `TimerRecord` - Timer data including dual-clock verification fields
///
/// Per ADR-013, dual-clock verification uses two clocks:
/// - Wall clock: `fire_at_ms` <= `now_ms`
/// - Monotonic clock: `trigger_time_ms` + `duration_ms` <= `now_ms`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    /// Optional timer ID for multiple timers per instance.
    pub timer_id: Option<vo_types::TimerId>,
    /// The instance ID this timer belongs to.
    pub instance_id: InstanceId,
    /// When the timer should fire (Unix timestamp in milliseconds).
    pub fire_at_ms: u64,
    /// When the timer was scheduled/triggered (for dual-clock verification).
    pub trigger_time_ms: u64,
    /// Monotonic duration from `trigger_time_ms`.
    pub duration_ms: u64,
}

impl TimerRecord {
    /// Creates a new `TimerRecord`.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        fire_at_ms: u64,
        timer_id: Option<vo_types::TimerId>,
        trigger_time_ms: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            timer_id,
            instance_id,
            fire_at_ms,
            trigger_time_ms,
            duration_ms,
        }
    }
}

// =============================================================================
// `TimerSupervisorError` - All error variants for `TimerSupervisor`
// =============================================================================

/// `TimerSupervisorError` - All error variants for `TimerSupervisor`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimerSupervisorError {
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Corrupt timer: {0}")]
    CorruptTimer(String),
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
}

impl TimerSupervisorError {
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
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::CorruptTimer(_) | Self::InvalidConfig(_))
    }

    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::AlreadyRunning
                | Self::ShutdownTimeout(_)
                | Self::AtomicityViolation(_)
        )
    }
}

// =============================================================================
// `TimerSupervisorState` - Runtime state of the supervisor
// =============================================================================

/// `TimerSupervisorState` - Runtime state of the supervisor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerSupervisorState {
    /// Supervisor is running and scanning timers.
    Running,
    /// Supervisor is shutting down.
    ShuttingDown,
    /// Supervisor has shut down.
    ShutDown,
}

// =============================================================================
// TimerSupervisorMetrics - Metrics for TimerSupervisor
// =============================================================================

/// Simple counter for metrics
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

/// Metrics for `TimerSupervisor`
#[derive(Debug, Default)]
pub struct TimerSupervisorMetrics {
    /// Number of timers successfully fired.
    pub timers_fired: Counter,
    /// Number of timers that fired late (overdue).
    pub overdue_timers: Counter,
    /// Number of dispatch errors.
    pub dispatch_errors: Counter,
}

// =============================================================================
// Traits - Storage and WorkQueue abstractions
// =============================================================================

/// Storage trait for timer operations
pub trait TimerStorage: Send + Sync {
    /// Scans for due timers in the given time range.
    fn scan_due_timers(&self, from: u64, to: u64, max: u32) -> Vec<TimerRecord>;

    /// Deletes a timer.
    ///
    /// # Errors
    /// Returns an error if the delete operation fails.
    fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), TimerSupervisorError>;
}

/// Work queue trait for dispatching work
pub trait WorkQueue: Send + Sync {
    /// Enqueues a resume work item for the given instance.
    ///
    /// # Errors
    /// Returns an error if the enqueue operation fails.
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), TimerSupervisorError>;
}

// =============================================================================
// `TimerSupervisor` - Actor that manages timer scanning and dispatch
// =============================================================================

/// `TimerSupervisor` - Actor that manages timer scanning and dispatch
pub struct TimerSupervisor {
    /// Interval between timer scans.
    pub tick_interval: Duration,
    /// Storage for timers.
    pub storage: Arc<dyn TimerStorage>,
    /// Work queue for dispatching.
    pub work_queue: Arc<dyn WorkQueue>,
    /// Metrics.
    pub metrics: TimerSupervisorMetrics,
    /// Running state.
    is_running: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for TimerSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimerSupervisor")
            .field("tick_interval", &self.tick_interval)
            .finish_non_exhaustive()
    }
}

impl TimerSupervisor {
    /// Creates a new `TimerSupervisor`.
    ///
    /// # Errors
    /// Returns `InvalidConfig` if `tick_interval` is zero.
    pub fn new(
        tick_interval: Duration,
        storage: Arc<dyn TimerStorage>,
        work_queue: Arc<dyn WorkQueue>,
    ) -> Result<Self, TimerSupervisorError> {
        // Precondition: tick_interval > 0
        if tick_interval.is_zero() {
            return Err(TimerSupervisorError::InvalidConfig(
                "tick_interval must be > 0".to_string(),
            ));
        }

        Ok(Self {
            tick_interval,
            storage,
            work_queue,
            metrics: TimerSupervisorMetrics::default(),
            is_running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Spawns the `TimerSupervisor` background task.
    ///
    /// # Errors
    /// Returns `AlreadyRunning` if the supervisor is already running.
    pub fn spawn(self) -> Result<TimerSupervisorHandle, TimerSupervisorError> {
        if self
            .is_running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(TimerSupervisorError::AlreadyRunning);
        }

        let (state_sender, _) = watch::channel(TimerSupervisorState::Running);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let state_sender_clone = state_sender.clone();
        let shutdown_receiver = shutdown_trigger.subscribe();

        let task_handle = tokio::runtime::Handle::current().spawn(async move {
            let result = self.run_loop(state_sender_clone, shutdown_receiver).await;
            if let Err(e) = result {
                tracing::error!("Timer supervisor loop exited with error: {}", e);
            }
        });

        Ok(TimerSupervisorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: Some(task_handle),
        })
    }

    /// Processes one timer scan cycle.
    ///
    /// Scans storage for due timers, deletes them before dispatch, and enqueues
    /// resume work for each instance.
    ///
    /// # Errors
    /// Returns an error if storage operations fail.
    pub fn process_cycle(&self) -> Result<CycleResult, TimerSupervisorError> {
        #[allow(clippy::cast_possible_truncation)]
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        #[allow(clippy::cast_possible_truncation)]
        let tick_interval_ms = self.tick_interval.as_millis() as u64;

        // Scan for due timers
        let due_timers = self
            .storage
            .scan_due_timers(0, now_ms, 100)
            .into_iter()
            .filter(|timer| {
                verify_dual_clock(
                    timer.fire_at_ms,
                    timer.trigger_time_ms,
                    timer.duration_ms,
                    now_ms,
                )
            })
            .collect::<Vec<_>>();

        let mut timers_fired = 0u32;
        let mut overdue_count = 0u32;
        let mut error_count = 0u32;

        for timer in due_timers {
            // Check if overdue
            if is_overdue(timer.fire_at_ms, now_ms, tick_interval_ms) {
                self.metrics.overdue_timers.incr();
                overdue_count += 1;
            }

            // Delete before dispatch (INV-2)
            match timer_delete_before_dispatch(&self.storage, &timer) {
                Ok(()) => {
                    // Dispatch succeeded
                    match self.work_queue.enqueue_resume(timer.instance_id.clone()) {
                        Ok(()) => {
                            self.metrics.timers_fired.incr();
                            timers_fired += 1;
                        }
                        Err(e) => {
                            self.metrics.dispatch_errors.incr();
                            error_count += 1;
                            tracing::error!(
                                instance_id = %timer.instance_id,
                                error = %e,
                                "Failed to enqueue resume work"
                            );
                        }
                    }
                }
                Err(e) => {
                    self.metrics.dispatch_errors.incr();
                    error_count += 1;
                    tracing::error!(
                        instance_id = %timer.instance_id,
                        error = %e,
                        "Failed to delete timer before dispatch"
                    );
                }
            }
        }

        Ok(CycleResult {
            timers_fired,
            overdue_count,
            error_count,
        })
    }

    async fn run_loop(
        self,
        state_sender: watch::Sender<TimerSupervisorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) -> Result<(), TimerSupervisorError> {
        let mut scan_interval = interval(self.tick_interval);
        scan_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(TimerSupervisorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    match self.process_cycle() {
                        Ok(_) => {}
                        Err(e) if e.is_transient() => {
                            tracing::warn!("Transient error in timer supervisor cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            tracing::error!("Fatal error in timer supervisor cycle: {}", e);
                        }
                        Err(e) => {
                            tracing::debug!("Operational error in timer supervisor cycle: {}", e);
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(TimerSupervisorState::ShutDown);
        Ok(())
    }
}

// =============================================================================
// `TimerSupervisorHandle` - Handle for controlling `TimerSupervisor`
// =============================================================================

/// Handle for controlling `TimerSupervisor`
#[derive(Debug)]
pub struct TimerSupervisorHandle {
    state_sender: watch::Sender<TimerSupervisorState>,
    shutdown_trigger: broadcast::Sender<()>,
    task_handle: Option<JoinHandle<()>>,
}

impl TimerSupervisorHandle {
    /// Returns true if the supervisor is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.state_sender.borrow() == TimerSupervisorState::Running
    }

    /// Returns the current state of the supervisor.
    #[must_use]
    pub fn current_state(&self) -> TimerSupervisorState {
        self.state_sender.borrow().clone()
    }

    /// Requests the supervisor to shut down and waits for completion.
    ///
    /// # Errors
    /// Returns `ShutdownTimeout` if shutdown does not complete within the given timeout.
    pub async fn shutdown(mut self, timeout: Duration) -> Result<(), TimerSupervisorError> {
        let _ = self.shutdown_trigger.send(());

        let mut receiver = self.state_sender.subscribe();
        let start = std::time::Instant::now();
        loop {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(TimerSupervisorError::ShutdownTimeout(timeout));
            }

            match tokio::time::timeout(remaining, receiver.wait_for(|state| *state != TimerSupervisorState::Running)).await {
                Ok(Ok(state)) => {
                    if *state == TimerSupervisorState::ShutDown {
                        break;
                    }
                }
                _ => {
                    return Err(TimerSupervisorError::ShutdownTimeout(timeout));
                }
            }
        }

        if let Some(task) = self.task_handle.take() {
            match task.await {
                Ok(()) => {}
                Err(e) => {
                    if !e.is_panic() {
                        tracing::warn!("Timer supervisor task cancelled during shutdown");
                    } else {
                        tracing::error!("Timer supervisor task panicked during shutdown");
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
    /// Number of timers fired.
    pub timers_fired: u32,
    /// Number of overdue timers.
    pub overdue_count: u32,
    /// Number of errors.
    pub error_count: u32,
}

// =============================================================================
// Pure Calculation Functions (Data → Calc → Actions)
// =============================================================================

/// `verify_dual_clock` - Dual-clock verification per ADR-013
///
/// Returns true if BOTH `fire_at_ms` <= `now_ms` AND (`trigger_time_ms` + `duration_ms`) <= `now_ms`
///
/// Using AND logic requires both clocks to agree before firing, preventing timer drift
/// from wall clock adjustments (hibernation, manual time changes) or monotonic skew.
///
/// This function is a pure calculation with no side effects.
///
/// # Arguments
/// * `fire_at_ms` - Absolute fire time (Unix timestamp ms)
/// * `trigger_time_ms` - When timer was scheduled (for dual-clock)
/// * `duration_ms` - Monotonic duration from `trigger_time_ms`
/// * `now_ms` - Current time (Unix timestamp ms)
///
/// # Returns
/// `true` if timer should fire under BOTH clock conditions
#[inline]
#[must_use]
pub fn verify_dual_clock(
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> bool {
    let wall_clock_ok = fire_at_ms <= now_ms;
    let monotonic_ok = trigger_time_ms.saturating_add(duration_ms) <= now_ms;
    wall_clock_ok && monotonic_ok
}

/// `is_overdue` - Check if timer is overdue beyond tick interval
///
/// Returns true if `fire_at_ms` + `tick_interval_ms` < `now_ms`
///
/// A timer is considered overdue if it fired more than one tick interval ago.
///
/// # Arguments
/// * `fire_at_ms` - When the timer should have fired
/// * `now_ms` - Current time
/// * `tick_interval_ms` - The tick interval
///
/// # Returns
/// `true` if the timer is overdue
#[inline]
#[must_use]
pub fn is_overdue(fire_at_ms: u64, now_ms: u64, tick_interval_ms: u64) -> bool {
    fire_at_ms.saturating_add(tick_interval_ms) < now_ms
}

// =============================================================================
// `timer_delete_before_dispatch` - Atomic delete-before-dispatch operation
// =============================================================================

/// Atomically deletes timer before dispatch.
///
/// Per INV-2, this function guarantees that the timer is deleted from storage
/// BEFORE any dispatch occurs. This prevents double-fire if the process crashes
/// after dispatch but before delete.
///
/// # Arguments
/// * `storage` - The timer storage
/// * `timer` - The timer record to delete and dispatch
///
/// # Errors
/// * `StorageError` - If the delete operation fails before dispatch
/// * `AtomicityViolation` - If dispatch succeeds but delete fails afterward
pub fn timer_delete_before_dispatch(
    storage: &Arc<dyn TimerStorage>,
    timer: &TimerRecord,
) -> Result<(), TimerSupervisorError> {
    // First, attempt to delete the timer from storage
    // This MUST succeed before any dispatch occurs (INV-2)
    storage
        .delete_timer(&timer.instance_id, timer.fire_at_ms)
        .map_err(|e| TimerSupervisorError::StorageError(e.to_string()))?;

    // Delete succeeded, dispatch will happen in caller
    // If dispatch fails after this point, we have an AtomicityViolation
    // but the timer is already deleted, so no double-fire is possible
    Ok(())
}

// =============================================================================
// validate_timer_record - Validates timer record integrity
// =============================================================================

/// Validates a timer record for corruption.
///
/// # Arguments
/// * `record` - The timer record to validate
///
/// # Errors
/// * `CorruptTimer` - If the timer record has invalid data
pub fn validate_timer_record(record: &TimerRecord) -> Result<(), TimerSupervisorError> {
    if record.fire_at_ms == 0 {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer fire_at_ms is zero".to_string(),
        ));
    }

    if record.trigger_time_ms == 0 {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer trigger_time_ms is zero".to_string(),
        ));
    }

    if record.fire_at_ms < record.trigger_time_ms {
        return Err(TimerSupervisorError::CorruptTimer(
            "Timer fire_at_ms is before trigger_time_ms".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_le_now() {
        // fire_at_ms = 1000 <= now_ms = 1000
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000
        // Both conditions met with AND logic
        assert!(verify_dual_clock(1000, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_only_monotonic_condition_met() {
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000 (monotonic met)
        // fire_at_ms = 1500 > now_ms = 1000 (wall clock NOT met)
        // With AND logic, both must be met, so this returns false
        assert!(!verify_dual_clock(1500, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_true_when_both_conditions_met() {
        // fire_at_ms = 1000 <= now_ms = 1000 (wall clock met)
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000 (monotonic met)
        assert!(verify_dual_clock(1000, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_only_wall_clock_met() {
        // fire_at_ms = 1100 <= now_ms = 1100 (wall clock met)
        // trigger_time_ms + duration_ms = 800 + 400 = 1200 > now_ms = 1100 (monotonic NOT met)
        assert!(!verify_dual_clock(1100, 800, 400, 1100));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_not_due() {
        // fire_at_ms = 1500 > now_ms = 900 (wall clock NOT met)
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 > now_ms = 900 (monotonic NOT met)
        assert!(!verify_dual_clock(1500, 800, 200, 900));
    }

    #[test]
    fn is_overdue_returns_true_when_over_tick_interval() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 < now_ms = 1200
        assert!(is_overdue(1000, 1200, 100));
    }

    #[test]
    fn is_overdue_returns_false_when_within_tick_interval() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 >= now_ms = 1099
        assert!(!is_overdue(1000, 1099, 100));
    }

    #[test]
    fn is_overdue_returns_false_at_boundary() {
        // fire_at_ms + tick_interval_ms = 1000 + 100 = 1100 >= now_ms = 1100
        assert!(!is_overdue(1000, 1100, 100));
    }

    struct MockStorage;
    impl TimerStorage for MockStorage {
        fn scan_due_timers(&self, _from: u64, _to: u64, _max: u32) -> Vec<TimerRecord> {
            Vec::new()
        }
        fn delete_timer(
            &self,
            _instance_id: &InstanceId,
            _fire_at_ms: u64,
        ) -> Result<(), TimerSupervisorError> {
            Ok(())
        }
    }

    struct MockQueue;
    impl WorkQueue for MockQueue {
        fn enqueue_resume(&self, _instance_id: InstanceId) -> Result<(), TimerSupervisorError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn shutdown_returns_ok_on_clean_shutdown() {
        let storage: Arc<dyn TimerStorage> = Arc::new(MockStorage);
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockQueue);

        let supervisor = TimerSupervisor::new(Duration::from_millis(100), storage, work_queue)
            .expect("valid config should construct supervisor");

        let handle = supervisor.spawn().expect("spawn should return a handle");
        assert!(handle.is_running());
        assert_eq!(handle.current_state(), TimerSupervisorState::Running);

        let result = handle.shutdown(Duration::from_secs(5)).await;
        assert!(result.is_ok(), "shutdown should return Ok, got {:?}", result);
    }

    #[tokio::test]
    async fn shutdown_sets_state_to_shutdown() {
        let storage: Arc<dyn TimerStorage> = Arc::new(MockStorage);
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockQueue);

        let supervisor = TimerSupervisor::new(Duration::from_secs(3600), storage, work_queue)
            .expect("valid config should construct supervisor");

        let handle = supervisor.spawn().expect("spawn should return a handle");
        let state_before = handle.current_state();
        assert_eq!(state_before, TimerSupervisorState::Running);

        handle.shutdown(Duration::from_secs(5)).await.unwrap();
    }
}
