//! Domain types for the circuit breaker module.
//!
//! This module defines the core data structures used across the circuit breaker
//! system: failure records, registration requests, registration outcomes,
//! quarantine events, unquarantine results, and circuit breaker errors.

use std::time::Instant;

use vo_types::{BinaryHash, WorkflowName};

use crate::circuit_breaker::RegistrationStatus;

/// A single failure observation for the circuit breaker.
///
/// Records when a specific binary hash failed for a workflow. Each record
/// contains the binary hash (identifying the exact binary build) and the
/// instant at which the failure was observed.
///
/// Multiple [`FailureRecord`] instances are grouped into a [`FailureWindow`]
/// per workflow. The circuit breaker triggers quarantine when the number of
/// **unique** hashes in the window reaches the failure threshold.
///
/// # Uniqueness
///
/// When the same binary hash fails multiple times, only the most recent
/// timestamp is retained (INV-004). The unique hash count — not the total
/// failure count — determines quarantine.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::FailureRecord;
/// use std::time::Instant;
/// use vo_types::BinaryHash;
///
/// let hash = BinaryHash::parse("abc123").unwrap();
/// let record = FailureRecord {
///     hash: hash.clone(),
///     failed_at: Instant::now(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRecord {
    /// The binary hash that failed.
    ///
    /// Two records with different hashes count as distinct failures toward
    /// the quarantine threshold. Two records with the same hash count as one.
    pub hash: BinaryHash,

    /// The instant at which the failure was observed.
    ///
    /// Used for sliding window expiration: records older than
    /// [`CircuitBreakerConfig::failure_window`] are evicted.
    pub failed_at: Instant,
}

/// Input for a binary registration attempt.
///
/// This struct represents a request to register a new binary for a workflow.
/// The request is evaluated by [`evaluate_registration()`][crate::circuit_breaker::evaluate_registration],
/// which applies rate limiting and status checks to determine whether to allow it.
///
/// # Fields
///
/// | Field | Description |
/// |-------|-------------|
/// | `workflow_name` | The workflow to register the binary for. |
/// | `binary_hash` | The hash of the binary being registered. |
/// | `force` | If `true`, bypasses ALL registration guards. |
///
/// # Force Flag
///
/// When `force` is `true`, the registration is always allowed regardless of:
/// - Workflow quarantine status
/// - Workflow deactivation status
/// - Rate limiting cooldown
///
/// However, the rate limiter is still updated (POST-005, B-09) to maintain
/// accurate timing for subsequent non-forced registrations.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::RegistrationRequest;
/// use vo_types::{WorkflowName, BinaryHash};
///
/// let wf = WorkflowName::parse("my-workflow").unwrap();
/// let hash = BinaryHash::parse("abc123").unwrap();
///
/// let request = RegistrationRequest {
///     workflow_name: wf.clone(),
///     binary_hash: hash,
///     force: false,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RegistrationRequest {
    /// The workflow name to register the binary for.
    pub workflow_name: WorkflowName,

    /// The hash of the binary being registered.
    pub binary_hash: BinaryHash,

    /// True if the operator provided `--force`.
    ///
    /// When true, bypasses all registration guards including rate limiting,
    /// quarantine, and deactivation checks.
    pub force: bool,
}

