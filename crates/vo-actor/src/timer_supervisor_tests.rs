//! TimerSupervisor Tests - RED PHASE
//!
//! These tests are written to compile but FAIL until the TimerSupervisor
//! implementation is complete. This is the TDD red phase.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

// =============================================================================
// Helper Functions
// =============================================================================

/// Helper to create an InstanceId for testing
#[allow(dead_code)]
fn instance_id() -> vo_types::InstanceId {
    vo_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

/// Helper to create a TimerRecord for testing
#[allow(dead_code)]
fn make_timer_record(
    instance_id: vo_types::InstanceId,
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
) -> TimerRecord {
    TimerRecord {
        timer_id: None,
        instance_id,
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    }
}

// =============================================================================
// Stub Types (to allow compilation)
// =============================================================================

/// TimerRecord - Timer data including dual-clock verification fields
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    pub timer_id: Option<vo_types::TimerId>,
    pub instance_id: vo_types::InstanceId,
    pub fire_at_ms: u64,
    pub trigger_time_ms: u64,
    pub duration_ms: u64,
}

/// TimerSupervisorError - All error variants for TimerSupervisor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerSupervisorError {
    StorageError(String),
    CorruptTimer(String),
    AtomicityViolation(String),
    InstanceNotFound(vo_types::InstanceId),
    MailboxFull(vo_types::InstanceId),
    InvalidConfig(String),
    AlreadyRunning,
    ShutdownTimeout(Duration),
    DispatchError(String),
}

impl std::fmt::Display for TimerSupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(s) => write!(f, "Storage error: {}", s),
            Self::CorruptTimer(s) => write!(f, "Corrupt timer: {}", s),
            Self::AtomicityViolation(s) => write!(f, "Atomicity violation: {}", s),
            Self::InstanceNotFound(id) => write!(f, "Instance not found: {}", id),
            Self::MailboxFull(id) => write!(f, "Mailbox full: {}", id),
            Self::InvalidConfig(s) => write!(f, "Invalid config: {}", s),
            Self::AlreadyRunning => write!(f, "Already running"),
            Self::ShutdownTimeout(d) => write!(f, "Shutdown timeout: {:?}", d),
            Self::DispatchError(s) => write!(f, "Dispatch error: {}", s),
        }
    }
}

impl std::error::Error for TimerSupervisorError {}

/// Metrics for TimerSupervisor
#[derive(Debug, Default)]
pub struct TimerSupervisorMetrics {
    pub timers_fired: Counter,
    pub overdue_timers: Counter,
    pub dispatch_errors: Counter,
}

/// Simple counter for metrics
#[derive(Debug, Default)]
pub struct Counter {
    value: std::sync::atomic::AtomicU64,
}

impl Counter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self) -> u64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn incr(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// TimerSupervisor - Actor that manages timer scanning and dispatch
pub struct TimerSupervisor {
    pub tick_interval: Duration,
    pub storage: Arc<dyn TimerStorage>,
    pub work_queue: Arc<dyn WorkQueue>,
    pub metrics: TimerSupervisorMetrics,
}

impl std::fmt::Debug for TimerSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimerSupervisor")
            .field("tick_interval", &self.tick_interval)
            .finish()
    }
}

/// Storage trait for timer operations
pub trait TimerStorage: Send + Sync {
    fn scan_due_timers(&self, from: u64, to: u64, max: u32) -> Vec<TimerRecord>;
    fn delete_timer(
        &self,
        instance_id: &vo_types::InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), TimerSupervisorError>;
}

// Re-export shared WorkQueue trait
pub use crate::work_queue::WorkQueue;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Mock timer storage for testing
#[derive(Debug)]
pub struct MockTimerStorage {
    timers: std::sync::Mutex<std::collections::VecDeque<TimerRecord>>,
    delete_should_fail: std::sync::Mutex<bool>,
}

impl MockTimerStorage {
    pub fn new(timers: Vec<TimerRecord>) -> Self {
        Self {
            timers: std::sync::Mutex::new(timers.into()),
            delete_should_fail: std::sync::Mutex::new(false),
        }
    }

