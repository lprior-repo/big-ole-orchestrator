//! Reanimator Loop - Timer-based workflow resumption service.
//!
//! Per ADR-005, the Reanimator Loop is a single background tokio task that:
//! - Every 1 second, performs a range scan on timers partition from 0 to current_timestamp
//! - For every timer key found, atomically records TimerFired and deletes the wake-up key
//! - Enqueues resume work for instance_id under fairness budget rules

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{broadcast, watch};
use tokio::time::{interval, MissedTickBehavior};
use vo_types::{InstanceId, TimestampMs};

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur in the Reanimator Loop.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReanimatorError {
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Corrupt key format: {0}")]
    CorruptKey(String),

    #[error("Atomicity violation: {0}")]
    AtomicityViolation(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(InstanceId),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("Failed to enqueue resume work: {0}")]
    EnqueueFailed(String),

    #[error("Reanimator is already running")]
    AlreadyRunning,

    #[error("Storage initialization failed: {0}")]
    StorageInitFailed(String),

    #[error("Failed to spawn reanimator task: {0}")]
    TaskSpawnFailed(String),

    #[error("Reanimator has already shut down")]
    AlreadyShutdown,

    #[error("Shutdown timed out after {0:?}")]
    ShutdownTimeout(Duration),
}

impl ReanimatorError {
    /// Returns true if this error indicates a transient failure that may succeed on retry.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::StorageError(_)
                | Self::EnqueueFailed(_)
                | Self::AtomicityViolation(_)
                | Self::BudgetExceeded(_)
        )
    }

    /// Returns true if this error indicates the operation should not be retried.
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::CorruptKey(_)
                | Self::InstanceNotFound(_)
                | Self::AlreadyRunning
                | Self::AlreadyShutdown
        )
    }
}

// =============================================================================
// Data Types
// =============================================================================

/// Configuration for the Reanimator Loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReanimatorConfig {
    /// Interval between timer scans.
    pub scan_interval: Duration,
    /// Maximum timers to process per scan cycle (fairness budget).
    pub max_timers_per_cycle: u32,
    /// Maximum concurrent resume operations.
    pub max_concurrent_resumes: u32,
    /// Shutdown timeout duration.
    pub shutdown_timeout: Duration,
}

impl Default for ReanimatorConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(1),
            max_timers_per_cycle: 100,
            max_concurrent_resumes: 10,
            shutdown_timeout: Duration::from_secs(30),
        }
    }
}

/// Represents a timer record from storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    /// The instance ID this timer belongs to.
    pub instance_id: InstanceId,
    /// When the timer should fire (Unix timestamp in milliseconds).
    pub fire_at_ms: TimestampMs,
    /// Optional timer ID for multiple timers per instance.
    pub timer_id: Option<vo_types::TimerId>,
    /// Metadata about when the timer was scheduled.
    pub scheduled_at_ms: TimestampMs,
}

impl TimerRecord {
    /// Creates a new TimerRecord.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        fire_at_ms: TimestampMs,
        timer_id: Option<vo_types::TimerId>,
        scheduled_at_ms: TimestampMs,
    ) -> Self {
        Self {
            instance_id,
            fire_at_ms,
            timer_id,
            scheduled_at_ms,
        }
    }
}

/// Result of a timer scan operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerScanResult {
    /// The timers that were found and due for firing.
    pub timers: Vec<TimerRecord>,
    /// The scan timestamp (current time when scan was performed).
    pub scanned_at_ms: TimestampMs,
    /// Number of timers skipped due to budget limits.
    pub skipped_count: u32,
}

impl TimerScanResult {
    /// Creates a new TimerScanResult.
    #[must_use]
    pub fn new(timers: Vec<TimerRecord>, scanned_at_ms: TimestampMs, skipped_count: u32) -> Self {
        Self {
            timers,
            scanned_at_ms,
            skipped_count,
        }
    }

    /// Returns true if no timers were found.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Returns the count of timers found.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.timers.len()
    }
}

/// Fairness budget for resume operations.
/// Ensures no single instance or workflow monopolizes the reanimator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FairnessBudget {
    /// Maximum resumes per instance per cycle.
    pub max_per_instance: u32,
    /// Maximum resumes per workflow per cycle.
    pub max_per_workflow: u32,
    /// Current count per instance (instance_id -> count).
    pub instance_counts: std::collections::HashMap<InstanceId, u32>,
}

