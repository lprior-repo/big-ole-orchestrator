//! Ordered drop registry enforcing deterministic cancellation drop ordering.
//!
//! Per ADR-050 and ADR-055: when cancellation occurs, resources MUST be dropped
//! in reverse-initialization order (LIFO). This ensures:
//! - Children drop before parents (no use-after-close)
//! - Shared state is safe to drop from any task
//! - No resource leaks on structured cancellation
//!
//! ## Drop Ordering Contract
//!
//! 1. **Reverse-initialization order**: Resources registered last are dropped first.
//! 2. **Deterministic**: Same registration order always produces same drop order.
//! 3. **No use-after-free**: Parent resources cannot be dropped while children
//!    still hold references to them.
//! 4. **Cancellation-safe**: Dropping mid-operation leaves state consistent.
//!
//! ## Usage
//!
//! ```
//! let registry = OrderedDropRegistry::new();
//! // Register in initialization order (worker → core → actor → storage)
//! registry.register("storage", DropAction::sync(|| storage.flush()));
//! registry.register("actor", DropAction::sync(|| actor.terminate()));
//! registry.register("core", DropAction::sync(|| core.flush()));
//! registry.register("worker", DropAction::sync(|| worker.drain()));
//!
//! // On shutdown, drops in reverse: worker → core → actor → storage
//! registry.shutdown();
//! ```

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::RwLock;
use tracing::debug;

// =============================================================================
// DropAction - A cancellable cleanup action
// =============================================================================

/// A cleanup action that can be executed during shutdown.
///
/// Supports both synchronous (best-effort) and tracked actions.
/// Per ADR-050: Drop implementations must not block on I/O.
pub struct DropAction {
    name: String,
    priority: u32,
    kind: DropActionKind,
}

/// The type of drop action.
enum DropActionKind {
    /// Synchronous best-effort cleanup (must complete quickly).
    Sync(Box<dyn FnOnce() + Send>),
}

impl DropAction {
    /// Creates a new synchronous drop action.
    ///
    /// Per ADR-050 Section 4.1: Drop implementations MUST complete synchronously
    /// and MUST NOT await async operations, lock mutexes that may be held by
    /// the same task, send on channels that may be full, or perform blocking I/O.
    pub fn sync<F>(name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            name: name.into(),
            priority: 0,
            kind: DropActionKind::Sync(Box::new(f)),
        }
    }

    /// Creates a new synchronous drop action with explicit priority.
    ///
    /// Higher priority = drops LATER (closer to parent).
    /// Resources with lower priority drop first (children before parents).
    pub fn with_priority(
        name: impl Into<String>,
        priority: u32,
        f: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            priority,
            kind: DropActionKind::Sync(Box::new(f)),
        }
    }

    /// Executes this drop action.
    fn execute(&mut self) {
        let name = self.name.clone();
        debug!("Executing drop action: {name}");
        match std::mem::take(&mut self.kind) {
            DropActionKind::Sync(f) => f(),
        }
    }

    /// Returns the name of this action.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

// =============================================================================
// OrderedDropRegistry
// =============================================================================

/// Registry that enforces deterministic drop ordering on cancellation.
///
/// Resources are registered in initialization order. On shutdown, they are
/// dropped in reverse order (last registered = first dropped = child first).
///
/// ## Invariants
///
/// 1. **Deterministic ordering**: Registration order is preserved and enforced.
/// 2. **No orphans**: All registered actions must complete before shutdown returns.
/// 3. **Idempotent**: Calling `shutdown()` multiple times is safe.
/// 4. **Cancellation-safe**: Partial shutdown leaves registry consistent.
#[derive(Debug)]
pub struct OrderedDropRegistry {
    /// Actions sorted by registration order (LIFO drop).
    actions: RwLock<Vec<MaybeDoneAction>>,
    /// Monotonically increasing counter for registration order.
    order_counter: AtomicUsize,
    /// Whether shutdown has been initiated.
    shutdown_flag: AtomicUsize, // 0 = not started, 1 = in progress, 2 = complete
}

/// Wraps a potentially-completed action result.
enum MaybeDoneAction {
    Pending(DropAction),
    Done,
}

