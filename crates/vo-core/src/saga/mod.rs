//! Compensation registry for forward-effect to compensation-effect linkage (ADR-034).
//!
//! This module implements the compensation registry that tracks committed forward effects
//! and their associated compensation effects. It provides:
//!
//! - **Compensation Registration**: Register compensations when forward effects are committed
//! - **Effect Linking**: Link forward effect IDs to their compensation effect IDs
//! - **Execution Ordering**: Compensations execute in reverse dependency order
//! - **Policy Enforcement**: Enforce compensation policy (None, Manual, Automatic)
//!
//! ## Architecture
//!
//! ```text
//! Forward Effect E1 commits → Compensation C1 registered
//! Forward Effect E2 commits → Compensation C2 registered
//! Rollback triggered → Compensations execute in reverse order: C2 → C1
//! ```
//!
//! ## Lifecycle
//!
//! Each compensation goes through:
//! 1. `Registered` - Compensation registered, awaiting execution
//! 2. `Pending` - Queued for execution (Automatic policy)
//! 3. `InProgress` - Compensation executing
//! 4. `Succeeded` - Compensation completed successfully (terminal)
//! 5. `Failed` - Compensation failed (terminal)
//! 6. `Ambiguous` - Outcome unclear, requires reconciliation
//!
//! ## Design Principles
//!
//! - **Functional Core**: Pure data structures and functions, no I/O
//! - **Zero Panics**: All operations return `Result<T, Error>`
//! - **Type Safety**: Illegal states are unrepresentable via the type system
//! - **Thread Safety**: Uses `Arc<Mutex<>>` for shared mutable state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vo_types::{CompensationPolicy, CompensationStatus, TimestampMs};

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Status of a compensation in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompensationRegistryStatus {
    Registered,
    Pending,
    InProgress,
    Succeeded,
    Failed,
    Ambiguous,
    TimedOut,
}

impl From<CompensationStatus> for CompensationRegistryStatus {
    fn from(status: CompensationStatus) -> Self {
        match status {
            CompensationStatus::NotNeeded => CompensationRegistryStatus::Succeeded,
            CompensationStatus::Pending => CompensationRegistryStatus::Pending,
            CompensationStatus::InProgress => CompensationRegistryStatus::InProgress,
            CompensationStatus::Succeeded => CompensationRegistryStatus::Succeeded,
            CompensationStatus::Failed => CompensationRegistryStatus::Failed,
        }
    }
}

/// Entry in the compensation registry for a committed forward effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationEntry {
    /// ID of the forward effect that requires compensation.
    pub effect_id: String,
    /// ID of the compensation effect (assigned when compensation is created).
    pub compensation_effect_id: Option<String>,
    /// Compensation policy for this effect.
    pub policy: CompensationPolicy,
    /// Current status of the compensation.
    pub status: CompensationRegistryStatus,
    /// Timestamp when compensation was registered.
    pub registered_at: TimestampMs,
    /// Timestamp when compensation started executing (if in progress).
    pub started_at: Option<TimestampMs>,
    /// Timestamp when compensation completed (terminal state).
    pub completed_at: Option<TimestampMs>,
    /// Timeout in milliseconds for compensation execution.
    pub timeout_ms: Option<u64>,
    /// Dependencies: other effect IDs that must complete compensation first.
    pub dependencies: Vec<String>,
}

impl CompensationEntry {
    /// Create a new compensation entry.
    #[must_use]
    pub fn new(effect_id: String, policy: CompensationPolicy, dependencies: Vec<String>) -> Self {
        Self {
            effect_id,
            compensation_effect_id: None,
            policy,
            status: CompensationRegistryStatus::Registered,
            registered_at: TimestampMs::now(),
            started_at: None,
            completed_at: None,
            timeout_ms: None,
            dependencies,
        }
    }

    /// Set timeout for compensation execution.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set the compensation effect ID.
    #[must_use]
    pub fn with_compensation_effect_id(mut self, compensation_effect_id: String) -> Self {
        self.compensation_effect_id = Some(compensation_effect_id);
        self
    }

    /// Check if this entry is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            CompensationRegistryStatus::Succeeded
                | CompensationRegistryStatus::Failed
                | CompensationRegistryStatus::TimedOut
        )
    }

    /// Check if this entry has timed out.
    #[must_use]
    pub fn is_timed_out(&self, now: TimestampMs) -> bool {
        if let (Some(started), Some(timeout)) = (self.started_at, self.timeout_ms) {
            let elapsed = now.as_u64().saturating_sub(started.as_u64());
            return elapsed > timeout;
        }
        false
    }
}