impl FairnessBudget {
    /// Creates a new FairnessBudget with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new FairnessBudget with custom limits.
    #[must_use]
    pub fn with_limits(max_per_instance: u32, max_per_workflow: u32) -> Self {
        Self {
            max_per_instance,
            max_per_workflow,
            instance_counts: std::collections::HashMap::new(),
        }
    }

    /// Checks if an instance can be resumed under this budget.
    #[must_use]
    pub fn can_resume(&self, instance_id: &InstanceId) -> bool {
        self.instance_counts
            .get(instance_id)
            .map_or(true, |count| *count < self.max_per_instance)
    }

    /// Records a resume for an instance, returning true if allowed.
    /// Returns false if the budget is exhausted for this instance.
    #[must_use]
    pub fn record_resume(&mut self, instance_id: InstanceId) -> bool {
        if !self.can_resume(&instance_id) {
            return false;
        }
        *self.instance_counts.entry(instance_id).or_insert(0) += 1;
        true
    }

    /// Resets the budget for a new cycle.
    pub fn reset(&mut self) {
        self.instance_counts.clear();
    }
}

impl Default for FairnessBudget {
    fn default() -> Self {
        Self {
            max_per_instance: 5,
            max_per_workflow: 50,
            instance_counts: std::collections::HashMap::new(),
        }
    }
}

/// The state of the Reanimator Loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReanimatorState {
    /// The reanimator is stopped.
    Stopped,
    /// The reanimator is running.
    Running,
    /// The reanimator is shutting down.
    ShuttingDown,
    /// The reanimator has shut down.
    ShutDown,
}

impl ReanimatorState {
    /// Returns true if the reanimator is currently active (running or shutting down).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::ShuttingDown)
    }
}

// =============================================================================
// Storage Trait
// =============================================================================

/// Trait for timer storage operations.
/// Abstracts the underlying storage implementation (e.g., fjall, rocksdb).
#[async_trait::async_trait]
pub trait TimerStorage: Send + Sync {
    /// Scans for timers that are due (fire_at_ms <= current_time).
    async fn scan_due_timers(
        &self,
        from_timestamp: TimestampMs,
        to_timestamp: TimestampMs,
        max_results: u32,
    ) -> Result<Vec<TimerRecord>, ReanimatorError>;

    /// Deletes a timer by its key.
    async fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;

    /// Records that a timer has fired (appends to events partition).
    async fn record_timer_fired(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;
}

// =============================================================================
// Work Queue Trait
// =============================================================================

/// Trait for enqueuing resume work.
/// Abstracts the work queue implementation.
#[async_trait::async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a resume message for an instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ReanimatorError>;
}

// =============================================================================
// Calculation Layer (Pure Functions)
// =============================================================================

/// Processes due timers, applying fairness budget rules.
/// Returns the list of timers that can be resumed this cycle.
pub fn filter_timers_by_fairness(
    timers: Vec<TimerRecord>,
    budget: &FairnessBudget,
) -> (Vec<TimerRecord>, Vec<TimerRecord>) {
    let mut allowed = Vec::with_capacity(timers.len());
    let mut rejected = Vec::new();

    for timer in timers {
        if budget.can_resume(&timer.instance_id) {
            allowed.push(timer);
        } else {
            rejected.push(timer);
        }
    }

    (allowed, rejected)
}

/// Calculates the number of timers to process, respecting budget limits.
pub fn calculate_batch_size(
    remaining_timers: usize,
    max_per_cycle: u32,
    current_batch: usize,
) -> usize {
    let budget_remaining = max_per_cycle.saturating_sub(current_batch as u32) as usize;
    remaining_timers.min(budget_remaining)
}

/// Validates a timer record, returning an error if corrupt.
pub fn validate_timer_record(record: &TimerRecord) -> Result<(), ReanimatorError> {
    if record.fire_at_ms.as_u64() == 0 {
        return Err(ReanimatorError::CorruptKey(
            "Timer fire_at_ms is zero".to_string(),
        ));
    }
    if record.scheduled_at_ms.as_u64() == 0 {
        return Err(ReanimatorError::CorruptKey(
            "Timer scheduled_at_ms is zero".to_string(),
        ));
    }
    if record.fire_at_ms < record.scheduled_at_ms {
        return Err(ReanimatorError::CorruptKey(
            "Timer fire_at_ms is before scheduled_at_ms".to_string(),
        ));
    }
    Ok(())
}

