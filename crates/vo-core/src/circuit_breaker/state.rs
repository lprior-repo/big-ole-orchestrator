//! Shared mutable state for the circuit breaker.
//!
//! This module defines the [`CircuitBreakerState`] struct that holds the
//! concurrent state for rate limiting, failure tracking, and workflow status
//! management.
//!
//! # Concurrency Model
//!
//! All maps are backed by [`DashMap`] for lock-free concurrent reads and
//! fine-grained partition-level writes (INV-010). This allows multiple threads
//! to evaluate registration requests, record failures, and query status
//! concurrently without deadlock.
//!
//! # Field Access Discipline
//!
//! MAJ-001: `DashMap` fields are not directly public. Core logic uses safe
//! accessor methods that guarantee guards are dropped before returning,
//! preventing deadlock from nested lock acquisition.
//!
//! Direct field access (`statuses`, `rate_limiter`, `failure_tracker`) is
//! available for test setup and advanced operations that require `DashMap`-level
//! control. However, production code should always use the accessor methods.
//!
//! # State Architecture
//!
//! ```text
//!  ┌─────────────────────────────────────────────────────────┐
//! │                 CircuitBreakerState                       │
//! │                                                          │
//! │  ┌───────────────────┐  ┌──────────────────┐            │
//! │  │   statuses        │  │  rate_limiter    │            │
//! │  │  DashMap<WF, Sts> │  │  DashMap<WF, T>  │            │
//! │  │                   │  │                  │            │
//! │  │  WF → Active      │  │  WF → Instant    │            │
//! │  │  WF → Quarantined │  │  WF → Instant    │            │
//! │  │  WF → Deactivated │  │                  │            │
//! │  │  WF → Deleted     │  │                  │            │
//! │  └───────────────────┘  └──────────────────┘            │
//! │                                                          │
//! │  ┌──────────────────────┐  ┌──────────────────────┐     │
//! │  │  failure_tracker     │  │  quarantine_callback │     │
//! │  │  DashMap<WF, FW>     │  │  Option<Arc<Fn(...)>>│     │
//! │  │                      │  │                      │     │
//! │  │  WF → FailureWindow  │  │  None / Some(cb)     │     │
//! │  │  WF → FailureWindow  │  │                      │     │
//! │  └──────────────────────┘  └──────────────────────┘     │
//! └─────────────────────────────────────────────────────────┘
//!
//!  Legend: WF = WorkflowName, Sts = RegistrationStatus, T = Instant, FW = FailureWindow
//! ```
//!
//! # Examples
//!
//! ```
//! use vo_core::circuit_breaker::{CircuitBreakerState, RegistrationStatus};
//! use vo_types::WorkflowName;
//!
//! let state = CircuitBreakerState::new();
//! let wf = WorkflowName::parse("test-wf").unwrap();
//!
//! // Unknown workflows default to Active (INV-005)
//! assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
//!
//! // Set and read status
//! state.set_status(wf.clone(), RegistrationStatus::Quarantined);
//! assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
//! ```

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
///
/// # Field Layout
///
/// | Field | Type | Purpose | Default |
/// |-------|------|---------|---------|
/// | `statuses` | `DashMap<WorkflowName, RegistrationStatus>` | Workflow lifecycle status | Unknown → `Active` |
/// | `rate_limiter` | `DashMap<WorkflowName, Instant>` | Last registration timestamp per workflow | None |
/// | `failure_tracker` | `DashMap<WorkflowName, FailureWindow>` | Sliding failure window per workflow | None |
/// | `quarantine_callback` | `Option<Arc<QuarantineCallback>>` | Optional notification on quarantine | `None` |
///
/// # Thread Safety
///
/// `CircuitBreakerState` is `Sync + Send` because all mutable state is accessed
/// through `DashMap`, which uses fine-grained sharded locking. Multiple threads
/// can safely read and write to different partitions concurrently.
///
/// # Examples
///
/// ```
/// use vo_core::circuit_breaker::{CircuitBreakerState, RegistrationStatus};
/// use std::time::Instant;
/// use vo_types::WorkflowName;
///
/// let state = CircuitBreakerState::new();
/// let wf = WorkflowName::parse("my-workflow").unwrap();
///
/// // Set status
/// state.set_status(wf.clone(), RegistrationStatus::Quarantined);
/// assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
///
/// // Set rate limiter
/// state.set_rate_limit(wf.clone(), Instant::now());
/// assert!(state.get_rate_limit(&wf).is_some());
///
/// // Remove rate limiter entry
/// state.remove_rate_limit(&wf);
/// assert!(state.get_rate_limit(&wf).is_none());
/// ```
pub struct CircuitBreakerState {
    /// Workflow status map. Unknown workflows default to `Active`.
    ///
    /// This map tracks the lifecycle status of each workflow: `Active`,
    /// `Quarantined`, `Deactivated`, or `Deleted`.
    ///
    /// # Access Pattern
    ///
    /// Prefer using [`get_status()`][Self::get_status] and
    /// [`set_status()`][Self::set_status] for safe access. Direct field access
    /// is available for test setup and advanced operations.
    ///
    /// # Invariant (INV-005)
    ///
    /// Unknown workflows (not present in this map) default to `Active` when
    /// queried via `get_status()`. This means new workflows can register
    /// immediately without pre-registration.
    pub statuses: DashMap<WorkflowName, RegistrationStatus>,

