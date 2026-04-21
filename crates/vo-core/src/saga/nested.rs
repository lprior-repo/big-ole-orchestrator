//! Nested saga support for ADR-034.
//!
//! This module implements nested saga patterns where a saga can contain other sagas as steps.
//! When a parent saga fails, all nested sagas are compensated in reverse dependency order.
//!
//! ## Architecture
//!
//! ```text
//! ParentSaga
//!   ├── Step 1: Effect E1 → Compensation C1
//!   ├── Step 2: NestedSaga A
//!   │           ├── Effect A1 → Compensation CA1
//!   │           └── Effect A2 → Compensation CA2
//!   └── Step 3: Effect E3 → Compensation C3
//!
//! Compensation order on failure: C3 → CA2 → CA1 → C1
//! ```
//!
//! ## Key Concepts
//!
//! - **NestedSaga**: A saga that is a step within another saga
//! - **SagaHierarchy**: Parent-child relationship between sagas
//! - **Reverse Compensation Order**: Compensation executes in reverse order of registration
//! - **Dependency Resolution**: Nested sagas must complete before dependent steps can execute

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use vo_types::{CompensationPolicy, TimestampMs};

use crate::compensation_saga::{
    CompensationEntry, CompensationError, CompensationManifest, CompensationSaga,
    ReconciliationAction, SagaCompensationStatus,
};

/// A nested saga that is a step within a parent saga.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedSaga {
    /// Unique identifier for this nested saga.
    pub id: String,
    /// The nested saga instance.
    pub saga: Arc<Mutex<CompensationSaga>>,
    /// Dependencies: other nested sagas that must complete first.
    pub dependencies: Vec<String>,
    /// Whether this nested saga has been executed.
    pub executed: bool,
    /// Whether this nested saga has been compensated.
    pub compensated: bool,
}

impl NestedSaga {
    #[must_use]
    pub fn new(id: String, saga: CompensationSaga, dependencies: Vec<String>) -> Self {
        Self {
            id,
            saga: Arc::new(Mutex::new(saga)),
            dependencies,
            executed: false,
            compensated: false,
        }
    }

    #[must_use]
    pub fn with_dependencies(id: String, saga: CompensationSaga, dependencies: Vec<String>) -> Self {
        Self {
            id,
            saga: Arc::new(Mutex::new(saga)),
            dependencies,
            executed: false,
            compensated: false,
        }
    }
}

/// A saga that can contain nested sagas as steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchicalSaga {
    /// Root saga for top-level effects.
    pub root_saga: CompensationSaga,
    /// Nested sagas that are steps within this saga.
    pub nested_sagas: HashMap<String, NestedSaga>,
    /// Order in which nested sagas were registered.
    pub nested_registration_order: Vec<String>,
    /// Version counter for the hierarchy.
    pub version: u64,
}

