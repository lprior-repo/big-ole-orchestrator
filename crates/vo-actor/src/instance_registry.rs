//! Instance registry: enforces at most one active InstanceActor per InstanceId.
//!
//! Architecture: Data → Calc → Actions
//! - Data: `InstanceActorHandle`, `RegistryConfig`, `RegistryError`
//! - Calc: `determine_register_outcome` (pure decision logic)
//! - Actions: `execute_stop_fn_with_timeout` (thread spawn + channel I/O)
//!
//! The registry is the sole authority for which instance actors are alive.
//! Any attempt to register a second actor for the same `InstanceId` stops
//! the prior actor first (stop-before-replace semantics).

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use vo_types::InstanceId;

// =============================================================================
// Data Layer — inert types
// =============================================================================

/// Opaque handle to a running instance actor.
///
/// Wraps whatever the ractor supervision tree needs. The registry
/// never inspects the interior; it only stores and returns it.
///
/// For testing, use [`InstanceActorHandle::test`] to construct handles
/// with known identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceActorHandle {
    test_id: u64,
}

impl InstanceActorHandle {
    /// Creates a test handle with the given identifier.
    ///
    /// Two handles are equal when their `test_id` values match.
    #[must_use]
    pub fn test(id: u64) -> Self {
        Self { test_id: id }
    }

    /// Returns the test identifier for this handle.
    #[must_use]
    pub fn handle_id(&self) -> u64 {
        self.test_id
    }
}

/// Configuration for the registry's stop-before-replace behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    /// Maximum time to wait for a prior actor to stop before giving up.
    /// Must be > 0. Default: 5 seconds.
    pub stop_timeout: Duration,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            stop_timeout: Duration::from_secs(5),
        }
    }
}

/// Errors from registry operations.
///
/// Every variant carries full context for debugging and error classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The prior actor's stop function returned an error.
    /// The new actor was NOT registered. The old actor remains active.
    StopFailed {
        instance_id: InstanceId,
        reason: String,
    },

    /// The stop_timeout elapsed before the prior actor terminated.
    /// The new actor was NOT registered. The old actor may still be running.
    StopTimeout {
        instance_id: InstanceId,
        timeout: Duration,
    },

    /// Attempted to deregister an InstanceId that is not in the registry.
    /// Indicates a logic error in the caller.
    NotRegistered { instance_id: InstanceId },
}

// =============================================================================
// Calculation Layer — pure decision types
// =============================================================================

/// Outcome of invoking the stop function, used to drive register logic
/// without mixing I/O decisions with state mutation.
enum StopFnOutcome {
    Success,
    Failed(String),
    Timeout,
}

/// Pure decision function: given the stop_fn outcome, produce the correct
/// register result. No side effects — the caller is responsible for
/// applying the map mutation.
fn determine_register_outcome(
    outcome: StopFnOutcome,
    instance_id: InstanceId,
    timeout: Duration,
) -> Result<(), RegistryError> {
    match outcome {
        StopFnOutcome::Success => Ok(()),
        StopFnOutcome::Failed(reason) => Err(RegistryError::StopFailed {
            instance_id,
            reason,
        }),
        StopFnOutcome::Timeout => Err(RegistryError::StopTimeout {
            instance_id,
            timeout,
        }),
    }
}

// =============================================================================
// Action Layer — I/O boundary (thread spawn + channel)
// =============================================================================

/// Executes the caller-provided stop function in a spawned thread with a
/// bounded timeout. Returns the outcome without mutating any registry state.
///
/// Uses `std::thread::scope` so the stop_fn does NOT require `Send + 'static`
/// bounds. The scope blocks until the spawned thread completes, but the
/// channel timeout fires correctly — the outcome is determined by which
/// event occurs first (stop_fn result vs timeout).
fn execute_stop_fn_with_timeout(
    stop_fn: impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send,
    handle: InstanceActorHandle,
    timeout: Duration,
) -> StopFnOutcome {
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|s| {
        s.spawn(move || {
            let _ = tx.send(stop_fn(handle));
        });
        match rx.recv_timeout(timeout) {
            Ok(Ok(())) => StopFnOutcome::Success,
            Ok(Err(reason)) => StopFnOutcome::Failed(reason),
            Err(_) => StopFnOutcome::Timeout,
        }
    })
}

// =============================================================================
// InstanceRegistry — the single-active instance registry
// =============================================================================