/// Constructs a TimerScanResult for the given parameters.
/// This is a pure function for calculating scan results.
#[allow(dead_code)]
pub fn calculate_scan_result(
    timers: Vec<TimerRecord>,
    scanned_at_ms: TimestampMs,
    _max_timers: u32,
    skipped_count: u32,
) -> TimerScanResult {
    TimerScanResult::new(timers, scanned_at_ms, skipped_count)
}

/// Enqueues resume work for a single instance, respecting budget rules.
pub fn check_resume_budget(
    instance_id: &InstanceId,
    budget: &FairnessBudget,
) -> Result<(), ReanimatorError> {
    if !budget.can_resume(instance_id) {
        return Err(ReanimatorError::BudgetExceeded(format!(
            "Instance {} has exceeded resume budget",
            instance_id
        )));
    }
    Ok(())
}

// =============================================================================
// Actions Layer
// =============================================================================

/// Handle for controlling the Reanimator Loop.
#[derive(Debug)]
pub struct ReanimatorHandle {
    state_sender: watch::Sender<ReanimatorState>,
    shutdown_trigger: broadcast::Sender<()>,
}

impl ReanimatorHandle {
    /// Requests the reanimator to shut down.
    pub async fn shutdown(self) -> Result<(), ReanimatorError> {
        // Signal shutdown
        let _ = self.shutdown_trigger.send(());

        // Wait for state change to Shutdown
        let mut receiver = self.state_sender.subscribe();
        loop {
            match receiver.changed().await {
                Ok(()) => {
                    let state = (*receiver.borrow()).clone();
                    match state {
                        ReanimatorState::ShutDown => return Ok(()),
                        ReanimatorState::ShuttingDown => continue,
                        _ => {
                            return Err(ReanimatorError::AtomicityViolation(format!(
                                "Unexpected state during shutdown: {:?}",
                                state
                            )));
                        }
                    }
                }
                Err(_) => {
                    return Err(ReanimatorError::AlreadyShutdown);
                }
            }
        }
    }

    /// Gets the current state of the reanimator.
    #[must_use]
    pub fn current_state(&self) -> ReanimatorState {
        self.state_sender.borrow().clone()
    }
}

/// The Reanimator Loop background task.
pub struct ReanimatorLoop;

impl ReanimatorLoop {
    /// Spawns the Reanimator Loop as a background task.
    ///
    /// # Errors
    /// Returns `ReanimatorError::AlreadyRunning` if a reanimator is already running.
    pub fn spawn<S, Q>(
        config: ReanimatorConfig,
        storage: Arc<S>,
        work_queue: Arc<Q>,
    ) -> Result<ReanimatorHandle, ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        // Create channels for state and shutdown
        let (state_sender, _) = watch::channel(ReanimatorState::Stopped);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let handle = ReanimatorHandle {
            state_sender,
            shutdown_trigger: shutdown_trigger.clone(),
        };

        // Spawn the background task
        tokio::spawn(Self::run_loop(
            config,
            storage,
            work_queue,
            handle.state_sender.clone(),
            shutdown_trigger.subscribe(),
        ));

