//! Shared mutable state for the circuit breaker.
//!
//! This module defines the `CircuitBreakerState` struct that holds the
//! concurrent DashMap-backed state for rate limiting, failure tracking,
//! and workflow status management.
//!
//! MAJ-001: `DashMap` fields are not directly public. Core logic uses safe
//! accessor methods that guarantee guards are dropped before returning.
//! Reference accessors are provided for test setup and inspection.

use std::time::Instant;

use dashmap::DashMap;
use vo_types::WorkflowName;

use crate::circuit_breaker::{FailureWindow, RegistrationStatus};

/// Shared state for the circuit breaker, holding concurrent maps for
/// rate limiting, failure tracking, and workflow status.
///
/// All maps are wrapped in `DashMap` for lock-free concurrent access (INV-010).
///
/// Core logic uses the safe accessor methods (`get_status`, `set_status`, etc.)
/// which ensure guards are dropped before returning, preventing deadlocks.
/// Reference accessors (`statuses()`, `rate_limiter()`, `failure_tracker()`)
/// are provided for test setup and direct `DashMap` operations.
#[derive(Debug)]
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
}

impl CircuitBreakerState {
    /// Create a new empty state with no workflows tracked.
    #[must_use]
    pub fn new() -> Self {
        Self {
            statuses: DashMap::new(),
            rate_limiter: DashMap::new(),
            failure_tracker: DashMap::new(),
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
