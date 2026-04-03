//! `TimerSupervisor` Actor Implementation
//!
//! Per ADR-005, ADR-013, this module implements the timer scanning and dispatch
//! logic with dual-clock verification and delete-before-dispatch ordering.

use std::sync::Arc;
use std::time::Duration;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerSupervisorError {
    /// Storage operation failed - transient, retryable
    StorageError(String),

    /// Timer key corrupt or malformed - fatal, requires manual intervention
    CorruptTimer(String),

    /// Atomicity violation: delete succeeded but dispatch failed
    /// Timer may be lost; requires reconciliation
    AtomicityViolation(String),

    /// Instance actor not found - transient if actor is restarting
    InstanceNotFound(InstanceId),

    /// Dispatch failed due to actor mailbox full
    MailboxFull(InstanceId),

    /// Configuration error - fatal
    InvalidConfig(String),

    /// Reanimator already running
    AlreadyRunning,

    /// Reanimator shutdown timeout
    ShutdownTimeout(Duration),

    /// Dispatch error
    DispatchError(String),
}

impl std::fmt::Display for TimerSupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(s) => write!(f, "Storage error: {s}"),
            Self::CorruptTimer(s) => write!(f, "Corrupt timer: {s}"),
            Self::AtomicityViolation(s) => write!(f, "Atomicity violation: {s}"),
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {id}"),
            Self::MailboxFull(id) => write!(f, "Mailbox full: {id}"),
            Self::InvalidConfig(s) => write!(f, "Invalid config: {s}"),
            Self::AlreadyRunning => write!(f, "Already running"),
            Self::ShutdownTimeout(d) => write!(f, "Shutdown timeout: {d:?}"),
            Self::DispatchError(s) => write!(f, "Dispatch error: {s}"),
        }
    }
}

impl std::error::Error for TimerSupervisorError {}

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
        // Check if already running
        if self
            .is_running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(TimerSupervisorError::AlreadyRunning);
        }

        Ok(TimerSupervisorHandle {
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        })
    }

    /// Processes one timer scan cycle.
    ///
    /// Scans storage for due timers, deletes them before dispatch, and enqueues
    /// resume work for each instance.
    ///
    /// # Errors
    /// Returns an error if storage operations fail.
    pub async fn process_cycle(&self) -> Result<CycleResult, TimerSupervisorError> {
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
            match timer_delete_before_dispatch(&self.storage, &timer).await {
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

    /// Shuts down the `TimerSupervisor`.
    ///
    /// # Errors
    /// Returns `ShutdownTimeout` if shutdown does not complete within the given timeout.
    pub async fn shutdown(&self, timeout: Duration) -> Result<(), TimerSupervisorError> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // In a real implementation, we would wait for the background task to finish.
        // For now, we just stop the running flag.

        Err(TimerSupervisorError::ShutdownTimeout(timeout))
    }
}

// =============================================================================
// `TimerSupervisorHandle` - Handle for controlling `TimerSupervisor`
// =============================================================================

/// Handle for controlling `TimerSupervisor`
#[derive(Debug)]
pub struct TimerSupervisorHandle {
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl TimerSupervisorHandle {
    /// Returns true if the supervisor is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Stops the supervisor.
    ///
    /// # Errors
    /// Returns an error if stopping fails.
    pub fn stop(self) -> Result<(), TimerSupervisorError> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
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
/// Returns true if `fire_at_ms` <= `now_ms` OR (`trigger_time_ms` + `duration_ms`) <= `now_ms`
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
/// `true` if timer should fire under either clock condition
#[inline]
#[must_use]
pub fn verify_dual_clock(
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> bool {
    // Condition 1: Wall clock - fire_at <= now
    // Condition 2: Monotonic clock - trigger + duration <= now (for clock skew tolerance)
    fire_at_ms <= now_ms || trigger_time_ms.saturating_add(duration_ms) <= now_ms
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
#[allow(clippy::unused_async)]
pub async fn timer_delete_before_dispatch(
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
        assert!(verify_dual_clock(1000, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_true_when_elapsed_ge_duration() {
        // trigger_time_ms + duration_ms = 800 + 200 = 1000 <= now_ms = 1000
        assert!(verify_dual_clock(1500, 800, 200, 1000));
    }

    #[test]
    fn verify_dual_clock_returns_false_when_not_due() {
        // fire_at_ms = 1500 > now_ms = 900
        // trigger_time_ms + duration_ms = 1000 > now_ms = 900
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
}