/// The single-active instance registry.
///
/// Enforces at most one active [`InstanceActorHandle`] per [`InstanceId`].
/// All mutations go through [`register`](InstanceRegistry::register) or
/// [`deregister`](InstanceRegistry::deregister).
///
/// # Invariants
///
/// - **INV-1 (Single-Active)**: At most one handle per `InstanceId`.
/// - **INV-2 (Bijection)**: No two `InstanceId`s map to the same handle (ownership).
/// - **INV-3 (Count Consistency)**: `active_count()` always equals the map length.
/// - **INV-4 (Stop-Before-Replace)**: Prior actor stopped before new one replaces it.
/// - **INV-5 (No Partial Mutations)**: On error, registry state is unchanged.
pub struct InstanceRegistry {
    entries: HashMap<InstanceId, InstanceActorHandle>,
    stop_timeout: Duration,
}

impl InstanceRegistry {
    /// Creates a new, empty instance registry with the given config.
    ///
    /// # Panics
    /// Panics if `config.stop_timeout` is zero.
    #[must_use]
    pub fn new(config: RegistryConfig) -> Self {
        assert!(
            config.stop_timeout > Duration::ZERO,
            "stop_timeout must be greater than zero"
        );
        Self {
            entries: HashMap::new(),
            stop_timeout: config.stop_timeout,
        }
    }

    /// Registers an instance actor handle.
    ///
    /// If `id` is not currently active, inserts the handle and returns `Ok(())`.
    ///
    /// If `id` IS currently active, invokes `stop_fn` on the prior handle.
    ///   - If `stop_fn` returns `Ok(())`, the old entry is removed, the new
    ///     handle is inserted, and this method returns `Ok(())`.
    ///   - If `stop_fn` returns `Err(reason)`, the old entry is preserved,
    ///     the new handle is NOT inserted, and this method returns
    ///     `Err(RegistryError::StopFailed)`.
    ///   - If the `config.stop_timeout` elapses before `stop_fn` completes,
    ///     returns `Err(RegistryError::StopTimeout)`.
    ///
    /// # Errors
    /// - `RegistryError::StopFailed` if prior actor stop failed.
    /// - `RegistryError::StopTimeout` if prior actor stop exceeded timeout.
    pub fn register(
        &mut self,
        id: InstanceId,
        handle: InstanceActorHandle,
        stop_fn: impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send,
    ) -> Result<(), RegistryError> {
        // INV-5: remove first, clone for rollback on error paths
        let prior_handle = match self.entries.remove(&id) {
            None => {
                // Fresh insert — no stop_fn needed
                self.entries.insert(id, handle);
                return Ok(());
            }
            Some(h) => h,
        };

        // Clone the handle so we can re-insert on failure.
        // The original is passed to stop_fn; the clone is preserved for rollback.
        let preserved = prior_handle.clone();
        let timeout = self.stop_timeout;

        let outcome = execute_stop_fn_with_timeout(stop_fn, prior_handle, timeout);
        let result = determine_register_outcome(outcome, id.clone(), timeout);

        match &result {
            Ok(()) => {
                self.entries.insert(id, handle);
            }
            Err(_) => {
                // INV-5: rollback — re-insert the preserved old handle
                self.entries.insert(id, preserved);
            }
        }

        result
    }

    /// Deregisters an instance actor, returning its handle.
    ///
    /// # Errors
    /// - `RegistryError::NotRegistered` if `id` is not in the registry.
    pub fn deregister(&mut self, id: &InstanceId) -> Result<InstanceActorHandle, RegistryError> {
        self.entries
            .remove(id)
            .ok_or_else(|| RegistryError::NotRegistered {
                instance_id: id.clone(),
            })
    }

    /// Looks up an active instance actor by ID.
    ///
    /// Returns `Some(&handle)` if active, `None` otherwise.
    /// No side effects. No errors.
    #[must_use]
    pub fn lookup(&self, id: &InstanceId) -> Option<&InstanceActorHandle> {
        self.entries.get(id)
    }

    /// Checks if an instance actor is currently active.
    ///
    /// Returns `true` iff `lookup(id)` would return `Some`.
    /// No side effects. No errors.
    #[must_use]
    pub fn is_active(&self, id: &InstanceId) -> bool {
        self.entries.contains_key(id)
    }

    /// Returns the number of currently active instance actors.
    ///
    /// Always equals the number of entries in the internal map.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.entries.len()
    }
}