impl Default for HierarchicalSaga {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchicalSaga {
    /// Create a new hierarchical saga with an empty root saga.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root_saga: CompensationSaga::new(),
            nested_sagas: HashMap::new(),
            nested_registration_order: Vec::new(),
            version: 0,
        }
    }

    /// Register a nested saga as a step in this hierarchy.
    ///
    /// # Errors
    ///
    /// Returns `CompensationError::AlreadyRegistered` if a nested saga with this ID already exists.
    pub fn register_nested_saga(
        &mut self,
        id: String,
        nested_saga: CompensationSaga,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationError> {
        if self.nested_sagas.contains_key(&id) {
            return Err(CompensationError::AlreadyRegistered(id.clone()));
        }

        let nested = NestedSaga::new(id.clone(), nested_saga, dependencies);
        self.nested_sagas.insert(id.clone(), nested);
        self.nested_registration_order.push(id);
        self.version += 1;
        Ok(())
    }

    /// Register an effect in the root saga.
    ///
    /// # Errors
    ///
    /// Returns `CompensationError::AlreadyRegistered` if the effect is already registered.
    pub fn register_effect(
        &self,
        effect_id: String,
        policy: CompensationPolicy,
        dependencies: Vec<String>,
    ) -> Result<(), CompensationError> {
        self.root_saga.register(effect_id, policy, dependencies)
    }

    /// Check if all dependencies for a nested saga are satisfied.
    #[must_use]
    pub fn dependencies_satisfied(&self, nested_id: &str) -> bool {
        if let Some(nested) = self.nested_sagas.get(nested_id) {
            for dep_id in &nested.dependencies {
                if let Some(dep) = self.nested_sagas.get(dep_id) {
                    // Dependency must be executed and not compensated (still in progress or succeeded)
                    if !dep.executed {
                        return false;
                    }
                } else {
                    // Dependency might be a root effect - check if it's terminal
                    if let Some(entry) = self.root_saga.manifest().lock().unwrap().get(dep_id) {
                        if !entry.is_terminal() {
                            return false;
                        }
                    }
                }
            }
            return true;
        }
        false
    }

    /// Get the execution order for nested sagas (respecting dependencies).
    ///
    /// Returns sagas in an order where dependencies come before dependents.
    #[must_use]
    pub fn get_execution_order(&self) -> Vec<String> {
        let mut executed: HashSet<String> = HashSet::new();
        let mut order: Vec<String> = Vec::new();

        // Simple topological sort
        loop {
            let mut progress = false;

            for nested_id in &self.nested_registration_order {
                if executed.contains(nested_id) {
                    continue;
                }

                if let Some(nested) = self.nested_sagas.get(nested_id) {
                    let deps_satisfied = nested
                        .dependencies
                        .iter()
                        .all(|dep| executed.contains(dep) || !self.nested_sagas.contains_key(dep));

                    if deps_satisfied {
                        order.push(nested_id.clone());
                        executed.insert(nested_id.clone());
                        progress = true;
                    }
                }
            }

            if !progress {
                break;
            }
        }

        order
    }

    /// Get the compensation order for nested sagas (reverse of execution order).
    ///
    /// Compensation executes in reverse order of registration, respecting dependencies.
    #[must_use]
    pub fn get_compensation_order(&self) -> Vec<String> {
        let execution_order = self.get_execution_order();
        execution_order.into_iter().rev().collect()
    }

    /// Execute all nested sagas in dependency order.
    ///
    /// # Errors
    ///
    /// Returns `CompensationError` if any nested saga execution fails.
    pub fn execute_nested_sagas(&self) -> Result<Vec<String>, CompensationError> {
        let mut executed_ids = Vec::new();
        let execution_order = self.get_execution_order();

        for nested_id in execution_order {
            if let Some(nested) = self.nested_sagas.get(&nested_id) {
                let mut nested_saga = nested.saga.lock().unwrap();

                // Get compensation order for nested saga
                let order = nested_saga.get_compensation_order();
                for effect_id in order {
                    nested_saga.queue_pending(&effect_id)?;
                    nested_saga.start_compensation(&effect_id)?;
                    nested_saga.succeed(&effect_id)?;
                }

                nested.executed = true;
                drop(nested_saga);
                executed_ids.push(nested_id.clone());
            }
        }

        Ok(executed_ids)
    }

    /// Compensate all nested sagas in reverse dependency order.
    ///
    /// # Errors
    ///
    /// Returns `CompensationError` if any compensation fails.
    pub fn compensate_nested_sagas(&self) -> Result<Vec<String>, CompensationError> {
        let mut compensated_ids = Vec::new();
        let compensation_order = self.get_compensation_order();

        for nested_id in compensation_order {
            if let Some(nested) = self.nested_sagas.get(&nested_id) {
                if nested.compensated {
                    continue;
                }

                let mut nested_saga = nested.saga.lock().unwrap();

                let order = nested_saga.get_compensation_order();
                for effect_id in order {
                    nested_saga.queue_pending(&effect_id)?;
                    nested_saga.start_compensation(&effect_id)?;
                    nested_saga.succeed(&effect_id)?;
                }

                nested.compensated = true;
                drop(nested_saga);
                compensated_ids.push(nested_id.clone());
            }
        }

        Ok(compensated_ids)
    }

    /// Get all effect IDs that need compensation (root + nested).
    ///
    /// Returns effects in reverse registration order.
    #[must_use]
    pub fn get_all_effects_needing_compensation(&self) -> Vec<String> {
        let mut all_effects = Vec::new();

        // Add root saga effects
        let root_manifest = self.root_saga.manifest().lock().unwrap();
        let root_order: Vec<String> = root_manifest
            .registration_order
            .iter()
            .rev()
            .cloned()
            .collect();
        all_effects.extend(root_order);

        // Add nested saga effects in compensation order
        for nested_id in &self.nested_registration_order {
            if let Some(nested) = self.nested_sagas.get(nested_id) {
                let nested_manifest = nested.saga.lock().unwrap();
                let nested_order: Vec<String> = nested_manifest
                    .registration_order
                    .iter()
                    .rev()
                    .cloned()
                    .collect();
                all_effects.extend(nested_order);
            }
        }

        all_effects
    }

    /// Get compensation order for all effects (root + nested).
    ///
    /// Compensation order: nested sagas first (in reverse), then root effects (in reverse).
    #[must_use]
    pub fn get_full_compensation_order(&self) -> Vec<String> {
        let mut order = Vec::new();

        // Compensate nested sagas first (in reverse order)
        for nested_id in self.get_compensation_order() {
            if let Some(nested) = self.nested_sagas.get(&nested_id) {
                let nested_manifest = nested.saga.lock().unwrap();
                let nested_order: Vec<String> = nested_manifest
                    .registration_order
                    .iter()
                    .rev()
                    .cloned()
                    .collect();
                order.extend(nested_order);
            }
        }

        // Then compensate root effects (in reverse order)
        let root_manifest = self.root_saga.manifest().lock().unwrap();
        let root_order: Vec<String> = root_manifest
            .registration_order
            .iter()
            .rev()
            .cloned()
            .collect();
        order.extend(root_order);

        order
    }

    /// Check if all nested sagas have been compensated.
    #[must_use]
    pub fn all_nested_compensated(&self) -> bool {
        self.nested_sagas.values().all(|ns| ns.compensated)
    }

    /// Reset compensation state for re-execution.
    pub fn reset_compensation_state(&mut self) {
        for nested in self.nested_sagas.values_mut() {
            nested.compensated = false;
            nested.executed = false;
        }
    }

    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_nested_saga() {
        let mut hierarchy = HierarchicalSaga::new();
        let nested_saga = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-1".to_string(), nested_saga, vec![])
            .unwrap();

        assert!(hierarchy.nested_sagas.contains_key("nested-1"));
        assert_eq!(hierarchy.nested_registration_order.len(), 1);
    }

    #[test]
    fn test_register_duplicate_nested_fails() {
        let mut hierarchy = HierarchicalSaga::new();
        let nested_saga = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-1".to_string(), nested_saga.clone(), vec![])
            .unwrap();

        let result = hierarchy
            .register_nested_saga("nested-1".to_string(), nested_saga, vec![]);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CompensationError::AlreadyRegistered(_)
        ));
    }

    #[test]
    fn test_nested_saga_dependencies() {
        let mut hierarchy = HierarchicalSaga::new();

        let nested_a = CompensationSaga::new();
        let nested_b = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-a".to_string(), nested_a, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga(
                "nested-b".to_string(),
                nested_b,
                vec!["nested-a".to_string()],
            )
            .unwrap();

        assert!(hierarchy.dependencies_satisfied("nested-a"));
        assert!(!hierarchy.dependencies_satisfied("nested-b"));
    }

    #[test]
    fn test_get_execution_order_respects_dependencies() {
        let mut hierarchy = HierarchicalSaga::new();

        let nested_a = CompensationSaga::new();
        let nested_b = CompensationSaga::new();
        let nested_c = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-c".to_string(), nested_c, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga("nested-a".to_string(), nested_a, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga(
                "nested-b".to_string(),
                nested_b,
                vec!["nested-a".to_string(), "nested-c".to_string()],
            )
            .unwrap();

        let execution_order = hierarchy.get_execution_order();
        assert!(
            execution_order.iter().position(|id| id == "nested-a")
                < execution_order.iter().position(|id| id == "nested-b")
        );
        assert!(
            execution_order.iter().position(|id| id == "nested-c")
                < execution_order.iter().position(|id| id == "nested-b")
        );
    }

    #[test]
    fn test_get_compensation_order_is_reverse() {
        let mut hierarchy = HierarchicalSaga::new();

        let nested_a = CompensationSaga::new();
        let nested_b = CompensationSaga::new();
        let nested_c = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-a".to_string(), nested_a, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga("nested-b".to_string(), nested_b, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga("nested-c".to_string(), nested_c, vec![])
            .unwrap();

        let execution_order = hierarchy.get_execution_order();
        let compensation_order = hierarchy.get_compensation_order();

        assert_eq!(
            execution_order,
            compensation_order.into_iter().rev().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_nested_saga_with_root_effects() {
        let mut hierarchy = HierarchicalSaga::new();

        // Register root effects
        hierarchy
            .register_effect("root-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        hierarchy
            .register_effect("root-2".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        // Register nested saga
        let nested = CompensationSaga::new();
        nested
            .register("nested-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        hierarchy
            .register_nested_saga("nested-1".to_string(), nested, vec![])
            .unwrap();

        let all_effects = hierarchy.get_all_effects_needing_compensation();
        assert_eq!(all_effects.len(), 3);
    }

    #[test]
    fn test_full_compensation_order() {
        let mut hierarchy = HierarchicalSaga::new();

        // Register root effects
        hierarchy
            .register_effect("root-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();
        hierarchy
            .register_effect("root-2".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        // Register nested saga
        let nested = CompensationSaga::new();
        nested
            .register("nested-1".to_string(), CompensationPolicy::Automatic, vec![])
            .unwrap();

        hierarchy
            .register_nested_saga("nested-1".to_string(), nested, vec![])
            .unwrap();

        let order = hierarchy.get_full_compensation_order();
        // Nested should come before root (reverse order)
        let nested_idx = order.iter().position(|id| id == "nested-1");
        let root_2_idx = order.iter().position(|id| id == "root-2");
        let root_1_idx = order.iter().position(|id| id == "root-1");

        assert!(nested_idx.is_some());
        assert!(root_2_idx.is_some());
        assert!(root_1_idx.is_some());
        assert!(nested_idx.unwrap() < root_2_idx.unwrap());
        assert!(root_2_idx.unwrap() < root_1_idx.unwrap());
    }

    #[test]
    fn test_all_nested_compensated() {
        let mut hierarchy = HierarchicalSaga::new();

        let nested_a = CompensationSaga::new();
        let nested_b = CompensationSaga::new();

        hierarchy
            .register_nested_saga("nested-a".to_string(), nested_a, vec![])
            .unwrap();
        hierarchy
            .register_nested_saga("nested-b".to_string(), nested_b, vec![])
            .unwrap();

        assert!(!hierarchy.all_nested_compensated());

        // Manually mark as compensated
        for nested in hierarchy.nested_sagas.values_mut() {
            nested.compensated = true;
        }

        assert!(hierarchy.all_nested_compensated());
    }

    #[test]
    fn test_reset_compensation_state() {
        let mut hierarchy = HierarchicalSaga::new();

        let nested = CompensationSaga::new();
        hierarchy
            .register_nested_saga("nested-1".to_string(), nested, vec![])
            .unwrap();

        // Mark as compensated
        for nested in hierarchy.nested_sagas.values_mut() {
            nested.compensated = true;
        }

        assert!(hierarchy.all_nested_compensated());

        hierarchy.reset_compensation_state();

        assert!(!hierarchy.all_nested_compensated());
    }
}
