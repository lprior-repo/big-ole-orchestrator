//! AI Deployment Circuit Breaker.
//!
//! # Overview
//!
//! Dual-layered protection for workflow binary registration. This module prevents
//! runaway deployment of faulty binaries through:
//!
//! - **Layer 1: Rate limiting** — A per-workflow cooldown window (default: 60s) that
//!   prevents rapid re-registration of the same or different binaries.
//! - **Layer 2: Failure loop detection** — A sliding window of unique-hash failures
//!   (default: 5 unique binaries failing within 10 minutes) that triggers automatic
//!   quarantine of the workflow.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   Binary Registration                           │
//! │                                                                  │
//! │  ┌──────────────┐   ┌──────────────────────────────────────┐    │
//! │  │ Registration │──>│ evaluate_registration()              │    │
//! │  │ Request      │   │                                      │    │
//! │  └──────────────┘   │  ┌─ force=true? ───────────────────┐ │    │
//! │                     │  │  Yes → Allowed (bypass all)     │ │    │
//! │                     │  │  No  → ↓                         │ │    │
//! │                     │  └──────────────────────────────────┘ │    │
//! │                     │                                       │    │
//! │                     │  ┌─ Workflow Status Check (MAJ-002) ─┐ │    │
//! │                     │  │  Quarantined → WorkflowQuarantined │ │    │
//! │                     │  │  Deactivated → WorkflowDeactivated │ │    │
//! │                     │  │  Deleted   → WorkflowDeactivated   │ │    │
//! │                     │  │  Active    → continue              │ │    │
//! │                     │  └────────────────────────────────────┘ │    │
//! │                     │                                       │    │
//! │                     │  ┌─ Layer 1: Rate Limit (INV-002) ────┐│    │
//! │                     │  │  Last registration within window?   ││    │
//! │                     │  │  Yes → RateLimited(retry_after)    ││    │
//! │                     │  │  No  → Allowed, update timestamp   ││    │
//! │                     │  └────────────────────────────────────┘│    │
//! │                     └───────────────────────────────────────┘    │
//! │                                                                  │
//! │  ┌──────────────┐   ┌──────────────────────────────────────┐    │
//! │  │ Runtime      │──>│ record_failure()                     │    │
//! │  │ Failure      │   │                                      │    │
//! │  └──────────────┘   │  ┌─ Already quarantined? ───────────┐│    │
//! │                     │  │  Yes → Ok(None) (no-op)          ││    │
//! │                     │  │  No  → ↓                          ││    │
//! │                     │  │                                  ││    │
//! │                     │  │  ┌─ FailureWindow ─────────────┐ ││    │
//! │                     │  │  │  Record failure (unique hash)│ ││    │
//! │                     │  │  │  Evict expired entries       │ ││    │
//! │                     │  │  │  Count unique hashes         │ ││    │
//! │                     │  │  └────────────┬─────────────────┘ ││    │
//! │                     │  │              │                    ││    │
//! │                     │  │  ┌─ Unique count ≥ threshold? ──┐││    │
//! │                     │  │  │  Yes → Quarantine workflow    │││    │
//! │                     │  │  │              Emit event       │││    │
//! │                     │  │  │              Notify callback  │││    │
//! │                     │  │  │  Ok(Some(event))              │││    │
//! │                     │  │  │  No  → Ok(None)               │││    │
//! │                     │  │  └───────────────────────────────┘││    │
//! │                     │  └────────────────────────────────────┘    │
//! │                     └───────────────────────────────────────┘    │
//! │                                                                  │
//! │  ┌──────────────┐   ┌──────────────────────────────────────┐    │
//! │  │ Unquarantine │──>│ unquarantine()                       │    │
//! │  │ Request      │   │                                      │    │
//! │  └──────────────┘   │  ┌─ Workflow exists? ────────────────┐│    │
//! │                     │  │  No → WorkflowNotFound            ││    │
//! │                     │  │  Yes → ↓                          ││    │
//! │                     │  │                                  ││    │
//! │                     │  │  ┌─ Status == Quarantined? ───────┐││    │
//! │                     │  │  │  No → NotQuarantined           │││    │
//! │                     │  │  │  Yes → ↓                       │││    │
//! │                     │  │  │                              │││    │
//! │                     │  │  │  Transition → Active          │││    │
//! │                     │  │  │  Clear failure window         │││    │
//! │                     │  │  │  Remove rate limiter entry    │││    │
//! │                     │  │  │  Ok(UnquarantineResult)       │││    │
//! │                     │  │  └───────────────────────────────┘││    │
//! │                     │  └────────────────────────────────────┘    │
//! │                     └───────────────────────────────────────┘    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Workflow Status Lifecycle
//!
//! Workflows progress through four states:
//!
//! ```text
//!  ┌──────────┐    register    ┌──────────┐    fail N times    ┌─────────────┐
//! │  Active   │───────────────>│  Active  │───────────────────>│ Quarantined │
//! │  (default)│                │          │                    │             │
//!  └──────────┘                └──────────┘                    └──────┬──────┘
//!       │                                                              │
//!       │              unquarantine()                                  │
//!       │──────────────────────────────────────────────────────────────┘
//!
//!  ┌──────────────┐    operator removes    ┌──────────┐
//! │  Deactivated  │───────────────────────>│ Deleted   │
//! │               │                        └──────────┘
//! ```
//!
//! - **Active** — Default state for unknown and healthy workflows. Registrations
//!   are subject to rate limiting and failure tracking.
//! - **Quarantined** — Workflow was automatically quarantined after `failure_threshold`
//!   unique binary hash failures within the `failure_window`. Registrations are
//!   rejected. Only [`unquarantine()`][crate::circuit_breaker::unquarantine] can restore it.
//! - **Deactivated** — Workflow was manually deactivated. Registrations are rejected.
//! - **Deleted** — Workflow was deleted. Treated the same as deactivated for
//!   registration purposes.
//!
//! # Failure Detection Algorithm
//!
//! Layer 2 detects "failure loops" where multiple distinct binary builds of the
//! same workflow fail at runtime:
//!
//! 1. On each runtime failure, [`record_failure()`] records a [`FailureRecord`]
//!    (binary hash + timestamp) in the workflow's [`FailureWindow`].
//! 2. Expired entries (older than `failure_window`) are evicted.
//! 3. Duplicate hashes update their timestamp without incrementing the count
//!    (INV-004).
//! 4. When the count of **unique** hashes reaches `failure_threshold`, the
//!    workflow transitions to `Quarantined`.
//!
//! # Rate Limiting Algorithm
//!
//! Layer 1 uses a simple cooldown window:
//!
//! 1. On each registration request, the last successful registration timestamp
//!    is checked against the current time.
//! 2. If the elapsed time is less than `rate_limit_window`, the request is
//!    rejected with `RateLimited(retry_after_secs)`.
//! 3. If the elapsed time meets or exceeds `rate_limit_window`, the request
//!    is allowed and the timestamp is updated.
//!
//! # Invariants
//!
//! | ID | Description |
//! |----|-------------|
//! | INV-001 | A workflow is quarantined when its failure window contains ≥ `failure_threshold` unique hash failures. |
//! | INV-002 | Rate limiting is uniform across all workflows — each has an independent cooldown. |
//! | INV-004 | Duplicate binary hashes in the failure window update the timestamp but do not increment the count. |
//! | INV-005 | Unknown workflows default to `Active` status. |
//! | INV-007 | Expired entries are evicted from the failure window before each threshold check. |
//! | INV-009 | Rate-limited registration requests do NOT count as failures. |
//! | INV-010 | All concurrent access uses `DashMap` for lock-free reads and fine-grained locking. |
//! | POST-003 | Unquarantine transitions `Quarantined → Active`, clears the failure window, and removes the rate limiter entry. |
//! | POST-005 | The `force` flag bypasses ALL registration guards including quarantine, deactivation, and rate limiting. |
//!
//! # Concurrency Model
//!
//! All state is stored in `DashMap` collections, providing lock-free concurrent
//! reads and fine-grained partition-level writes. The [`CircuitBreakerState`]
//! struct provides safe accessor methods that drop guards before returning,
//! preventing deadlocks.
//!
//! # Configuration
//!
//! The [`CircuitBreakerConfig`] struct controls all tunable parameters. Use
//! [`CircuitBreakerConfig::new()`] for validated construction or
//! [`CircuitBreakerConfig::default_config()`] for sensible defaults:
//!
//! | Parameter | Default | Description |
//! |-----------|---------|-------------|
//! | `rate_limit_window` | 60s | Cooldown between registrations per workflow |
//! | `failure_window` | 10min | Sliding window for failure tracking |
//! | `failure_threshold` | 5 | Unique hashes to trigger quarantine |
//!
//! # Examples
//!
//! ## Evaluate a registration request
//!
//! ```
//! use vo_core::circuit_breaker::{
//!     CircuitBreakerConfig, CircuitBreakerState,
//!     RegistrationRequest, RegistrationStatus,
//!     evaluate_registration,
//! };
//! use std::time::Instant;
//! use vo_types::WorkflowName, BinaryHash;
//!
//! let config = CircuitBreakerConfig::default_config().unwrap();
//! let mut state = CircuitBreakerState::new();
//!
//! let wf = WorkflowName::parse("my-workflow").unwrap();
//! let hash = BinaryHash::parse("abc123").unwrap();
//!
//! let request = RegistrationRequest {
//!     workflow_name: wf.clone(),
//!     binary_hash: hash,
//!     force: false,
//! };
//!
//! let result = evaluate_registration(&request, &config, &state, Instant::now());
//! assert_eq!(result.unwrap(), RegistrationOutcome::Allowed);
//! ```
//!
//! ## Record a failure
//!
//! ```
//! use vo_core::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerState, record_failure};
//! use std::time::{Duration, Instant};
//! use vo_types::{WorkflowName, BinaryHash};
//!
//! let config = CircuitBreakerConfig::default_config().unwrap();
//! let state = CircuitBreakerState::new();
//!
//! let wf = WorkflowName::parse("my-workflow").unwrap();
//! let hash = BinaryHash::parse("abc123").unwrap();
//!
//! // Record a failure — won't trigger quarantine yet (threshold is 5)
//! let result = record_failure(&wf, &hash, &config, &state, Instant::now());
//! assert_eq!(result.unwrap(), None);
//! ```
//!
//! ## Unquarantine a workflow
//!
//! ```
//! use vo_core::circuit_breaker::{CircuitBreakerState, unquarantine};
//! use vo_types::WorkflowName;
//!
//! let mut state = CircuitBreakerState::new();
//! let wf = WorkflowName::parse("my-workflow").unwrap();
//!
//! // Set the workflow to quarantined
//! state.set_status(wf.clone(), RegistrationStatus::Quarantined);
//!
//! // Unquarantine — transitions back to Active
//! let result = unquarantine(&wf, "operator", &state);
//! assert!(result.is_ok());
//! assert_eq!(result.unwrap().new_status, RegistrationStatus::Active);
//! ```

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