        Ok(handle)
    }

    /// The main loop implementation.
    async fn run_loop<S, Q>(
        config: ReanimatorConfig,
        storage: Arc<S>,
        work_queue: Arc<Q>,
        state_sender: watch::Sender<ReanimatorState>,
        mut shutdown_receiver: broadcast::Receiver<()>,
    ) where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        // Transition to Running
        let _ = state_sender.send(ReanimatorState::Running);

        let mut scan_interval = interval(config.scan_interval);
        scan_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut budget = FairnessBudget::with_limits(
            config.max_timers_per_cycle,
            config.max_timers_per_cycle * config.max_concurrent_resumes,
        );

        let mut max_already_processed = 0u32;

        loop {
            tokio::select! {
                _ = shutdown_receiver.recv() => {
                    let _ = state_sender.send(ReanimatorState::ShuttingDown);
                    break;
                }
                _ = scan_interval.tick() => {
                    // Perform one scan cycle
                    match Self::process_cycle(&config, &storage, &work_queue, &mut budget, max_already_processed).await {
                        Ok(processed) => {
                            max_already_processed = processed;
                        }
                        Err(e) if e.is_transient() => {
                            // Log and continue
                            tracing::warn!("Transient error in reanimator cycle: {}", e);
                        }
                        Err(e) if e.is_fatal() => {
                            // Log and continue but don't die
                            tracing::error!("Fatal error in reanimator cycle: {}", e);
                        }
                        Err(e) => {
                            tracing::error!("Unknown error in reanimator cycle: {}", e);
                        }
                    }
                }
            }
        }

        let _ = state_sender.send(ReanimatorState::ShutDown);
    }

    /// Processes a single scan cycle.
    async fn process_cycle<S, Q>(
        config: &ReanimatorConfig,
        storage: &Arc<S>,
        work_queue: &Arc<Q>,
        budget: &mut FairnessBudget,
        max_already_processed: u32,
    ) -> Result<u32, ReanimatorError>
    where
        S: TimerStorage + 'static,
        Q: WorkQueue + 'static,
    {
        let current_time = TimestampMs::now();

        // Scan for due timers
        let scan_result = storage
            .scan_due_timers(
                TimestampMs::try_from(0u64).expect("0 is valid TimestampMs"),
                current_time,
                config.max_timers_per_cycle,
            )
            .await?;

        // Reset budget for this cycle
        budget.reset();

        let mut processed = 0u32;
        let mut failed_count = 0u32;

        // Process each timer
        for timer in scan_result
            .iter()
            .take(config.max_timers_per_cycle as usize)
        {
            // Validate timer
            if let Err(e) = validate_timer_record(timer) {
                tracing::error!("Invalid timer record: {}", e);
                continue;
            }

            // Check budget
            if !budget.can_resume(&timer.instance_id) {
                tracing::debug!("Instance {} exceeded budget, skipping", timer.instance_id);
                continue;
            }

            // Atomically record TimerFired and delete timer key
            let record_result = storage
                .record_timer_fired(&timer.instance_id, timer.fire_at_ms)
                .await;

            match record_result {
                Ok(()) => {
                    let delete_result = storage
                        .delete_timer(&timer.instance_id, timer.fire_at_ms)
                        .await;

                    match delete_result {
                        Ok(()) => {
                            // Enqueue resume work
                            match work_queue.enqueue_resume(timer.instance_id.clone()).await {
                                Ok(()) => {
                                    let _ = budget.record_resume(timer.instance_id.clone());
                                    processed += 1;
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to enqueue resume for {}: {}",
                                        timer.instance_id,
                                        e
                                    );
                                    failed_count += 1;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to delete timer for {}: {}",
                                timer.instance_id,
                                e
                            );
                            failed_count += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to record TimerFired for {}: {}",
                        timer.instance_id,
                        e
                    );
                    failed_count += 1;
                }
            }
        }

        // Reset max_already_processed if we processed any timers
        let new_max = if processed > 0 {
            0
        } else {
            max_already_processed + processed
        };

        tracing::debug!(
            "Reanimator cycle complete: processed={}, failed={}",
            processed,
            failed_count
        );

        Ok(new_max)
    }
}

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