/// Result of the circuit breaker evaluation.
///
/// Returned by [`evaluate_registration()`][crate::circuit_breaker::evaluate_registration],
/// this enum indicates whether the registration request is permitted and, if
/// denied, the reason why.
///
/// # Decision Flow
///
/// ```text
///  Registration Request
///        │
///        ▼
///  ┌───────────────┐
///  │ force = true? │── Yes ──> Allowed
///  └───────┬───────┘
///          │ No
///          ▼
///  ┌──────────────────┐
///  │ Status = Quar?   │── Yes ──> WorkflowQuarantined
///  └───────┬──────────┘
///          │ No
///          ▼
///  ┌──────────────────┐
///  │ Status = Deact?  │── Yes ──> WorkflowDeactivated
///  └───────┬──────────┘
///          │ No
///          ▼
///  ┌──────────────────┐
///  │ Rate Limited?    │── Yes ──> RateLimited { retry_after_secs }
///  └───────┬──────────┘
///          │ No
///          ▼
///      Allowed
/// ```
///
/// # Variants
///
/// | Variant | Meaning | Action |
/// |---------|---------|--------|
/// | `Allowed` | Registration is permitted | Proceed with binary registration |
/// | `RateLimited` | Too soon since last registration | Wait `retry_after_secs` before retrying |
/// | `WorkflowQuarantined` | Workflow is automatically quarantined | Contact operator to unquarantine |
/// | `WorkflowDeactivated` | Workflow is manually deactivated | Contact operator to reactivate |
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::RegistrationOutcome;
///
/// match RegistrationOutcome::Allowed {
///     RegistrationOutcome::Allowed => { /* proceed */ }
///     RegistrationOutcome::RateLimited { retry_after_secs } => { /* wait */ }
///     RegistrationOutcome::WorkflowQuarantined { .. } => { /* quarantined */ }
///     RegistrationOutcome::WorkflowDeactivated { .. } => { /* deactivated */ }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationOutcome {
    /// Registration is permitted. Proceed with binary registration.
    Allowed,

    /// Registration denied: rate limit exceeded.
    ///
    /// The workflow's last registration was within the cooldown window.
    /// The caller should wait `retry_after_secs` seconds before retrying.
    RateLimited { retry_after_secs: u64 },

    /// Registration denied: workflow is quarantined.
    ///
    /// The workflow was automatically quarantined by Layer 2 failure detection
    /// because `failure_threshold` unique binary hashes failed within the
    /// `failure_window`. Only [`unquarantine()`][crate::circuit_breaker::unquarantine]
    /// can restore the workflow.
    WorkflowQuarantined { workflow_name: WorkflowName },

    /// Registration denied: workflow is deactivated.
    ///
    /// The workflow was manually deactivated by an operator. It may be
    /// reactivated by the operator.
    WorkflowDeactivated { workflow_name: WorkflowName },
}

/// Event emitted when a workflow is quarantined.
///
/// This struct is created when the failure threshold is breached and sent to
/// the quarantine callback (if set). It identifies the quarantined workflow.
///
/// # Callback Invocation
///
/// The event is passed to the callback set via
/// [`CircuitBreakerState::set_quarantine_callback`]. The callback type is
/// [`QuarantineCallback`], which is `Box<dyn Fn(&QuarantineEvent) + Send + Sync>`.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::QuarantineEvent;
/// use vo_types::WorkflowName;
///
/// let wf = WorkflowName::parse("my-workflow").unwrap();
/// let event = QuarantineEvent {
///     workflow_name: wf.clone(),
/// };
/// assert_eq!(event.workflow_name, wf);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineEvent {
    /// The workflow that was quarantined.
    pub workflow_name: WorkflowName,
}

/// Successful result of an unquarantine operation.
///
/// Returned by [`unquarantine()`][crate::circuit_breaker::unquarantine], this
/// struct provides details about the status transition and the failures that
/// were cleared.
///
/// # Postconditions (POST-003)
///
/// After a successful unquarantine:
/// - Status transitions from `Quarantined` to `Active`
/// - The failure window is cleared (0 entries)
/// - The rate limiter entry is removed
/// - `failures_cleared` indicates how many failure records were removed
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{CircuitBreakerState, unquarantine, RegistrationStatus};
/// use vo_types::WorkflowName;
///
/// let mut state = CircuitBreakerState::new();
/// let wf = WorkflowName::parse("my-workflow").unwrap();
/// state.set_status(wf.clone(), RegistrationStatus::Quarantined);
///
/// let result = unquarantine(&wf, "operator", &state).unwrap();
/// assert_eq!(result.previous_status, RegistrationStatus::Quarantined);
/// assert_eq!(result.new_status, RegistrationStatus::Active);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnquarantineResult {
    /// The workflow that was unquarantined.
    pub workflow_name: WorkflowName,

    /// The status before unquarantine (always `Quarantined`).
    pub previous_status: RegistrationStatus,

    /// The status after unquarantine (always `Active`).
    pub new_status: RegistrationStatus,

    /// Number of failure records that were cleared during unquarantine.
    ///
    /// This equals the size of the failure window that was removed.
    pub failures_cleared: usize,
}