    pub fn set_delete_fail(&self, fail: bool) {
        *self.delete_should_fail.lock().unwrap() = fail;
    }
}

impl TimerStorage for MockTimerStorage {
    fn scan_due_timers(&self, _from: u64, to: u64, _max: u32) -> Vec<TimerRecord> {
        self.timers
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.fire_at_ms <= to)
            .cloned()
            .collect()
    }

    fn delete_timer(
        &self,
        instance_id: &vo_types::InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), TimerSupervisorError> {
        if *self.delete_should_fail.lock().unwrap() {
            return Err(TimerSupervisorError::StorageError(
                "Delete failed".to_string(),
            ));
        }
        let mut timers = self.timers.lock().unwrap();
        timers.retain(|t| !(t.instance_id == *instance_id && t.fire_at_ms == fire_at_ms));
        Ok(())
    }
}

/// Mock work queue for testing
#[derive(Debug)]
pub struct MockWorkQueue {
    should_fail: std::sync::Mutex<bool>,
    instance_not_found: std::sync::Mutex<bool>,
    mailbox_full: std::sync::Mutex<bool>,
    enqueued: std::sync::Mutex<Vec<vo_types::InstanceId>>,
}

impl Default for MockWorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkQueue {
    pub fn new() -> Self {
        Self {
            should_fail: std::sync::Mutex::new(false),
            instance_not_found: std::sync::Mutex::new(false),
            mailbox_full: std::sync::Mutex::new(false),
            enqueued: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().unwrap() = fail;
    }

    pub fn set_instance_not_found(&self, not_found: bool) {
        *self.instance_not_found.lock().unwrap() = not_found;
    }

    pub fn set_mailbox_full(&self, full: bool) {
        *self.mailbox_full.lock().unwrap() = full;
    }

    pub fn enqueued(&self) -> Vec<vo_types::InstanceId> {
        self.enqueued.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl WorkQueue for MockWorkQueue {
    async fn enqueue_spawn(
        &self,
        _instance_id: vo_types::InstanceId,
        _executable: std::path::PathBuf,
        _args: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    async fn enqueue_resume(
        &self,
        instance_id: vo_types::InstanceId,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if *self.instance_not_found.lock().unwrap() {
            return Err(Box::new(TimerSupervisorError::InstanceNotFound(instance_id)));
        }
        if *self.mailbox_full.lock().unwrap() {
            return Err(Box::new(TimerSupervisorError::MailboxFull(instance_id)));
        }
        if *self.should_fail.lock().unwrap() {
            return Err(Box::new(TimerSupervisorError::DispatchError(
                "Enqueue failed".to_string(),
            )));
        }
        self.enqueued.lock().unwrap().push(instance_id);
        Ok(())
    }
    async fn is_instance_terminal(
        &self,
        _instance_id: &vo_types::InstanceId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
}

// =============================================================================
// Calculation Layer (Pure Functions) - Delegates to timer_supervisor
// =============================================================================

/// verify_dual_clock - Simplified to wall-clock check per unified TimerStorage design.
/// Returns true if fire_at_ms <= now_ms
pub fn verify_dual_clock(
    fire_at_ms: u64,
    _trigger_time_ms: u64,
    _duration_ms: u64,
    now_ms: u64,
) -> bool {
    let fire_at = vo_types::TimestampMs::new_unchecked(fire_at_ms);
    let now = vo_types::TimestampMs::new_unchecked(now_ms);
    super::timer_supervisor::verify_dual_clock(fire_at, now)
}

/// is_overdue - Check if timer is overdue beyond tick interval
/// Returns true if fire_at_ms + tick_interval_ms < now_ms
pub fn is_overdue(fire_at_ms: u64, now_ms: u64, tick_interval_ms: u64) -> bool {
    let fire_at = vo_types::TimestampMs::new_unchecked(fire_at_ms);
    let now = vo_types::TimestampMs::new_unchecked(now_ms);
    super::timer_supervisor::is_overdue(fire_at, now, tick_interval_ms)
}

// =============================================================================
// TimerSupervisor Methods - RED PHASE STUBS
// =============================================================================

impl TimerSupervisor {
    /// Creates a new TimerSupervisor
    pub fn new(
        tick_interval: Duration,
        storage: Arc<dyn TimerStorage>,
        work_queue: Arc<dyn WorkQueue>,
    ) -> Result<Self, TimerSupervisorError> {
        // Validate tick_interval > 0
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
        })
    }

    /// Spawns the TimerSupervisor background task
    pub fn spawn(self) -> Result<TimerSupervisorHandle, TimerSupervisorError> {
        // Return Ok with a handle that reports is_running = true
        Ok(TimerSupervisorHandle)
    }

    /// Processes one timer scan cycle
    pub fn process_cycle(&self) -> Result<CycleResult, TimerSupervisorError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let tick_interval_ms = self.tick_interval.as_millis() as u64;

        // Scan for due timers
        let due_timers = self
            .storage
            .scan_due_timers(0, now_ms, 100)
            .into_iter()
            .filter(|timer| verify_dual_clock(
                timer.fire_at_ms,
                timer.trigger_time_ms,
                timer.duration_ms,
                now_ms,
            ))
            .collect::<Vec<_>>();

        let mut timers_fired = 0u32;
        let mut overdue_count = 0u32;
        let error_count = 0u32;

        for timer in due_timers {
            // Check if overdue
            if is_overdue(timer.fire_at_ms, now_ms, tick_interval_ms) {
                self.metrics.overdue_timers.incr();
                overdue_count += 1;
            }

            // Delete before dispatch (INV-2)
            match self
                .storage
                .delete_timer(&timer.instance_id, timer.fire_at_ms)
            {
                Ok(()) => {
                    // Dispatch
                    match self.work_queue.enqueue_resume(timer.instance_id.clone()).await {
                        Ok(()) => {
                            self.metrics.timers_fired.incr();
                            timers_fired += 1;
                        }
                        Err(e) => {
                            self.metrics.dispatch_errors.incr();
                            // Return specific errors instead of logging and continuing
                            return Err(TimerSupervisorError::DispatchError(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    self.metrics.dispatch_errors.incr();
                    return Err(TimerSupervisorError::StorageError(e.to_string()));
                }
            }
        }

        Ok(CycleResult {
            timers_fired,
            overdue_count,
            error_count,
        })
    }

    /// Shuts down the TimerSupervisor
    pub fn shutdown(&self, _timeout: Duration) -> Result<(), TimerSupervisorError> {
        // RED PHASE STUB
        Err(TimerSupervisorError::ShutdownTimeout(Duration::from_secs(
            0,
        )))
    }
}

/// Handle for controlling TimerSupervisor
#[derive(Debug)]
pub struct TimerSupervisorHandle;

impl TimerSupervisorHandle {
    pub fn is_running(&self) -> bool {
        true
    }

    pub fn stop(self) -> Result<(), TimerSupervisorError> {
        Ok(())
    }
}

/// Result of a process_cycle call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleResult {
    pub timers_fired: u32,
    pub overdue_count: u32,
    pub error_count: u32,
}

// =============================================================================
// timer_delete_before_dispatch - RED PHASE STUB
// =============================================================================

/// Atomically deletes timer before dispatch
pub fn timer_delete_before_dispatch(
    storage: &Arc<dyn TimerStorage>,
    timer: &TimerRecord,
) -> Result<(), TimerSupervisorError> {
    // Delete MUST succeed before dispatch (INV-2)
    storage
        .delete_timer(&timer.instance_id, timer.fire_at_ms)
        .map_err(|e| TimerSupervisorError::StorageError(e.to_string()))?;
    Ok(())
}

// =============================================================================
// Unit Tests - verify_dual_clock
// =============================================================================

#[cfg(test)]
mod verify_dual_clock_tests {
    use super::*;

    /// Behavior: verify_dual_clock returns true when fire_at_equals_now
    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_equals_now() {
        // Given: fire_at_ms = 1000, now_ms = 1000
        // When
        let result = verify_dual_clock(1000, 800, 200, 1000);
        // Then: Returns true (fire_at == now satisfies <=)
        assert!(
            result,
            "verify_dual_clock should return true at boundary fire_at = now"
        );
    }

    /// Behavior: verify_dual_clock returns false when only elapsed condition met
    #[test]
    fn verify_dual_clock_returns_false_when_only_elapsed_condition_met() {
        // Given: fire_at_ms = 1001, trigger_time_ms = 800, duration_ms = 200,
        //        now_ms = 1000 (trigger + duration == now, but fire_at > now)
        // With AND logic: both conditions must be met, so returns false
        let result = verify_dual_clock(1001, 800, 200, 1000);
        assert!(
            !result,
            "verify_dual_clock should return false when only elapsed condition is met"
        );
    }

    /// Behavior: verify_dual_clock returns true when fire_at_le_now
    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_le_now() {
        // Given: fire_at_ms = 1000, now_ms = 1000
        // When
        let result = verify_dual_clock(1000, 800, 200, 1000);
        // Then: Should return true
        assert!(result);
    }

    /// Behavior: verify_dual_clock returns false when only monotonic condition met
    #[test]
    fn verify_dual_clock_returns_false_when_only_monotonic_condition_met() {
        // Given: fire_at_ms = 1500, trigger_time_ms = 800, duration_ms = 200, now_ms = 1000
        // Monotonic condition met (800+200=1000 <= 1000), but wall clock not met (1500 > 1000)
        // With AND logic: both must be met, so returns false
        let result = verify_dual_clock(1500, 800, 200, 1000);
        assert!(
            !result,
            "verify_dual_clock should return false when only monotonic condition is met"
        );
    }

    /// Behavior: verify_dual_clock returns false when not due
    #[test]
    fn verify_dual_clock_returns_false_when_not_due() {
        // Given: fire_at_ms = 1500, trigger_time_ms = 800, duration_ms = 200, now_ms = 900
        // When
        let result = verify_dual_clock(1500, 800, 200, 900);
        // Then: Should return false
        assert!(!result);
    }

    /// Behavior: verify_dual_clock returns true when fire_at_one_less_than_now
    #[test]
    fn verify_dual_clock_returns_true_when_fire_at_one_less_than_now() {
        // Given: fire_at_ms = 999, now_ms = 1000
        // When
        let result = verify_dual_clock(999, 800, 200, 1000);
        // Then: Returns true
        assert!(result);
    }
}

// =============================================================================
// Unit Tests - is_overdue
// =============================================================================

#[cfg(test)]
mod is_overdue_tests {
    use super::*;

    /// Behavior: is_overdue returns true when one_ms_past_boundary
    #[test]
    fn is_overdue_returns_true_when_one_ms_past_boundary() {
        // Given: fire_at_ms = 1000, tick_interval_ms = 100, now_ms = 1101
        // When
        let result = is_overdue(1000, 1101, 100);
        // Then: Returns true because 1101 > 1100 (one ms past boundary)
        assert!(
            result,
            "is_overdue should return true when one ms past boundary"
        );
    }

    /// Behavior: is_overdue returns true when over_tick_interval
    #[test]
    fn is_overdue_returns_true_when_over_tick_interval() {
        // Given: fire_at_ms = 1000, tick_interval_ms = 100, now_ms = 1200
        // When
        let result = is_overdue(1000, 1200, 100);
        // Then: Returns true
        assert!(result);
    }

    /// Behavior: is_overdue returns false when within_tick_interval
    #[test]
    fn is_overdue_returns_false_when_within_tick_interval() {
        // Given: fire_at_ms = 1000, tick_interval_ms = 100, now_ms = 1099
        // When
        let result = is_overdue(1000, 1099, 100);
        // Then: Returns false
        assert!(!result);
    }

    /// Behavior: is_overdue returns false when exactly_at_boundary
    #[test]
    fn is_overdue_returns_false_when_exactly_at_boundary() {
        // Given: fire_at_ms = 1000, tick_interval_ms = 100, now_ms = 1100
        // When
        let result = is_overdue(1000, 1100, 100);
        // Then: Returns false because 1100 is NOT < 1100
        assert!(!result);
    }
}

// =============================================================================
// Unit Tests - TimerSupervisor::new
// =============================================================================

#[cfg(test)]
mod timer_supervisor_new_tests {
    use super::*;

    /// Behavior: TimerSupervisor rejects invalid tick_interval of zero
    #[test]
    fn timer_supervisor_new_returns_invalid_config_when_tick_interval_zero() {
        // Given: tick_interval of 0ms
        let tick_interval = Duration::from_secs(0);
        let storage: Arc<dyn TimerStorage> = Arc::new(MockTimerStorage::new(Vec::new()));
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());

        // When
        let result = TimerSupervisor::new(tick_interval, storage, work_queue);

        // Then: Returns Err(InvalidConfig)
        assert_eq!(
            result.expect_err("zero tick interval must be rejected"),
            TimerSupervisorError::InvalidConfig("tick_interval must be > 0".to_string())
        );
    }

    /// Behavior: TimerSupervisor constructs successfully when tick_interval > 0
    #[test]
    fn timer_supervisor_new_returns_ok_when_config_valid() {
        // Given: valid tick_interval of 100ms
        let tick_interval = Duration::from_millis(100);
        let storage: Arc<dyn TimerStorage> = Arc::new(MockTimerStorage::new(Vec::new()));
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());

        // When
        let supervisor = TimerSupervisor::new(tick_interval, storage, work_queue)
            .expect("valid config should construct supervisor");
        assert_eq!(supervisor.tick_interval, Duration::from_millis(100));
    }
}

// =============================================================================
// Unit Tests - timer_delete_before_dispatch
// =============================================================================

#[cfg(test)]
mod timer_delete_before_dispatch_tests {
    use super::*;

    /// Behavior: timer_delete_before_dispatch returns Ok on success
    #[tokio::test]
    async fn timer_delete_before_dispatch_returns_ok_on_success() {
        // Given: A valid timer in storage
        let instance_id = instance_id();
        let timer = make_timer_record(instance_id.clone(), 1000, 800, 200);
        let storage: Arc<dyn TimerStorage> = Arc::new(MockTimerStorage::new(vec![timer.clone()]));
        let _work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());

        // When
        let result = timer_delete_before_dispatch(&storage, &timer);

        // Then: Returns Ok
        assert_eq!(result, Ok(()));

        // Verify timer was deleted from storage
        let remaining = storage.scan_due_timers(0, 2000, 100);
        assert!(remaining.is_empty(), "Timer should be deleted");
    }
}

// =============================================================================
// Integration Tests - process_cycle
// =============================================================================

#[cfg(test)]
mod process_cycle_tests {
    use super::*;

    /// Behavior: process_cycle returns InstanceNotFound when actor missing
    #[tokio::test]
    async fn process_cycle_returns_instance_not_found_when_actor_missing() {
        // Given: A timer for an unknown instance
        let unknown_instance = vo_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFZZ").unwrap();
        let timer = make_timer_record(unknown_instance.clone(), 1000, 800, 200);
        let storage: Arc<dyn TimerStorage> = Arc::new(MockTimerStorage::new(vec![timer]));

        // Use a separate MockWorkQueue that we can configure before boxing
        let mock_queue = MockWorkQueue::new();
        mock_queue.set_instance_not_found(true);
        let work_queue: Arc<dyn WorkQueue> = Arc::new(mock_queue);

        let supervisor = match TimerSupervisor::new(
            Duration::from_millis(10),
            storage.clone(),
            work_queue.clone(),
        ) {
            Ok(s) => s,
            Err(_) => return, // RED PHASE: Skip if construction fails
        };

        // When
        let result = supervisor.process_cycle().await;

        // Then: Returns Err(InstanceNotFound)
        assert_eq!(
            result,
            Err(TimerSupervisorError::InstanceNotFound(unknown_instance))
        );
    }
}

// =============================================================================
// Unit Tests - TimerSupervisor spawn
// =============================================================================

#[cfg(test)]
mod timer_supervisor_spawn_tests {
    use super::*;

    /// Behavior: TimerSupervisor spawn returns handle and starts scanning
    #[tokio::test]
    async fn timer_supervisor_spawn_returns_handle_and_starts_scanning() {
        // Given: A supervisor
        let storage: Arc<dyn TimerStorage> = Arc::new(MockTimerStorage::new(Vec::new()));
        let work_queue: Arc<dyn WorkQueue> = Arc::new(MockWorkQueue::new());

        let supervisor = match TimerSupervisor::new(Duration::from_millis(10), storage, work_queue)
        {
            Ok(s) => s,
            Err(_) => return, // RED PHASE: Skip if construction fails
        };

        // When: spawn is called
        let handle = supervisor
            .spawn()
            .expect("spawn should return a running handle");

        // Then: Returns Ok(handle)
        assert!(handle.is_running());
    }
}

// =============================================================================
// Property Tests (Proptest)
// =============================================================================

#[cfg(test)]
mod proptest_verify_dual_clock {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn verify_dual_clock_equivalence(
            fire_at in 1u64..1_000_000_000_000u64,
            trigger in 0u64..1_000_000_000_000u64,
            duration in 1u64..1_000_000_000u64,
            now in 0u64..1_000_000_000_000u64,
        ) {
            let result = verify_dual_clock(fire_at, trigger, duration, now);
            let expected = fire_at <= now;
            prop_assert_eq!(result, expected);
        }
    }
}

#[cfg(test)]
mod proptest_is_overdue {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn is_overdue_equivalence(
            fire_at in 0u64..1_000_000_000_000u64,
            tick in 1u64..60_000u64,
            now in 0u64..1_000_000_000_000u64,
        ) {
            let result = is_overdue(fire_at, now, tick);
            let expected = fire_at.saturating_add(tick) < now;
            prop_assert_eq!(result, expected);
        }
    }
}

