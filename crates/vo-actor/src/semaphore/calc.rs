//! Calculation Layer — Pure Decision Functions
//!
//! Contains pure functions for computing backpressure status and wait estimates.
//! These functions have no side effects and are suitable for testing.

use crate::semaphore::types::{BackpressureStatus, SemaphoreConfig};

/// Calculates the current backpressure status based on waiters and permits.
#[inline]
#[must_use]
pub fn calculate_backpressure_status(
    available_permits: usize,
    total_permits: usize,
    waiting_count: usize,
    max_waiters_for_shed: usize,
) -> BackpressureStatus {
    let usage_ratio = if total_permits > 0 {
        (total_permits - available_permits) as f64 / total_permits as f64
    } else {
        1.0
    };

    if waiting_count >= max_waiters_for_shed {
        BackpressureStatus::ShedLoad
    } else if waiting_count > total_permits / 2 || usage_ratio > 0.8 {
        BackpressureStatus::Heavy
    } else if usage_ratio > 0.5 || waiting_count > total_permits / 4 {
        BackpressureStatus::Moderate
    } else {
        BackpressureStatus::Healthy
    }
}

/// Estimates wait time in milliseconds based on position and available permits.
#[inline]
#[must_use]
pub fn estimate_wait_ms(
    position: usize,
    available_permits: usize,
    avg_task_duration_ms: u64,
) -> u64 {
    if available_permits == 0 {
        return (position as u64 + 1) * avg_task_duration_ms;
    }
    let ahead = position / available_permits;
    (ahead as u64 + 1) * avg_task_duration_ms
}

/// Determines if a workflow is saturated (too many pending operations).
#[inline]
#[must_use]
pub fn is_workflow_saturated(pending_count: usize, max_per_workflow: usize) -> bool {
    pending_count >= max_per_workflow
}

/// Calculates backpressure status from a SemaphoreConfig and atomic state.
#[inline]
#[must_use]
pub fn status_from_config_and_state(
    config: &SemaphoreConfig,
    available_permits: usize,
    waiting_count: usize,
) -> BackpressureStatus {
    calculate_backpressure_status(
        available_permits,
        config.max_concurrent_binaries,
        waiting_count,
        config.max_waiters_for_shed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_backpressure_status_healthy() {
        let status = calculate_backpressure_status(400, 500, 50, 5000);
        assert_eq!(status, BackpressureStatus::Healthy);
    }

    #[test]
    fn calculate_backpressure_status_heavy() {
        let status = calculate_backpressure_status(100, 500, 300, 5000);
        assert_eq!(status, BackpressureStatus::Heavy);
    }

    #[test]
    fn calculate_backpressure_status_shed_load() {
        let status = calculate_backpressure_status(0, 500, 5001, 5000);
        assert_eq!(status, BackpressureStatus::ShedLoad);
    }

    #[test]
    fn calculate_backpressure_status_moderate_by_waiting() {
        let status = calculate_backpressure_status(400, 500, 150, 5000);
        assert_eq!(status, BackpressureStatus::Moderate);
    }

    #[test]
    fn calculate_backpressure_status_moderate_by_usage() {
        let status = calculate_backpressure_status(200, 500, 50, 5000);
        assert_eq!(status, BackpressureStatus::Moderate);
    }

    #[test]
    fn estimate_wait_ms_calculation() {
        let wait = estimate_wait_ms(50, 10, 100);
        assert_eq!(wait, 600);
    }

    #[test]
    fn estimate_wait_ms_no_permits() {
        let wait = estimate_wait_ms(5, 0, 100);
        assert_eq!(wait, 600);
    }

    #[test]
    fn is_workflow_saturated_false() {
        assert!(!is_workflow_saturated(5, 10));
    }

    #[test]
    fn is_workflow_saturated_true() {
        assert!(is_workflow_saturated(10, 10));
        assert!(is_workflow_saturated(15, 10));
    }
}
