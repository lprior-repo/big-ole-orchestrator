//! Timer supervisor types
//!
//! Contains all type definitions for the timer supervisor module.

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
            Self::AlreadyRunning | Self::ShutdownTimeout(_) | Self::AtomicityViolation(_)
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
    /// Number of times a timer was deleted but dispatch failed (recovered via retry).
    pub timer_deleted_but_dispatch_failed: Counter,
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