    /// Rate limiter: last successful registration timestamp per workflow.
    ///
    /// Stores the `Instant` of the last successful registration for each
    /// workflow. Used by [`check_rate_limit()`][crate::circuit_breaker::check_rate_limit]
    /// to determine if a new registration is within the cooldown window.
    ///
    /// # Access Pattern
    ///
    /// Prefer using [`get_rate_limit()`][Self::get_rate_limit] and
    /// [`set_rate_limit()`][Self::set_rate_limit] for safe access.
    pub rate_limiter: DashMap<WorkflowName, Instant>,

    /// Failure tracker: sliding failure window per workflow.
    ///
    /// Maps each workflow to its [`FailureWindow`], which tracks unique
    /// binary hash failures within a sliding time window. Used by
    /// [`record_failure()`][crate::circuit_breaker::record_failure] to count
    /// failures toward the quarantine threshold.
    ///
    /// # Access Pattern
    ///
    /// Prefer using [`get_failure_count()`][Self::get_failure_count] for safe
    /// reads. Direct field access via `entry()` is used by `record_failure()`
    /// for atomic read-modify-write on the failure window.
    pub failure_tracker: DashMap<WorkflowName, FailureWindow>,

    /// Optional callback for quarantine notifications (ADR-026).
    ///
    /// When set, this callback is invoked whenever a workflow is quarantined
    /// (i.e., when [`record_failure()`] triggers quarantine due to threshold
    /// breach). The callback receives a reference to the
    /// [`QuarantineEvent`].
    ///
    /// The callback is wrapped in an `Arc` to allow sharing across multiple
    /// `CircuitBreakerState` instances (e.g., in multi-threaded environments).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{CircuitBreakerState, QuarantineEvent};
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// let state = CircuitBreakerState::new();
    /// let counter = Arc::new(AtomicUsize::new(0));
    /// let counter_clone = counter.clone();
    ///
    /// state.set_quarantine_callback(Box::new(move |event: &QuarantineEvent| {
    ///     counter_clone.fetch_add(1, Ordering::Relaxed);
    /// }));
    ///
    /// assert!(state.quarantine_callback.is_some());
    /// ```
    pub quarantine_callback: Option<Arc<QuarantineCallback>>,
}

