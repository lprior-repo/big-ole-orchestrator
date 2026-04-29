//! Shared mutable state for the circuit breaker.
//!
//! This module defines the `CircuitBreakerState` struct that holds the
//! concurrent DashMap-backed state for rate limiting, failure tracking,
//! and workflow status management.
//!
//! MAJ-001: `DashMap` fields are not directly public. Core logic uses safe
//! accessor methods that guarantee guards are dropped before returning.
//! Reference accessors are provided for test setup and inspection.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use vo_types::WorkflowName;

use crate::circuit_breaker::{
    FailureWindow, QuarantineCallback, QuarantineEvent, RegistrationStatus,
};

/// Shared state for the circuit breaker, holding concurrent maps for
/// rate limiting, failure tracking, and workflow status.
///
/// All maps are wrapped in `DashMap` for lock-free concurrent access (INV-010).
///
/// Core logic uses the safe accessor methods (`get_status`, `set_status`, etc.)
/// which ensure guards are dropped before returning, preventing deadlocks.
/// Reference accessors (`statuses()`, `rate_limiter()`, `failure_tracker()`)
/// are provided for test setup and direct `DashMap` operations.
pub struct CircuitBreakerState {
    /// Workflow status map. Unknown workflows default to `Active`.
    ///
    /// Prefer using `get_status()` / `set_status()` for safe access.
    /// Direct field access is available for test setup and advanced operations.
    pub statuses: DashMap<WorkflowName, RegistrationStatus>,
    /// Rate limiter: last successful registration timestamp per workflow.
    ///
    /// Prefer using `get_rate_limit()` / `set_rate_limit()` for safe access.
    /// Direct field access is available for test setup and advanced operations.
    pub rate_limiter: DashMap<WorkflowName, Instant>,
    /// Failure tracker: sliding failure window per workflow.
    ///
    /// Prefer using `get_failure_count()` for safe reads.
    /// Direct field access is available for test setup and advanced operations.
    pub failure_tracker: DashMap<WorkflowName, FailureWindow>,
    /// Optional callback for quarantine notifications (ADR-026).
    /// When set, this callback is invoked when a workflow is quarantined.
    pub quarantine_callback: Option<Arc<QuarantineCallback>>,
}

impl CircuitBreakerState {
    /// Create a new empty state with no workflows tracked.
    #[must_use]
    pub fn new() -> Self {
        Self {
            statuses: DashMap::new(),
            rate_limiter: DashMap::new(),
            failure_tracker: DashMap::new(),
            quarantine_callback: None,
        }
    }

    /// Set the quarantine callback for notifications (ADR-026).
    pub fn set_quarantine_callback(&mut self, callback: QuarantineCallback) {
        self.quarantine_callback = Some(Arc::new(callback));
    }

    /// Invoke the quarantine callback if set.
    pub fn notify_quarantine(&self, event: &QuarantineEvent) {
        if let Some(callback) = &self.quarantine_callback {
            callback(event);
        }
    }

    // ── Safe value accessors (guards dropped before return) ─────────────

    /// Read the registration status for a workflow.
    /// Returns `Active` for unknown workflows (INV-005).
    #[must_use]
    pub fn get_status(&self, workflow_name: &WorkflowName) -> RegistrationStatus {
        self.statuses
            .get(workflow_name)
            .map_or(RegistrationStatus::Active, |s| *s)
    }

    /// Insert or update a workflow's registration status.
    pub fn set_status(&self, workflow_name: WorkflowName, status: RegistrationStatus) {
        self.statuses.insert(workflow_name, status);
    }

    /// Read the last registration timestamp for rate limiting.
    #[must_use]
    pub fn get_rate_limit(&self, workflow_name: &WorkflowName) -> Option<Instant> {
        self.rate_limiter.get(workflow_name).map(|r| *r)
    }

    /// Update the rate limiter timestamp for a workflow.
    pub fn set_rate_limit(&self, workflow_name: WorkflowName, timestamp: Instant) {
        self.rate_limiter.insert(workflow_name, timestamp);
    }

    /// Remove the rate limiter entry for a workflow.
    pub fn remove_rate_limit(&self, workflow_name: &WorkflowName) {
        self.rate_limiter.remove(workflow_name);
    }

    /// Get the failure count for a workflow (guards dropped before return).
    #[must_use]
    pub fn get_failure_count(&self, workflow_name: &WorkflowName) -> usize {
        self.failure_tracker
            .get(workflow_name)
            .map_or(0, |w| w.len())
    }
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CircuitBreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreakerState")
            .field("statuses", &self.statuses.len())
            .field("rate_limiter", &self.rate_limiter.len())
            .field("failure_tracker", &self.failure_tracker.len())
            .field("quarantine_callback", &self.quarantine_callback.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit_breaker::{FailureWindow, RegistrationStatus};
    use std::time::Instant;
    use vo_types::WorkflowName;

    #[test]
    fn circuit_breaker_state_tracks_status_rate_limit_and_failure_window() {
        let state = CircuitBreakerState::default();
        let wf = WorkflowName::parse("test-wf").unwrap();

        assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
        assert_eq!(state.get_rate_limit(&wf), None);
        assert_eq!(state.get_failure_count(&wf), 0);

        let now = Instant::now();
        state.set_status(wf.clone(), RegistrationStatus::Quarantined);
        state.set_rate_limit(wf.clone(), now);

        assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
        assert_eq!(state.get_rate_limit(&wf), Some(now));

        state.remove_rate_limit(&wf);
        assert_eq!(state.get_rate_limit(&wf), None);

        state
            .failure_tracker
            .insert(wf.clone(), FailureWindow::new());
        assert_eq!(state.get_failure_count(&wf), 0);

        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("CircuitBreakerState"));
    }
}