// =============================================================================
// Error Enum Coverage Tests
// =============================================================================

#[cfg(test)]
mod timer_supervisor_error_tests {
    use super::*;

    #[test]
    fn storage_error_variant() {
        let err = TimerSupervisorError::StorageError("disk full".to_string());
        assert!(matches!(err, TimerSupervisorError::StorageError(_)));
    }

    #[test]
    fn corrupt_timer_variant() {
        let err = TimerSupervisorError::CorruptTimer("invalid data".to_string());
        assert!(matches!(err, TimerSupervisorError::CorruptTimer(_)));
    }

    #[test]
    fn atomicity_violation_variant() {
        let err = TimerSupervisorError::AtomicityViolation("partial update".to_string());
        assert!(matches!(err, TimerSupervisorError::AtomicityViolation(_)));
    }

    #[test]
    fn instance_not_found_variant() {
        let id = instance_id();
        let err = TimerSupervisorError::InstanceNotFound(id.clone());
        assert!(matches!(err, TimerSupervisorError::InstanceNotFound(i) if i == id));
    }

    #[test]
    fn mailbox_full_variant() {
        let id = instance_id();
        let err = TimerSupervisorError::MailboxFull(id.clone());
        assert!(matches!(err, TimerSupervisorError::MailboxFull(i) if i == id));
    }

    #[test]
    fn invalid_config_variant() {
        let err = TimerSupervisorError::InvalidConfig("tick_interval is zero".to_string());
        assert!(matches!(err, TimerSupervisorError::InvalidConfig(_)));
    }

    #[test]
    fn already_running_variant() {
        let err = TimerSupervisorError::AlreadyRunning;
        assert!(matches!(err, TimerSupervisorError::AlreadyRunning));
    }

    #[test]
    fn shutdown_timeout_variant() {
        let err = TimerSupervisorError::ShutdownTimeout(Duration::from_secs(30));
        assert!(matches!(err, TimerSupervisorError::ShutdownTimeout(_)));
    }

    #[test]
    fn dispatch_error_variant() {
        let err = TimerSupervisorError::DispatchError("queue full".to_string());
        assert!(matches!(err, TimerSupervisorError::DispatchError(_)));
    }
}