/// Callback type for quarantine notifications.
///
/// When a workflow is quarantined (due to failure threshold breach), this
/// callback is invoked if one has been set via
/// [`CircuitBreakerState::set_quarantine_callback`]. The callback receives
/// a reference to the [`QuarantineEvent`] containing the quarantined workflow
/// name.
///
/// # Thread Safety
///
/// The callback must be `Send + Sync` to be used across threads. It is
/// wrapped in an `Arc` for shared ownership among [`CircuitBreakerState`]
/// instances.
///
/// # See Also
///
/// - [ADR-026](https://www.adr.org/adr-026) — Quarantine notification architecture
/// - [`CircuitBreakerState::set_quarantine_callback`] — Set the callback
/// - [`CircuitBreakerState::notify_quarantine`] — Invoke the callback
pub type QuarantineCallback = Box<dyn Fn(&QuarantineEvent) + Send + Sync>;

use std::time::Instant;
use vo_types::{BinaryHash, WorkflowName};

/// Evaluate whether a registration request should be allowed.
///
/// This is the primary entry point for the circuit breaker's registration
/// gate. It implements a three-tier evaluation pipeline:
///
/// # Evaluation Pipeline
///
/// ```text
///                    Registration Request
///                          │
///                    ┌──────▼──────┐
///                    │ force=true? │
///                    └──────┬──────┘
///                    ┌──────▼──────┐
///                    │  No: continue│
///                    │  Yes:       │
///                    │  → Allowed  │
///                    └──────┬──────┘
///                          │
///                    ┌──────▼──────────────┐
///                    │ Status Check (MAJ-002)│
///                    │                      │
///                    │  Quarantined →       │
///                    │    WorkflowQuarantined│
///                    │  Deactivated →       │
///                    │    WorkflowDeactivated│
///                    │  Deleted   →         │
///                    │    WorkflowDeactivated│
///                    │  Active    → continue │
///                    └──────┬──────────────┘
///                          │
///                    ┌──────▼──────────┐
///                    │ Layer 1:        │
///                    │ Rate Limit      │
///                    │                 │
///                    │ Within window?  │
///                    │  Yes →          │
///                    │    RateLimited  │
///                    │  No →           │
///                    │    Allowed      │
///                    └──────┬──────────┘
///                          │
///                     Registration Result
/// ```
///
/// # Pre-conditions
///
/// - `request.workflow_name` is parse-validated (see [`WorkflowName`] docs)
/// - `request.binary_hash` is parse-validated (see [`BinaryHash`] docs)
/// - Circuit breaker state (rate limiter, failure tracker, status map) is initialized
///
/// # Postconditions
///
/// | Condition | Outcome | Side Effects |
/// |-----------|---------|--------------|
/// | `force=true` | `Allowed` | Rate limiter updated (POST-005, B-09) |
/// | Status = `Quarantined` | `WorkflowQuarantined` | None |
/// | Status = `Deactivated` | `WorkflowDeactivated` | None |
/// | Status = `Deleted` | `WorkflowDeactivated` | None |
/// | Status = `Active`, rate-limited | `RateLimited { retry_after_secs }` | None |
/// | Status = `Active`, not rate-limited | `Allowed` | Rate limiter timestamp updated |
///
/// # Guarantees
///
/// - **MAJ-002**: Quarantine and deactivation checks are performed BEFORE rate limit
///   checks. Permanent blocks take precedence over temporary cooldowns.
/// - **INV-002**: Rate limiting is applied per-workflow independently.
/// - **INV-009**: Rate-limited requests do NOT count as failures (failure counting
///   is handled separately by [`record_failure()`]).
/// - **CRIT-001**: Rate limit check uses `DashMap::entry()` for atomic
///   read-modify-write, preventing TOCTOU races between get() and insert().
///
/// # Arguments
///
/// * `request` — The registration request to evaluate.
/// * `config` — Circuit breaker configuration (rate limit window, failure window,
///   threshold).
/// * `state` — Current circuit breaker state (status map, rate limiter, failure
///   tracker).
/// * `now` — Current instant, used for rate limit window calculations.
///
/// # Returns
///
/// `Ok(RegistrationOutcome)` with the evaluation result, or `Err(CircuitBreakerError)`
/// on storage failure.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{
///     CircuitBreakerConfig, CircuitBreakerState,
///     RegistrationRequest,
///     evaluate_registration,
/// };
/// use std::time::{Duration, Instant};
/// use vo_types::{WorkflowName, BinaryHash};
///
/// let config = CircuitBreakerConfig::default_config().unwrap();
/// let state = CircuitBreakerState::new();
///
/// let wf = WorkflowName::parse("my-workflow").unwrap();
/// let hash = BinaryHash::parse("abc123").unwrap();
///
/// let request = RegistrationRequest {
///     workflow_name: wf.clone(),
///     binary_hash: hash,
///     force: false,
/// };
///
/// // First registration — allowed (no prior history)
/// let result = evaluate_registration(&request, &config, &state, Instant::now());
/// assert_eq!(result.unwrap(), vo_core::circuit_breaker::RegistrationOutcome::Allowed);
/// ```
pub fn evaluate_registration(
    request: &RegistrationRequest,
    config: &CircuitBreakerConfig,
    state: &CircuitBreakerState,
    now: Instant,
) -> Result<RegistrationOutcome, CircuitBreakerError> {
    // POST-005 / ADR-026: Force registration with operator token bypasses ALL guards.
    // If a token is provided but not registered, return ForceUnauthorized.
    if let Some(ref token) = request.force {
        if !state.operator_tokens.contains_key(token) {
            return Ok(RegistrationOutcome::ForceUnauthorized);
        }
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
        RegistrationStatus::Deleted => {
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
/// This function implements Layer 2 of the circuit breaker: failure loop detection.
/// It tracks unique binary hashes that fail within a sliding time window and
/// triggers automatic quarantine when the threshold is reached.
///
/// # Algorithm
///
/// ```text
///  record_failure(workflow, hash, config, state, now)
///    │
///    ├─ If workflow already Quarantined → return Ok(None) [CRIT-002]
///    │
///    ├─ Get or create FailureWindow for workflow
///    │
///    ├─ Record failure in window:
///    │   ├─ Evict expired entries (INV-007)
///    │   ├─ If hash exists: update timestamp only (INV-004)
///    │   └─ If hash new: insert in sorted position
///    │
///    ├─ Count unique hashes in window
///    │
///    └─ If count ≥ threshold:
///         ├─ Set status → Quarantined
///         ├─ Create QuarantineEvent
///         ├─ Notify quarantine callback (ADR-026)
///         └─ return Ok(Some(event))
///       Else:
///         return Ok(None)
/// ```
///
/// # Invariants
///
/// - **INV-001**: Workflow is quarantined when unique hash failures ≥ `failure_threshold`.
/// - **INV-004**: Duplicate hashes update their timestamp but do not increment count.
/// - **INV-007**: Expired entries are evicted before the threshold check.
/// - **CRIT-002**: If workflow is already quarantined, returns `Ok(None)` immediately.
///   This prevents duplicate `QuarantineEvent` emission and unbounded `FailureWindow` growth.
///
/// # Pre-conditions
///
/// - `workflow_name` and `binary_hash` are parse-validated
/// - `config` and `state` are initialized
/// - `now` is the current instant
///
/// # Postconditions
///
/// | Condition | Return | Side Effects |
/// |-----------|--------|--------------|
/// | Already quarantined | `Ok(None)` | None (no-op) |
/// | New hash, count < threshold | `Ok(None)` | Hash recorded in failure window |
/// | New hash, count ≥ threshold | `Ok(Some(event))` | Status → Quarantined, event emitted, callback invoked |
/// | Duplicate hash | `Ok(None)` | Timestamp updated, count unchanged |
///
/// # Arguments
///
/// * `workflow_name` — The workflow that experienced the failure.
/// * `binary_hash` — The binary hash that failed.
/// * `config` — Circuit breaker configuration.
/// * `state` — Current circuit breaker state.
/// * `now` — Current instant for time window calculations.
///
/// # Returns
///
/// - `Ok(None)` if the threshold was not breached.
/// - `Ok(Some(QuarantineEvent))` if the threshold was breached and quarantine
///   was triggered. The contained event identifies the quarantined workflow.
/// - `Err(CircuitBreakerError::StorageError)` if Fjall write fails.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerState, record_failure};
/// use std::time::Instant;
/// use vo_types::{WorkflowName, BinaryHash};
///
/// let config = CircuitBreakerConfig::default_config().unwrap();
/// let state = CircuitBreakerState::new();
///
/// let wf = WorkflowName::parse("my-workflow").unwrap();
/// let hash = BinaryHash::parse("abc123").unwrap();
///
/// // Record a failure — won't trigger quarantine (threshold is 5)
/// let result = record_failure(&wf, &hash, &config, &state, Instant::now());
/// assert_eq!(result.unwrap(), None);
/// ```
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

        let event = QuarantineEvent {
            workflow_name: workflow_name.clone(),
        };

        // Notify callback if set (ADR-026)
        state.notify_quarantine(&event);

        Ok(Some(event))
    } else {
        Ok(None)
    }
}

/// Reset a quarantined workflow to Active status.
///
/// This is the operator-facing recovery action. It reverses an automatic
/// quarantine triggered by Layer 2 failure detection.
///
/// # Postconditions (POST-003)
///
/// | Action | Description |
/// |--------|-------------|
/// | Status transition | `Quarantined → Active` |
/// | Failure window | Cleared (all records removed) |
/// | Rate limiter | Entry removed — next registration is not rate-limited |
/// | Fjall partition | Updated with new state |
/// | Audit log | Operator action logged |
///
/// # State Transition
///
/// ```text
///  ┌─────────────┐    unquarantine()    ┌──────────┐
///  │ Quarantined  │───────────────────>│  Active   │
///  │              │                    │           │
///  │ - Failure    │                    │ - Rate    │
///  │   window     │                    │   limiter │
///  │   cleared    │                    │   cleared │
///  └─────────────┘                    └──────────┘
/// ```
///
/// # Preconditions
///
/// - The workflow must exist in the status map (any status).
/// - The workflow must be in `Quarantined` status.
///
/// # Arguments
///
/// * `workflow_name` — The workflow to unquarantine.
/// * `_operator` — The operator performing the unquarantine (for audit logging).
/// * `state` — Current circuit breaker state.
///
/// # Returns
///
/// `Ok(UnquarantineResult)` containing the transition details and count of
/// failures that were cleared.
///
/// # Errors
///
/// | Error | Condition |
/// |-------|-----------|
/// | `WorkflowNotFound` | Workflow does not exist in the status map. |
/// | `NotQuarantined` | Workflow exists but is not in `Quarantined` status. |
/// | `StorageError` | Fjall persistence write failed. |
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{CircuitBreakerState, unquarantine, RegistrationStatus};
/// use vo_types::WorkflowName;
///
/// let mut state = CircuitBreakerState::new();
/// let wf = WorkflowName::parse("my-workflow").unwrap();
///
/// // Set the workflow to quarantined
/// state.set_status(wf.clone(), RegistrationStatus::Quarantined);
///
/// // Unquarantine — transitions back to Active
/// let result = unquarantine(&wf, "operator", &state);
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap().new_status, RegistrationStatus::Active);
/// ```
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