impl CircuitBreakerState {
    /// Create a new empty state with no workflows tracked.
    ///
    /// All three `DashMap` collections are initialized empty, and no
    /// quarantine callback is set.
    ///
    /// # Default Behavior
    ///
    /// - Unknown workflows return `Active` from `get_status()` (INV-005).
    /// - Unknown workflows return `None` from `get_rate_limit()`.
    /// - Unknown workflows return `0` from `get_failure_count()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerState;
    ///
    /// let state = CircuitBreakerState::new();
    /// assert!(state.statuses.is_empty());
    /// assert!(state.rate_limiter.is_empty());
    /// assert!(state.failure_tracker.is_empty());
    /// assert!(state.quarantine_callback.is_none());
    /// ```
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
    ///
    /// The callback will be invoked whenever a workflow is quarantined due
    /// to failure threshold breach. Any previously set callback is replaced.
    ///
    /// # Arguments
    ///
    /// * `callback` — The callback function. Must be `Send + Sync` to work
    ///   across threads. Wrapped in `Arc` for shared ownership.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{CircuitBreakerState, QuarantineEvent};
    ///
    /// let mut state = CircuitBreakerState::new();
    /// state.set_quarantine_callback(Box::new(|event: &QuarantineEvent| {
    ///     eprintln!("Workflow quarantined: {}", event.workflow_name);
    /// }));
    /// assert!(state.quarantine_callback.is_some());
    /// ```
    pub fn set_quarantine_callback(&mut self, callback: QuarantineCallback) {
        self.quarantine_callback = Some(Arc::new(callback));
    }

    /// Invoke the quarantine callback if set.
    ///
    /// This method is called internally by [`record_failure()`][crate::circuit_breaker::record_failure]
    /// when the failure threshold is breached. It passes the quarantine event
    /// to the callback, which can perform side effects such as logging, metrics
    /// collection, or alerting.
    ///
    /// If no callback is set, this is a no-op.
    ///
    /// # Arguments
    ///
    /// * `event` — The quarantine event containing the quarantined workflow name.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{CircuitBreakerState, QuarantineEvent};
    /// use std::sync::{Arc, Mutex};
    /// use vo_types::WorkflowName;
    ///
    /// let mut state = CircuitBreakerState::new();
    /// let logged = Arc::new(Mutex::new(Vec::new()));
    /// let logged_clone = logged.clone();
    ///
    /// state.set_quarantine_callback(Box::new(move |event: &QuarantineEvent| {
    ///     logged_clone.lock().unwrap().push(event.workflow_name.to_string());
    /// }));
    ///
    /// let wf = WorkflowName::parse("test").unwrap();
    /// let event = QuarantineEvent {
    ///     workflow_name: wf.clone(),
    /// };
    /// state.notify_quarantine(&event);
    ///
    /// assert_eq!(*logged.lock().unwrap(), vec![wf.to_string()]);
    /// ```
    pub fn notify_quarantine(&self, event: &QuarantineEvent) {
        if let Some(callback) = &self.quarantine_callback {
            callback(event);
        }
    }

    // ── Safe value accessors (guards dropped before return) ─────────────

    /// Read the registration status for a workflow.
    ///
    /// Returns `Active` for unknown workflows (INV-005). This default-to-active
    /// behavior means new workflows can register without pre-registration.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow to look up.
    ///
    /// # Returns
    ///
    /// The current [`RegistrationStatus`] for the workflow, or `Active` if
    /// the workflow has no entry in the status map.
    ///
    /// # Invariant (INV-005)
    ///
    /// Unknown workflows default to `Active`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{CircuitBreakerState, RegistrationStatus};
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("unknown").unwrap();
    ///
    /// // Unknown workflow → Active (INV-005)
    /// assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
    ///
    /// // Set and read
    /// state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    /// assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
    /// ```
    #[must_use]
    pub fn get_status(&self, workflow_name: &WorkflowName) -> RegistrationStatus {
        self.statuses
            .get(workflow_name)
            .map_or(RegistrationStatus::Active, |s| *s)
    }

    /// Insert or update a workflow's registration status.
    ///
    /// This method atomically sets the status for a workflow in the status map.
    /// If the workflow already has a status, it is overwritten.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow whose status to set.
    /// * `status` — The new [`RegistrationStatus`] to assign.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::{CircuitBreakerState, RegistrationStatus};
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("my-wf").unwrap();
    ///
    /// state.set_status(wf.clone(), RegistrationStatus::Quarantined);
    /// assert_eq!(state.get_status(&wf), RegistrationStatus::Quarantined);
    ///
    /// state.set_status(wf, RegistrationStatus::Active);
    /// assert_eq!(state.get_status(&wf), RegistrationStatus::Active);
    /// ```
    pub fn set_status(&self, workflow_name: WorkflowName, status: RegistrationStatus) {
        self.statuses.insert(workflow_name, status);
    }

