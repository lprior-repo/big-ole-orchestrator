//! Data types for the Reanimator Loop.

use std::time::Duration;

use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::ReanimatorError;

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
        if self.max_per_instance == 0 {
            return false;
        }
        self.instance_counts
            .get(instance_id)
            .is_none_or(|count| *count < self.max_per_instance)
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
// Pure calculation functions
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
