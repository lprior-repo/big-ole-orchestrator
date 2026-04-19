//! Data Layer — Configuration and Status Types
//!
//! Contains the data types for the semaphore system:
//! - `SemaphoreConfig`: Configuration for execution semaphore
//! - `BackpressureStatus`: System load state indicators
//! - `AdmissionDecision`: Result of admission control
//! - `RejectionReason`: Why a request was rejected

use std::time::Duration;

// =============================================================================
// Constants
// =============================================================================

pub const DEFAULT_MAX_CONCURRENT_BINARIES: usize = 500;
pub const DEFAULT_MAX_WAITERS_FOR_SHED: usize = 5000;
pub const DEFAULT_MAX_PER_WORKFLOW: usize = 10;

// =============================================================================
// Data Layer — Configuration and Status Types
// =============================================================================

/// Configuration for the execution semaphore system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemaphoreConfig {
    /// Maximum concurrent binary spawns (default: 500).
    pub max_concurrent_binaries: usize,
    /// Threshold for ingress load shedding (default: 5000).
    pub max_waiters_for_shed: usize,
    /// Maximum concurrent operations per workflow (default: 10).
    pub max_per_workflow: usize,
    /// Timeout for acquiring a permit (default: 30s).
    pub acquire_timeout: Duration,
    /// Reserved permits for recovery tasks (default: 50).
    pub reserved_permits: usize,
}

impl Default for SemaphoreConfig {
    fn default() -> Self {
        Self {
            max_concurrent_binaries: DEFAULT_MAX_CONCURRENT_BINARIES,
            max_waiters_for_shed: DEFAULT_MAX_WAITERS_FOR_SHED,
            max_per_workflow: DEFAULT_MAX_PER_WORKFLOW,
            acquire_timeout: Duration::from_secs(30),
            reserved_permits: 50,
        }
    }
}

/// Backpressure status indicating system load state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackpressureStatus {
    /// System is healthy, no backpressure.
    Healthy,
    /// System is experiencing moderate load.
    Moderate,
    /// System is under heavy load, queuing expected.
    Heavy,
    /// Load shedding active, rejecting new requests.
    ShedLoad,
}

impl BackpressureStatus {
    /// Returns true if new requests should be rejected.
    #[must_use]
    pub fn should_reject(&self) -> bool {
        matches!(self, Self::ShedLoad)
    }

    /// Returns true if waiters are being queued.
    #[must_use]
    pub fn is_queued(&self) -> bool {
        matches!(self, Self::Heavy | Self::ShedLoad)
    }
}

/// Result of an admission control decision.
#[derive(Debug, Clone)]
pub enum AdmissionDecision {
    /// Request admitted, semaphore permit acquired.
    Admitted,
    /// Request queued, waiting for permit.
    Queued {
        position: usize,
        estimated_wait_ms: u64,
    },
    /// Request rejected due to load shedding.
    Rejected {
        reason: RejectionReason,
        retry_after_secs: u32,
    },
}

impl PartialEq for AdmissionDecision {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Admitted, Self::Admitted) => true,
            (
                Self::Queued {
                    position: l_pos,
                    estimated_wait_ms: l_wait,
                },
                Self::Queued {
                    position: r_pos,
                    estimated_wait_ms: r_wait,
                },
            ) => l_pos == r_pos && l_wait == r_wait,
            (
                Self::Rejected {
                    reason: l_reason,
                    retry_after_secs: l_retry,
                },
                Self::Rejected {
                    reason: r_reason,
                    retry_after_secs: r_retry,
                },
            ) => l_reason == r_reason && l_retry == r_retry,
            _ => false,
        }
    }
}

impl Eq for AdmissionDecision {}

/// Reason for rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Load shedding is active.
    LoadShed,
    /// Workflow has too many pending operations.
    WorkflowSaturated,
    /// Timeout waiting for permit.
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semaphore_config_default_values() {
        let config = SemaphoreConfig::default();
        assert_eq!(
            config.max_concurrent_binaries,
            DEFAULT_MAX_CONCURRENT_BINARIES
        );
        assert_eq!(config.max_waiters_for_shed, DEFAULT_MAX_WAITERS_FOR_SHED);
        assert_eq!(config.max_per_workflow, DEFAULT_MAX_PER_WORKFLOW);
        assert_eq!(config.reserved_permits, 50);
    }

    #[test]
    fn backpressure_status_ordering() {
        assert!(BackpressureStatus::Healthy < BackpressureStatus::Moderate);
        assert!(BackpressureStatus::Moderate < BackpressureStatus::Heavy);
        assert!(BackpressureStatus::Heavy < BackpressureStatus::ShedLoad);
    }

    #[test]
    fn backpressure_status_should_reject() {
        assert!(!BackpressureStatus::Healthy.should_reject());
        assert!(!BackpressureStatus::Moderate.should_reject());
        assert!(!BackpressureStatus::Heavy.should_reject());
        assert!(BackpressureStatus::ShedLoad.should_reject());
    }

    #[test]
    fn backpressure_status_is_queued() {
        assert!(!BackpressureStatus::Healthy.is_queued());
        assert!(!BackpressureStatus::Moderate.is_queued());
        assert!(BackpressureStatus::Heavy.is_queued());
        assert!(BackpressureStatus::ShedLoad.is_queued());
    }
}