/// Mock timer storage for testing.
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A mock timer storage that stores timers in memory.
    #[derive(Debug)]
    pub struct MockTimerStorage {
        timers: Mutex<VecDeque<TimerRecord>>,
        fire_calls: Mutex<Vec<(InstanceId, TimestampMs)>>,
        delete_calls: Mutex<Vec<(InstanceId, TimestampMs)>>,
        should_fail: Mutex<bool>,
    }

    impl MockTimerStorage {
        /// Creates a new MockTimerStorage with the given initial timers.
        pub fn new(timers: Vec<TimerRecord>) -> Self {
            Self {
                timers: Mutex::new(timers.into()),
                fire_calls: Mutex::new(Vec::new()),
                delete_calls: Mutex::new(Vec::new()),
                should_fail: Mutex::new(false),
            }
        }

        /// Sets whether operations should fail.
        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        /// Gets the recorded fire calls.
        pub fn fire_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
            self.fire_calls.lock().unwrap().clone()
        }

        /// Gets the recorded delete calls.
        pub fn delete_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
            self.delete_calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl TimerStorage for MockTimerStorage {
        async fn scan_due_timers(
            &self,
            _from_timestamp: TimestampMs,
            to_timestamp: TimestampMs,
            max_results: u32,
        ) -> Result<Vec<TimerRecord>, ReanimatorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(ReanimatorError::StorageError("Mock failure".to_string()));
            }

            let timers = self.timers.lock().unwrap();
            let due: Vec<TimerRecord> = timers
                .iter()
                .filter(|t| t.fire_at_ms <= to_timestamp)
                .take(max_results as usize)
                .cloned()
                .collect();

            Ok(due)
        }

        async fn delete_timer(
            &self,
            instance_id: &InstanceId,
            fire_at_ms: TimestampMs,
        ) -> Result<(), ReanimatorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(ReanimatorError::StorageError("Mock failure".to_string()));
            }

            self.delete_calls
                .lock()
                .unwrap()
                .push((instance_id.clone(), fire_at_ms));

            let mut timers = self.timers.lock().unwrap();
            timers.retain(|t| !(t.instance_id == *instance_id && t.fire_at_ms == fire_at_ms));

            Ok(())
        }

        async fn record_timer_fired(
            &self,
            instance_id: &InstanceId,
            fire_at_ms: TimestampMs,
        ) -> Result<(), ReanimatorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(ReanimatorError::StorageError("Mock failure".to_string()));
            }

            self.fire_calls
                .lock()
                .unwrap()
                .push((instance_id.clone(), fire_at_ms));

            Ok(())
        }
    }

    /// Mock work queue for testing.
    #[derive(Debug)]
    pub struct MockWorkQueue {
        enqueued: Mutex<Vec<InstanceId>>,
        should_fail: Mutex<bool>,
    }

    impl MockWorkQueue {
        /// Creates a new MockWorkQueue.
        pub fn new() -> Self {
            Self {
                enqueued: Mutex::new(Vec::new()),
                should_fail: Mutex::new(false),
            }
        }

        /// Sets whether operations should fail.
        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        /// Gets the enqueued instance IDs.
        pub fn enqueued(&self) -> Vec<InstanceId> {
            self.enqueued.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl WorkQueue for MockWorkQueue {
        async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ReanimatorError> {
            if *self.should_fail.lock().unwrap() {
                return Err(ReanimatorError::EnqueueFailed("Mock failure".to_string()));
            }
            self.enqueued.lock().unwrap().push(instance_id);
            Ok(())
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create TimestampMs from u64 without unwrap in test code
    fn ts_ms(value: u64) -> TimestampMs {
        TimestampMs::try_from(value).expect("valid timestamp")
    }

    // =============================================================================
    // Error Type Tests
    // =============================================================================

    mod reanimator_error_tests {
        use super::*;

        #[test]
        fn storage_error_is_transient() {
            let err = ReanimatorError::StorageError("disk full".to_string());
            assert!(err.is_transient());
            assert!(!err.is_fatal());
        }

        #[test]
        fn corrupt_key_is_fatal() {
            let err = ReanimatorError::CorruptKey("invalid format".to_string());
            assert!(!err.is_transient());
            assert!(err.is_fatal());
        }

        #[test]
        fn atomicity_violation_is_transient() {
            let err = ReanimatorError::AtomicityViolation("partial update".to_string());
            assert!(err.is_transient());
            assert!(!err.is_fatal());
        }

        #[test]
        fn budget_exceeded_is_transient() {
            let err = ReanimatorError::BudgetExceeded("limit reached".to_string());
            assert!(err.is_transient());
            assert!(!err.is_fatal());
        }

        #[test]
        fn already_running_is_fatal() {
            let err = ReanimatorError::AlreadyRunning;
            assert!(!err.is_transient());
            assert!(err.is_fatal());
        }

        #[test]
        fn already_shutdown_is_fatal() {
            let err = ReanimatorError::AlreadyShutdown;
            assert!(!err.is_transient());
            assert!(err.is_fatal());
        }

        #[test]
        fn error_display_format() {
            let err = ReanimatorError::StorageError("test error".to_string());
            assert_eq!(format!("{}", err), "Storage error: test error");

            let err = ReanimatorError::CorruptKey("bad key".to_string());
            assert_eq!(format!("{}", err), "Corrupt key format: bad key");

            let err = ReanimatorError::ShutdownTimeout(Duration::from_secs(5));
            assert_eq!(format!("{}", err), "Shutdown timed out after 5s");
        }
    }

    // =============================================================================
    // TimerRecord Tests
    // =============================================================================

    mod timer_record_tests {
        use super::*;

        #[test]
        fn timer_record_constructor() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let fire_at = ts_ms(1000);
            let scheduled = ts_ms(500);
            let timer_id = vo_types::TimerId::parse("timer-1").ok();

            let record = TimerRecord::new(instance_id.clone(), fire_at, timer_id, scheduled);

            assert_eq!(record.instance_id, instance_id);
            assert_eq!(record.fire_at_ms, fire_at);
            assert_eq!(record.scheduled_at_ms, scheduled);
        }

        #[test]
        fn timer_record_without_timer_id() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let fire_at = ts_ms(1000);
            let scheduled = ts_ms(500);

            let record = TimerRecord::new(instance_id.clone(), fire_at, None, scheduled);

            assert_eq!(record.instance_id, instance_id);
            assert!(record.timer_id.is_none());
        }
    }

    // =============================================================================
    // TimerScanResult Tests
    // =============================================================================

    mod timer_scan_result_tests {
        use super::*;

        #[test]
        fn scan_result_empty() {
            let result = TimerScanResult::new(Vec::new(), ts_ms(1000), 0);
            assert!(result.is_empty());
            assert_eq!(result.len(), 0);
        }

        #[test]
        fn scan_result_with_timers() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let timers = vec![
                TimerRecord::new(instance_id.clone(), ts_ms(1000), None, ts_ms(500)),
                TimerRecord::new(instance_id, ts_ms(2000), None, ts_ms(1500)),
            ];
            let result = TimerScanResult::new(timers, ts_ms(3000), 5);

            assert!(!result.is_empty());
            assert_eq!(result.len(), 2);
            assert_eq!(result.skipped_count, 5);
        }
    }

    // =============================================================================
    // FairnessBudget Tests
    // =============================================================================

    mod fairness_budget_tests {
        use super::*;

        #[test]
        fn budget_default_limits() {
            let budget = FairnessBudget::default();
            assert_eq!(budget.max_per_instance, 5);
            assert!(budget.instance_counts.is_empty());
        }

        #[test]
        fn budget_custom_limits() {
            let budget = FairnessBudget::with_limits(10, 100);
            assert_eq!(budget.max_per_instance, 10);
            assert_eq!(budget.max_per_workflow, 100);
        }

        #[test]
        fn can_resume_allows_first_resume() {
            let budget = FairnessBudget::default();
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            assert!(budget.can_resume(&instance_id));
        }

        #[test]
        fn can_resume_blocks_after_limit() {
            let mut budget = FairnessBudget::with_limits(2, 100);
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

            assert!(budget.record_resume(instance_id.clone()));
            assert!(budget.record_resume(instance_id.clone()));
            assert!(!budget.can_resume(&instance_id));
        }

        #[test]
        fn reset_clears_counts() {
            let mut budget = FairnessBudget::with_limits(1, 100);
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

            assert!(budget.record_resume(instance_id.clone()));
            assert!(!budget.can_resume(&instance_id));

            budget.reset();
            assert!(budget.can_resume(&instance_id));
        }

        #[test]
        fn different_instances_have_separate_counts() {
            let mut budget = FairnessBudget::with_limits(1, 100);
            let instance1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let instance2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

            assert!(budget.record_resume(instance1.clone()));
            assert!(!budget.can_resume(&instance1));
            assert!(budget.can_resume(&instance2));
        }
    }

    // =============================================================================
    // ReanimatorState Tests
    // =============================================================================

    mod reanimator_state_tests {
        use super::*;

        #[test]
        fn stopped_is_not_active() {
            let state = ReanimatorState::Stopped;
            assert!(!state.is_active());
        }

        #[test]
        fn running_is_active() {
            let state = ReanimatorState::Running;
            assert!(state.is_active());
        }

        #[test]
        fn shutting_down_is_active() {
            let state = ReanimatorState::ShuttingDown;
            assert!(state.is_active());
        }

        #[test]
        fn shut_down_is_not_active() {
            let state = ReanimatorState::ShutDown;
            assert!(!state.is_active());
        }
    }

    // =============================================================================
    // ReanimatorConfig Tests
    // =============================================================================

    mod reanimator_config_tests {
        use super::*;

        #[test]
        fn default_config() {
            let config = ReanimatorConfig::default();
            assert_eq!(config.scan_interval, Duration::from_secs(1));
            assert_eq!(config.max_timers_per_cycle, 100);
            assert_eq!(config.max_concurrent_resumes, 10);
            assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
        }

        #[test]
        fn custom_config() {
            let config = ReanimatorConfig {
                scan_interval: Duration::from_millis(500),
                max_timers_per_cycle: 50,
                max_concurrent_resumes: 5,
                shutdown_timeout: Duration::from_secs(60),
            };
            assert_eq!(config.scan_interval, Duration::from_millis(500));
            assert_eq!(config.max_timers_per_cycle, 50);
            assert_eq!(config.max_concurrent_resumes, 5);
            assert_eq!(config.shutdown_timeout, Duration::from_secs(60));
        }
    }

    // =============================================================================
    // Calculation Layer Tests
    // =============================================================================

    mod calculation_tests {
        use super::*;

        #[test]
        fn filter_timers_by_fairness_allows_within_budget() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let timers = vec![TimerRecord::new(
                instance_id.clone(),
                ts_ms(1000),
                None,
                ts_ms(500),
            )];

            let budget = FairnessBudget::default();
            let (allowed, rejected) = filter_timers_by_fairness(timers.clone(), &budget);

            assert_eq!(allowed.len(), 1);
            assert_eq!(rejected.len(), 0);
        }

        #[test]
        fn filter_timers_by_fairness_rejects_over_budget() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let mut budget = FairnessBudget::with_limits(1, 100);

            // Exhaust budget
            let _ = budget.record_resume(instance_id.clone());

            let timers = vec![TimerRecord::new(
                instance_id.clone(),
                ts_ms(1000),
                None,
                ts_ms(500),
            )];

            let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

            assert_eq!(allowed.len(), 0);
            assert_eq!(rejected.len(), 1);
        }

        #[test]
        fn calculate_batch_size_respects_budget() {
            assert_eq!(calculate_batch_size(50, 100, 0), 50);
            assert_eq!(calculate_batch_size(50, 100, 30), 50);
            assert_eq!(calculate_batch_size(50, 100, 70), 30);
            assert_eq!(calculate_batch_size(50, 100, 100), 0);
            assert_eq!(calculate_batch_size(50, 100, 101), 0);
        }

        #[test]
        fn calculate_batch_size_respects_remaining() {
            assert_eq!(calculate_batch_size(10, 100, 0), 10);
            assert_eq!(calculate_batch_size(10, 100, 95), 5);
        }

        #[test]
        fn validate_timer_record_accepts_valid_record() {
            let record = TimerRecord::new(
                InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
                ts_ms(1000),
                None,
                ts_ms(500),
            );
            assert!(validate_timer_record(&record).is_ok());
        }

        #[test]
        fn validate_timer_record_rejects_zero_fire_at() {
            let record = TimerRecord::new(
                InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
                ts_ms(0),
                None,
                ts_ms(500),
            );
            let err = validate_timer_record(&record).unwrap_err();
            assert!(matches!(err, ReanimatorError::CorruptKey(_)));
        }

        #[test]
        fn validate_timer_record_rejects_zero_scheduled_at() {
            let record = TimerRecord::new(
                InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
                ts_ms(1000),
                None,
                ts_ms(0),
            );
            let err = validate_timer_record(&record).unwrap_err();
            assert!(matches!(err, ReanimatorError::CorruptKey(_)));
        }

        #[test]
        fn validate_timer_record_rejects_fire_before_scheduled() {
            let record = TimerRecord::new(
                InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
                ts_ms(500),
                None,
                ts_ms(1000),
            );
            let err = validate_timer_record(&record).unwrap_err();
            assert!(matches!(err, ReanimatorError::CorruptKey(_)));
        }

        #[test]
        fn check_resume_budget_success() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let budget = FairnessBudget::default();

            let result = check_resume_budget(&instance_id, &budget);
            assert!(result.is_ok());
        }

        #[test]
        fn check_resume_budget_fails_over_budget() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let mut budget = FairnessBudget::with_limits(1, 100);

            // Exhaust budget
            let _ = budget.record_resume(instance_id.clone());

            let result = check_resume_budget(&instance_id, &budget);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                ReanimatorError::BudgetExceeded(_)
            ));
        }
    }

    // =============================================================================
    // ReanimatorHandle Tests
    // =============================================================================

    mod reanimator_handle_tests {
        use super::*;

        #[test]
        fn handle_initial_state() {
            let (state_sender, _) = watch::channel(ReanimatorState::Stopped);
            let (shutdown_trigger, _) = broadcast::channel(1);

            let handle = ReanimatorHandle {
                state_sender,
                shutdown_trigger,
            };

            assert_eq!(handle.current_state(), ReanimatorState::Stopped);
        }
    }

    // =============================================================================
    // Mock Storage Integration Tests
    // =============================================================================

    mod mock_storage_tests {
        use super::mock::*;
        use super::*;

        #[tokio::test]
        async fn mock_storage_scan_returns_due_timers() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let timers = vec![TimerRecord::new(
                instance_id.clone(),
                ts_ms(1000),
                None,
                ts_ms(500),
            )];

            let storage = Arc::new(MockTimerStorage::new(timers));
            let result = storage.scan_due_timers(ts_ms(0), ts_ms(2000), 100).await;

            assert!(result.is_ok());
            let timers = result.unwrap();
            assert_eq!(timers.len(), 1);
        }

        #[tokio::test]
        async fn mock_storage_delete_removes_timer() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let timers = vec![TimerRecord::new(
                instance_id.clone(),
                ts_ms(1000),
                None,
                ts_ms(500),
            )];

            let storage = Arc::new(MockTimerStorage::new(timers));

            storage
                .delete_timer(&instance_id, ts_ms(1000))
                .await
                .unwrap();

            // Verify timer was removed
            let remaining = storage
                .scan_due_timers(ts_ms(0), ts_ms(2000), 100)
                .await
                .unwrap();

            assert!(remaining.is_empty());
        }

        #[tokio::test]
        async fn mock_storage_record_fire_tracks_call() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let storage = Arc::new(MockTimerStorage::new(Vec::new()));

            storage
                .record_timer_fired(&instance_id, ts_ms(1000))
                .await
                .unwrap();

            let calls = storage.fire_calls();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, instance_id);
            assert_eq!(calls[0].1, ts_ms(1000));
        }

        #[tokio::test]
        async fn mock_storage_failure_returns_error() {
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            let storage = Arc::new(MockTimerStorage::new(Vec::new()));
            storage.set_should_fail(true);

            let result = storage.record_timer_fired(&instance_id, ts_ms(1000)).await;

            assert!(result.is_err());
        }
    }

    // =============================================================================
    // Mock WorkQueue Tests
    // =============================================================================

    mod mock_work_queue_tests {
        use super::mock::*;
        use super::*;

        #[tokio::test]
        async fn mock_work_queue_enqueue() {
            let queue = Arc::new(MockWorkQueue::new());
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

            queue.enqueue_resume(instance_id.clone()).await.unwrap();

            let enqueued = queue.enqueued();
            assert_eq!(enqueued.len(), 1);
            assert_eq!(enqueued[0], instance_id);
        }

        #[tokio::test]
        async fn mock_work_queue_failure() {
            let queue = Arc::new(MockWorkQueue::new());
            let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
            queue.set_should_fail(true);

            let result = queue.enqueue_resume(instance_id.clone()).await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn mock_work_queue_multiple_enqueues() {
            let queue = Arc::new(MockWorkQueue::new());

            // InstanceId requires exactly 26 characters
            let ids = [
                "01H5JYV4XHGSR2F8KZ9BWNRFMA",
                "01H5JYV4XHGSR2F8KZ9BWNRFMB",
                "01H5JYV4XHGSR2F8KZ9BWNRFMC",
                "01H5JYV4XHGSR2F8KZ9BWNRFMD",
                "01H5JYV4XHGSR2F8KZ9BWNRFME",
            ];

            for id in &ids {
                let instance_id = InstanceId::parse(id).unwrap();
                queue.enqueue_resume(instance_id).await.unwrap();
            }

            assert_eq!(queue.enqueued().len(), 5);
        }
    }
}