/// Errors that can occur during circuit breaker operations.
///
/// This enum covers all failure modes of the circuit breaker system, including
/// rate limit violations, quarantine blocks, deactivation blocks, storage errors,
/// and unquarantine failures.
///
/// # Error Categories
///
/// | Category | Variants | Recoverable |
/// |----------|----------|-------------|
/// | Rate limit | [`RateLimited`] | Yes — retry after `retry_after_secs` |
/// | Quarantine | [`WorkflowQuarantined`] | Yes — call [`unquarantine()`][crate::circuit_breaker::unquarantine] |
/// | Deactivation | [`WorkflowDeactivated`] | Yes — operator must reactivate |
/// | Storage | [`StorageError`] | No — investigate disk/persistence |
/// | Workflow not found | [`WorkflowNotFound`] | No — workflow was never registered |
/// | Invalid unquarantine | [`NotQuarantined`] | Yes — workflow is in a different state |
///
/// # Error Handling Strategy
///
/// - **RateLimited**: The caller should back off and retry after the specified
///   number of seconds.
/// - **WorkflowQuarantined**: The caller should notify the operator or attempt
///   automatic unquarantine after a grace period.
/// - **WorkflowDeactivated**: The caller should check whether the workflow is
///   intentionally disabled.
/// - **StorageError**: This indicates a system-level failure (disk, Fjall)
///   that requires investigation.
/// - **WorkflowNotFound**: The workflow was never seen before. For unquarantine
///   operations, this is an error. For registration, this should not occur
///   (the workflow defaults to `Active`).
/// - **NotQuarantined**: The workflow exists but is not in the expected state.
///   The caller should check the current status.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::CircuitBreakerError;
///
/// let err = CircuitBreakerError::RateLimited { retry_after_secs: 30 };
/// assert_eq!(err.to_string(), "rate_limited: retry after 30s");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CircuitBreakerError {
    /// Attempted to register a binary for a rate-limited workflow.
    ///
    /// The workflow's last registration was within the cooldown window.
    /// Retry after `retry_after_secs` seconds.
    #[error("rate_limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    /// Attempted to register a binary for a quarantined workflow.
    ///
    /// The workflow was automatically quarantined due to repeated failures.
    /// Contact the operator to unquarantine.
    #[error("workflow_quarantined: {workflow_name}")]
    WorkflowQuarantined { workflow_name: String },

    /// Attempted to register a binary for a deactivated workflow.
    ///
    /// The workflow was manually deactivated. Contact the operator to reactivate.
    #[error("workflow_deactivated: {workflow_name}")]
    WorkflowDeactivated { workflow_name: String },

    /// Persistence failure when reading/writing quarantine state.
    ///
    /// This error indicates a system-level failure in the storage layer
    /// (typically Fjall). The circuit breaker may be in an inconsistent
    /// state. Investigate the underlying cause.
    #[error("storage_error: {reason}")]
    StorageError { reason: String },

    /// Workflow not found when attempting unquarantine.
    ///
    /// The workflow was never registered in the status map. For unquarantine
    /// operations, this means the workflow has no history.
    #[error("workflow_not_found: {workflow_name}")]
    WorkflowNotFound { workflow_name: String },

    /// Attempted to unquarantine a workflow that is not quarantined.
    ///
    /// The workflow exists in the status map but is in a different state
    /// (e.g., `Active`, `Deactivated`, `Deleted`). Check the current status
    /// before attempting unquarantine.
    #[error("not_quarantined: {workflow_name} is {current_status:?}")]
    NotQuarantined {
        workflow_name: String,
        current_status: RegistrationStatus,
    },
}
