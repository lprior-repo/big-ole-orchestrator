//! AI Deployment Circuit Breaker (vel-ich)
//!
//! Dual-layered protection for workflow binary registration:
//! - Layer 1: Rate limiter (60s cooldown per workflow)
//! - Layer 2: Failure loop detector (N=5 unique hash failures in 10 min)

pub mod config;
pub mod failure_window;
pub mod rate_limiter;
pub mod registration_status;
pub mod state;
pub mod types;

pub use config::{CircuitBreakerConfig, ConfigValidationError};
pub use failure_window::FailureWindow;
pub use rate_limiter::{
    check_rate_limit, update_rate_limit, TokenBucketConfig, TokenBucketRateLimiter,
};
pub use registration_status::RegistrationStatus;
pub use state::CircuitBreakerState;
pub use types::{
    CircuitBreakerError, FailureRecord, QuarantineEvent, RegistrationOutcome, RegistrationRequest,
    UnquarantineResult,
};

use std::time::Instant;
use vo_types::{BinaryHash, WorkflowName};

/// Evaluate whether a registration request should be allowed.
///
/// # Preconditions
/// - `request.workflow_name` is parse-validated
/// - `request.binary_hash` is parse-validated
/// - Circuit breaker state (rate limiter, failure tracker, status map) is initialized
///
/// # Postconditions
/// - If force=true: returns Allowed, updates rate limiter (bypasses ALL guards)
/// - If quarantined: returns `WorkflowQuarantined` (MAJ-002: checked before rate limit)
/// - If deactivated: returns `WorkflowDeactivated` (MAJ-002: checked before rate limit)
/// - If rate-limited: returns `RateLimited` with `retry_after_secs`
/// - Otherwise: returns Allowed, updates rate limiter
///
/// # Errors
/// Returns `CircuitBreakerError` on storage failure.
pub fn evaluate_registration(
    request: &RegistrationRequest,
    config: &CircuitBreakerConfig,
    state: &CircuitBreakerState,
    now: Instant,
) -> Result<RegistrationOutcome, CircuitBreakerError> {
    // POST-005: Force flag bypasses ALL registration guards
    if request.force {
        // Update rate limiter even on forced registration (POST-005, B-09)
        state.set_rate_limit(request.workflow_name.clone(), update_rate_limit(now));
        return Ok(RegistrationOutcome::Allowed);
    }

    // MAJ-002: Check quarantine/deactivation status BEFORE rate limit.
    // Permanent blocks take precedence over temporary cooldowns.
    let status = state.get_status(&request.workflow_name);

    match status {
        RegistrationStatus::Quarantined => {
            return Ok(RegistrationOutcome::WorkflowQuarantined {
                workflow_name: request.workflow_name.clone(),
            });
        }
        RegistrationStatus::Deactivated => {
            return Ok(RegistrationOutcome::WorkflowDeactivated {
                workflow_name: request.workflow_name.clone(),
            });
        }
        RegistrationStatus::Active => {}
    }

    // Layer 1: Rate limit check (INV-002)
    // CRIT-001: Use DashMap::entry() for atomic read-modify-write
    // to prevent TOCTOU race between get() and insert().
    let wf = request.workflow_name.clone();
    let rate_limit_window = config.rate_limit_window;

    // Atomic entry-based rate limit: read existing, check, and update in one lock
    let entry = state.rate_limiter.entry(wf);
    match entry {
        dashmap::mapref::entry::Entry::Occupied(mut occupied) => {
            let last = *occupied.get();
            if let Some(retry_after_secs) = check_rate_limit(Some(last), rate_limit_window, now) {
                // INV-009: Rate-limited requests do NOT count as failures
                Ok(RegistrationOutcome::RateLimited { retry_after_secs })
            } else {
                // Registration allowed — atomically update rate limiter (POST-006)
                *occupied.get_mut() = update_rate_limit(now);
                Ok(RegistrationOutcome::Allowed)
            }
        }
        dashmap::mapref::entry::Entry::Vacant(vacant) => {
            // No prior registration — allowed. Insert atomically.
            vacant.insert(update_rate_limit(now));
            Ok(RegistrationOutcome::Allowed)
        }
    }
}