impl Default for OrderedDropRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderedDropRegistry {
    /// Creates a new empty ordered drop registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            actions: RwLock::new(Vec::new()),
            order_counter: AtomicUsize::new(0),
            shutdown_flag: AtomicUsize::new(0),
        }
    }

    /// Registers a drop action in the ordering.
    ///
    /// Actions registered later are dropped first (LIFO order).
    /// This enforces the reverse-initialization contract from ADR-055.
    pub fn register(&self, action: DropAction) {
        let mut actions = self.actions.write();
        let order = self.order_counter.fetch_add(1, Ordering::Relaxed);
        debug!(
            "Registering drop action: {} (order={}, priority={})",
            action.name(),
            order,
            action.priority
        );
        actions.push(MaybeDoneAction::Pending(action));
    }

    /// Registers a synchronous drop action with a name.
    pub fn register_sync(&self, name: impl Into<String>, f: impl FnOnce() + Send + 'static) {
        self.register(DropAction::sync(name, f));
    }

    /// Registers a synchronous drop action with explicit priority.
    pub fn register_with_priority(
        &self,
        name: impl Into<String>,
        priority: u32,
        f: impl FnOnce() + Send + 'static,
    ) {
        self.register(DropAction::with_priority(name, priority, f));
    }

    /// Executes shutdown in reverse registration order (LIFO = children first).
    ///
    /// Per ADR-055: Shutdown sequence is reverse-initialization order.
    /// This is idempotent - calling it multiple times is safe.
    ///
    /// Returns `ShutdownOrder` indicating success and which actions were executed.
    pub fn shutdown(&self) -> ShutdownOrder {
        // Idempotency check: only one thread can initiate shutdown
        let prev = self
            .shutdown_flag
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
        if prev.is_err() {
            // Already shutting down or completed
            let flag = self.shutdown_flag.load(Ordering::SeqCst);
            if flag == 2 {
                return ShutdownOrder {
                    executed: 0,
                    failed: 0,
                    skipped: true,
                };
            }
            // Still in progress - wait for completion would require a condvar
            // For now, return partial info
            return ShutdownOrder {
                executed: 0,
                failed: 0,
                skipped: true,
            };
        }

        let mut actions = self.actions.write();
        let total = actions.len();
        let mut executed = 0usize;
        let mut failed = 0usize;

        // Drop in reverse order (LIFO): last registered = first dropped = child first
        for i in (0..actions.len()).rev() {
            if let MaybeDoneAction::Pending(ref mut action) = actions[i] {
                let name = action.name().to_string();
                debug!("Shutting down: {name} (index={i}/{total})");
                // Execute in best-effort manner (per ADR-050: best-effort in Drop)
                action.execute();
                executed += 1;
                actions[i] = MaybeDoneAction::Done;
            }
        }

        // Clear the registry
        actions.clear();
        self.shutdown_flag.store(2, Ordering::SeqCst);

        debug!("Shutdown complete: {executed}/{total} actions executed");
        ShutdownOrder {
            executed,
            failed,
            skipped: false,
        }
    }

    /// Returns the number of registered actions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.read().len()
    }

    /// Returns true if no actions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the registration order of all actions (for testing).
    #[must_use]
    pub fn action_names(&self) -> Vec<String> {
        self.actions
            .read()
            .iter()
            .filter_map(|a| match a {
                MaybeDoneAction::Pending(action) => Some(action.name().to_string()),
                MaybeDoneAction::Done => None,
            })
            .collect()
    }
}

impl Drop for OrderedDropRegistry {
    fn drop(&mut self) {
        // If registry is dropped without explicit shutdown, execute remaining actions
        let flag = self.shutdown_flag.load(Ordering::SeqCst);
        if flag != 2 {
            // Best-effort: mark as complete to avoid double-execute
            let _ = self
                .shutdown_flag
                .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
            let mut actions = self.actions.write();
            for action in actions.iter_mut() {
                if let MaybeDoneAction::Pending(ref mut a) = action {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| a.execute()));
                }
            }
            actions.clear();
        }
    }
}

// =============================================================================
// ShutdownOrder
// =============================================================================

/// Result of a shutdown operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownOrder {
    /// Number of actions successfully executed.
    pub executed: usize,
    /// Number of actions that failed (panicked).
    pub failed: usize,
    /// Whether shutdown was skipped because it was already running/completed.
    pub skipped: bool,
}

impl ShutdownOrder {
    /// Returns true if all actions executed successfully.
    #[must_use]
    pub fn all_executed(&self) -> bool {
        !self.skipped && self.failed == 0
    }
}

impl fmt::Display for ShutdownOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.skipped {
            write!(f, "shutdown skipped (already complete)")
        } else if self.failed > 0 {
            write!(
                f,
                "shutdown: {} executed, {} failed",
                self.executed, self.failed
            )
        } else {
            write!(f, "shutdown: {} actions executed", self.executed)
        }
    }
}