/// Registry of compensation entries for committed forward effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationRegistry {
    entries: HashMap<String, CompensationEntry>,
    registration_order: Vec<String>,
    version: u64,
}

impl Default for CompensationRegistry {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            registration_order: Vec::new(),
            version: 0,
        }
    }
}

impl CompensationRegistry {
    /// Create a new empty compensation registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a compensation for a committed forward effect.
    ///
    /// # Errors
    ///
    /// Returns `CompensationRegistryError::AlreadyRegistered` if a compensation
    /// is already registered for this effect ID.
    pub fn register(
        &mut self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationRegistryError> {
        if self.entries.contains_key(&effect_id) {
            return Err(CompensationRegistryError::AlreadyRegistered(effect_id));
        }
        let entry = CompensationEntry::new(effect_id.clone(), policy, dependencies);
        self.entries.insert(effect_id.clone(), entry);
        self.registration_order.push(effect_id);
        self.version += 1;
        Ok(())
    }

    /// Get a compensation entry by effect ID.
    #[must_use]
    pub fn get(&self, effect_id: &str) -> Option<&CompensationEntry> {
        self.entries.get(effect_id)
    }

    /// Get a mutable compensation entry by effect ID.
    #[must_use]
    pub fn get_mut(&mut self, effect_id: &str) -> Option<&mut CompensationEntry> {
        self.entries.get_mut(effect_id)
    }

    /// Transition a compensation to a new status.
    ///
    /// # Errors
    ///
    /// Returns `CompensationRegistryError::NotFound` if the effect ID is not found.
    /// Returns `CompensationRegistryError::TerminalState` if the compensation
    /// is already in a terminal state.
    pub fn transition_to(
        &mut self,
        effect_id: &str,
        new_status: CompensationRegistryStatus,
    ) -> Result<(), CompensationRegistryError> {
        let entry = self
            .entries
            .get_mut(effect_id)
            .ok_or_else(|| CompensationRegistryError::NotFound(effect_id.to_string()))?;

        if entry.is_terminal() {
            return Err(CompensationRegistryError::TerminalState {
                effect_id: effect_id.to_string(),
                status: entry.status,
            });
        }

        entry.status = new_status;

        if new_status == CompensationRegistryStatus::InProgress {
            entry.started_at = Some(TimestampMs::now());
        }

        if matches!(
            new_status,
            CompensationRegistryStatus::Succeeded | CompensationRegistryStatus::Failed
        ) {
            entry.completed_at = Some(TimestampMs::now());
        }

        self.version += 1;
        Ok(())
    }

    /// Mark a compensation as ambiguous (requires reconciliation).
    pub fn set_ambiguous(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        self.transition_to(effect_id, CompensationRegistryStatus::Ambiguous)
    }

    /// Mark a compensation as timed out.
    pub fn set_timed_out(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        self.transition_to(effect_id, CompensationRegistryStatus::TimedOut)
    }

    /// Get all pending compensations.
    pub fn pending_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == CompensationRegistryStatus::Pending)
    }

    /// Get all in-progress compensations.
    pub fn in_progress_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == CompensationRegistryStatus::InProgress)
    }

    /// Get all ambiguous compensations.
    pub fn ambiguous_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == CompensationRegistryStatus::Ambiguous)
    }

    /// Get all timed-out compensations.
    pub fn timed_out_compensations(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries
            .values()
            .filter(|e| e.status == CompensationRegistryStatus::TimedOut)
    }

    /// Get compensations awaiting execution in reverse registration order.
    #[must_use]
    pub fn compensations_awaiting_execution(&self) -> Vec<&CompensationEntry> {
        self.registration_order
            .iter()
            .rev()
            .filter_map(|id| {
                let entry = self.entries.get(id)?;
                if entry.status == CompensationRegistryStatus::Pending {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all entries.
    pub fn all_entries(&self) -> impl Iterator<Item = &CompensationEntry> {
        self.entries.values()
    }

    /// Get the current version number.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Register a compensation when a forward effect is committed.
    ///
    /// This is the core linkage logic that implements the requirement:
    /// "WHEN a forward effect is marked Committed, THE SYSTEM SHALL register
    /// its compensation effect in the saga state."
    ///
    /// # Arguments
    ///
    /// * `effect_id` - The ID of the forward effect that was just committed
    /// * `policy` - The compensation policy for this effect
    /// * `compensation_effect_id` - The ID of the compensation effect to link
    /// * `dependencies` - Other effect IDs that must compensate first
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the compensation was successfully registered
    /// * `Err(CompensationRegistryError::AlreadyRegistered)` if a compensation
    ///   is already registered for this effect
    ///
    /// # Design Principles
    ///
    /// - **Invariant**: Number of registered compensations = number of committed
    ///   reversible effects (ADR-034)
    /// - **No Panics**: Returns Result with specific error types
    /// - **Idempotent**: If compensation already registered, returns error rather
    ///   than overwriting
    pub fn register_compensation_for_committed_effect(
        &mut self,
        effect_id: String,
        policy: CompensationPolicy,
        compensation_effect_id: String,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationRegistryError> {
        // Register the compensation entry
        self.register(effect_id.clone(), policy, dependencies)?;

        // Link the compensation effect ID to the entry
        if let Some(entry) = self.entries.get_mut(&effect_id) {
            entry.compensation_effect_id = Some(compensation_effect_id);
        }

        Ok(())
    }

    /// Get the linked compensation effect ID for a committed forward effect.
    ///
    /// Returns `None` if:
    /// - No compensation is registered for this effect
    /// - The policy is `None` (irreversible effect)
    #[must_use]
    pub fn get_compensation_effect_id(&self, effect_id: &str) -> Option<&String> {
        self.entries
            .get(effect_id)
            .and_then(|entry| entry.compensation_effect_id.as_ref())
    }

    /// Check if a forward effect has a registered compensation.
    #[must_use]
    pub fn has_compensation(&self, effect_id: &str) -> bool {
        self.entries
            .get(effect_id)
            .map(|entry| entry.compensation_effect_id.is_some())
            .unwrap_or(false)
    }

    /// Check if a forward effect has an irreversible policy (no compensation).
    #[must_use]
    pub fn is_irreversible(&self, effect_id: &str) -> bool {
        self.entries
            .get(effect_id)
            .map(|entry| entry.policy == CompensationPolicy::None)
            .unwrap_or(false)
    }

    /// Get all compensations that can be executed (pending with satisfied dependencies).
    #[must_use]
    pub fn compensations_ready_to_execute(&self) -> Vec<String> {
        self.registration_order
            .iter()
            .rev()
            .filter_map(|id| {
                let entry = self.entries.get(id)?;
                if entry.status == CompensationRegistryStatus::Pending && self.can_execute(id) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if a compensation can execute (dependencies satisfied).
    #[must_use]
    pub fn can_execute(&self, effect_id: &str) -> bool {
        if let Some(entry) = self.entries.get(effect_id) {
            if entry.status != CompensationRegistryStatus::Pending {
                return false;
            }
            for dep in &entry.dependencies {
                if let Some(dep_entry) = self.entries.get(dep) {
                    if !dep_entry.is_terminal() {
                        return false;
                    }
                }
            }
            return true;
        }
        false
    }

    /// Get the compensation execution order (reverse registration order).
    #[must_use]
    pub fn get_compensation_order(&self) -> Vec<String> {
        self.registration_order
            .iter()
            .rev()
            .filter(|id| {
                self.entries
                    .get(*id)
                    .map(|e| e.status == CompensationRegistryStatus::Pending)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    /// Queue a compensation for execution (transition to Pending).
    ///
    /// # Errors
    ///
    /// Returns `CompensationRegistryError::NotFound` if the effect ID is not found.
    /// Returns `CompensationRegistryError::PolicyViolation` if the policy is `None`.
    pub fn queue_pending(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        let entry = self
            .entries
            .get_mut(effect_id)
            .ok_or_else(|| CompensationRegistryError::NotFound(effect_id.to_string()))?;

        if entry.policy == CompensationPolicy::None {
            return Err(CompensationRegistryError::PolicyViolation {
                effect_id: effect_id.to_string(),
                policy: entry.policy,
            });
        }

        self.transition_to(effect_id, CompensationRegistryStatus::Pending)
    }

    /// Start executing a compensation.
    ///
    /// # Errors
    ///
    /// Returns `CompensationRegistryError::PolicyViolation` if the compensation
    /// cannot execute (dependencies not satisfied or policy violation).
    pub fn start_compensation(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        if !self.can_execute(effect_id) {
            return Err(CompensationRegistryError::PolicyViolation {
                effect_id: effect_id.to_string(),
                policy: self
                    .get(effect_id)
                    .map(|e| e.policy)
                    .unwrap_or(CompensationPolicy::None),
            });
        }
        self.transition_to(effect_id, CompensationRegistryStatus::InProgress)
    }

    /// Mark a compensation as succeeded.
    pub fn succeed(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        self.transition_to(effect_id, CompensationRegistryStatus::Succeeded)
    }

    /// Mark a compensation as failed.
    pub fn fail(&mut self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        self.transition_to(effect_id, CompensationRegistryStatus::Failed)
    }

    /// Check for timed-out compensations.
    #[must_use]
    pub fn check_timeouts(&self, now: TimestampMs) -> Vec<String> {
        let mut timed_out = Vec::new();

        for entry in self.in_progress_compensations() {
            if entry.is_timed_out(now) {
                timed_out.push(entry.effect_id.clone());
            }
        }

        timed_out
    }

    /// Expire all timed-out compensations.
    pub fn expire_timed_out(&mut self) -> Result<(), CompensationRegistryError> {
        let now = TimestampMs::now();
        let timed_out = self.check_timeouts(now);
        for effect_id in timed_out {
            self.set_timed_out(&effect_id)?;
        }
        Ok(())
    }
}

/// Error type for compensation registry operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensationRegistryError {
    #[error("compensation already registered for: {0}")]
    AlreadyRegistered(String),

    #[error("compensation not found for: {0}")]
    NotFound(String),

    #[error("effect {effect_id} is in terminal state: {status:?}")]
    TerminalState {
        effect_id: String,
        status: CompensationRegistryStatus,
    },

    #[error("invalid transition for {effect_id}: {from:?} -> {to:?}")]
    InvalidTransition {
        effect_id: String,
        from: CompensationRegistryStatus,
        to: CompensationRegistryStatus,
    },

    #[error("policy violation for {effect_id}: {policy:?}")]
    PolicyViolation {
        effect_id: String,
        policy: CompensationPolicy,
    },

    #[error("compensation timeout for: {effect_id}")]
    Timeout { effect_id: String },
}

// ============================================================================
// Shared Compensation Registry (Thread-Safe)
// ============================================================================

/// Thread-safe shared compensation registry.
#[derive(Debug, Clone)]
pub struct SharedCompensationRegistry {
    registry: Arc<Mutex<CompensationRegistry>>,
}

impl SharedCompensationRegistry {
    /// Create a new shared compensation registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(CompensationRegistry::new())),
        }
    }

    /// Register a compensation for a committed forward effect.
    pub fn register(
        &self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.register(effect_id, policy, dependencies)
    }

    /// Get a compensation entry by effect ID.
    #[must_use]
    pub fn get(&self, effect_id: &str) -> Option<CompensationEntry> {
        #[expect(clippy::unwrap_used)]
        let registry = self.registry.lock().unwrap();
        registry.get(effect_id).cloned()
    }

    /// Queue a compensation for execution.
    pub fn queue_pending(&self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.queue_pending(effect_id)
    }

    /// Start executing a compensation.
    pub fn start_compensation(&self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.start_compensation(effect_id)
    }

    /// Mark a compensation as succeeded.
    pub fn succeed(&self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.succeed(effect_id)
    }

    /// Mark a compensation as failed.
    pub fn fail(&self, effect_id: &str) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.fail(effect_id)
    }

    /// Get the compensation execution order.
    #[must_use]
    pub fn get_compensation_order(&self) -> Vec<String> {
        #[expect(clippy::unwrap_used)]
        let registry = self.registry.lock().unwrap();
        registry.get_compensation_order()
    }

    /// Check for timed-out compensations.
    #[must_use]
    pub fn check_timeouts(&self) -> Vec<String> {
        #[expect(clippy::unwrap_used)]
        let registry = self.registry.lock().unwrap();
        registry.check_timeouts(TimestampMs::now())
    }

    /// Expire all timed-out compensations.
    pub fn expire_timed_out(&self) -> Result<(), CompensationRegistryError> {
        #[expect(clippy::unwrap_used)]
        let mut registry = self.registry.lock().unwrap();
        registry.expire_timed_out()
    }

    /// Get the current version.
    #[must_use]
    pub fn version(&self) -> u64 {
        #[expect(clippy::unwrap_used)]
        let registry = self.registry.lock().unwrap();
        registry.version()
    }
}

impl Default for SharedCompensationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Module Exports
// ============================================================================

#[cfg(test)]
mod tests;