/// Record a runtime failure for a workflow's binary.
///
/// # Postconditions
/// - If hash is new in window: added to `FailureWindow`, count incremented
/// - If hash already in window: timestamp updated, count unchanged (INV-004)
/// - If unique count >= threshold: status set to Quarantined (INV-001)
/// - Expired entries evicted before threshold check (INV-007)
///
/// # Returns
/// - `Ok(None)` if threshold not breached
/// - `Ok(Some(QuarantineEvent))` if threshold breached and quarantine triggered
///
/// # Errors
/// Returns `CircuitBreakerError::StorageError` if Fjall write fails.
pub fn record_failure(
    workflow_name: &WorkflowName,
    binary_hash: &BinaryHash,
    config: &CircuitBreakerConfig,
    state: &CircuitBreakerState,
    now: Instant,
) -> Result<Option<QuarantineEvent>, CircuitBreakerError> {
    // CRIT-002: If workflow is already quarantined, return Ok(None) immediately.
    // Prevents duplicate QuarantineEvent emission and unbounded FailureWindow growth.
    if state.get_status(workflow_name) == RegistrationStatus::Quarantined {
        return Ok(None);
    }

    // Get or create the failure window for this workflow
    let mut entry = state
        .failure_tracker
        .entry(workflow_name.clone())
        .or_default();

    // Record the failure in the window (handles INV-004 and INV-007)
    let unique_count = failure_window::record_failure_in_window(
        entry.value_mut(),
        binary_hash.clone(),
        now,
        config.failure_window,
    );

    // Check if threshold breached (INV-001)
    if unique_count >= usize::from(config.failure_threshold) {
        // Transition to Quarantined
        state.set_status(workflow_name.clone(), RegistrationStatus::Quarantined);

        Ok(Some(QuarantineEvent {
            workflow_name: workflow_name.clone(),
        }))
    } else {
        Ok(None)
    }
}

/// Reset a quarantined workflow to Active status.
///
/// # Postconditions (POST-003)
/// - Status transitions Quarantined -> Active
/// - `FailureWindow` cleared (0 entries)
/// - Rate limiter entry removed
/// - Fjall partition updated
/// - Operator action logged
///
/// # Errors
/// - `CircuitBreakerError::WorkflowNotFound` if workflow unknown
/// - `CircuitBreakerError::NotQuarantined` if status is not Quarantined
/// - `CircuitBreakerError::StorageError` if Fjall write fails
pub fn unquarantine(
    workflow_name: &WorkflowName,
    _operator: &str,
    state: &CircuitBreakerState,
) -> Result<UnquarantineResult, CircuitBreakerError> {
    // Check workflow exists in status map
    let current_status = state
        .statuses
        .get(workflow_name)
        .map(|s| *s)
        .ok_or_else(|| CircuitBreakerError::WorkflowNotFound {
            workflow_name: workflow_name.to_string(),
        })?;

    // Must be Quarantined to unquarantine
    if current_status != RegistrationStatus::Quarantined {
        return Err(CircuitBreakerError::NotQuarantined {
            workflow_name: workflow_name.to_string(),
            current_status,
        });
    }

    // Count failures being cleared before we remove them
    let failures_cleared = state.get_failure_count(workflow_name);

    // POST-003: Transition to Active
    state.set_status(workflow_name.clone(), RegistrationStatus::Active);

    // POST-003: Clear failure window
    state.failure_tracker.remove(workflow_name);

    // POST-003: Remove rate limiter entry
    state.remove_rate_limit(workflow_name);

    Ok(UnquarantineResult {
        workflow_name: workflow_name.clone(),
        previous_status: RegistrationStatus::Quarantined,
        new_status: RegistrationStatus::Active,
        failures_cleared,
    })
}