    /// Read the last registration timestamp for rate limiting.
    ///
    /// Returns `None` if the workflow has no rate limiter entry (i.e., has
    /// never been registered, or the entry was removed by unquarantine).
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow to look up.
    ///
    /// # Returns
    ///
    /// The `Instant` of the last successful registration, or `None` if not set.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerState;
    /// use std::time::Instant;
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("my-wf").unwrap();
    ///
    /// assert_eq!(state.get_rate_limit(&wf), None);
    ///
    /// state.set_rate_limit(wf.clone(), Instant::now());
    /// assert!(state.get_rate_limit(&wf).is_some());
    /// ```
    #[must_use]
    pub fn get_rate_limit(&self, workflow_name: &WorkflowName) -> Option<Instant> {
        self.rate_limiter.get(workflow_name).map(|r| *r)
    }

    /// Update the rate limiter timestamp for a workflow.
    ///
    /// Records the given timestamp as the last successful registration time
    /// for the workflow. This is called after a successful registration
    /// to update the cooldown window.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow to update.
    /// * `timestamp` — The current instant (last successful registration time).
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerState;
    /// use std::time::Instant;
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("my-wf").unwrap();
    /// let now = Instant::now();
    ///
    /// state.set_rate_limit(wf.clone(), now);
    /// assert_eq!(state.get_rate_limit(&wf), Some(now));
    /// ```
    pub fn set_rate_limit(&self, workflow_name: WorkflowName, timestamp: Instant) {
        self.rate_limiter.insert(workflow_name, timestamp);
    }

    /// Remove the rate limiter entry for a workflow.
    ///
    /// This clears the last registration timestamp, effectively resetting the
    /// rate limiter for this workflow. This is called during unquarantine
    /// (POST-003) to allow immediate re-registration after unquarantine.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow whose rate limiter entry to remove.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerState;
    /// use std::time::Instant;
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("my-wf").unwrap();
    /// state.set_rate_limit(wf.clone(), Instant::now());
    ///
    /// state.remove_rate_limit(&wf);
    /// assert_eq!(state.get_rate_limit(&wf), None);
    /// ```
    pub fn remove_rate_limit(&self, workflow_name: &WorkflowName) {
        self.rate_limiter.remove(workflow_name);
    }

    /// Get the failure count for a workflow (guards dropped before return).
    ///
    /// Returns the number of unique failure records currently in the workflow's
    /// [`FailureWindow`]. This is the count used by [`record_failure()`][crate::circuit_breaker::record_failure]
    /// to determine if the quarantine threshold has been reached.
    ///
    /// Returns `0` for unknown workflows (no failure window exists).
    ///
    /// # Arguments
    ///
    /// * `workflow_name` — The workflow to look up.
    ///
    /// # Returns
    ///
    /// The number of failure records in the workflow's failure window, or `0`
    /// if no failure window exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::circuit_breaker::CircuitBreakerState;
    /// use vo_types::WorkflowName;
    ///
    /// let state = CircuitBreakerState::new();
    /// let wf = WorkflowName::parse("my-wf").unwrap();
    ///
    /// // Unknown workflow → 0 failures
    /// assert_eq!(state.get_failure_count(&wf), 0);
    /// ```
    #[must_use]
    pub fn get_failure_count(&self, workflow_name: &WorkflowName) -> usize {
        self.failure_tracker
            .get(workflow_name)
            .map_or(0, |w| w.len())
    }
}

impl Default for CircuitBreakerState {
    /// Returns a new empty [`CircuitBreakerState`].
    ///
    /// Equivalent to [`CircuitBreakerState::new()`].
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CircuitBreakerState {
    /// Formats the state for debugging, showing map sizes rather than contents.
    ///
    /// This prevents flooding debug output with the contents of large maps
    /// while still providing useful diagnostic information (entry counts).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreakerState")
            .field("statuses", &self.statuses.len())
            .field("rate_limiter", &self.rate_limiter.len())
            .field("failure_tracker", &self.failure_tracker.len())
            .field("quarantine_callback", &self.quarantine_callback.is_some())
            .finish()
    }
}
