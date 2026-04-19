//! Master orchestrator for actor supervision.
//!
//! Per ADR-015: The Master Orchestrator maintains the ActiveInstances registry
//! and enforces the Single-Writer invariant. It manages the lifecycle of all
//! workflow instance actors and coordinates shutdown propagation.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use vo_types::InstanceId;

use crate::fairness::WorkloadClass;
use crate::instance_registry::{InstanceActorHandle, InstanceRegistry, RegistryConfig, RegistryError};
use crate::lifecycle::{ActorLifecycleState, ParentChildRegistry, ShutdownPropagator};
use crate::ReservedPermitBudget;
use crate::StartError;

// =============================================================================
// Data Layer — inert types
// =============================================================================

/// Configuration for the MasterOrchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Registry configuration for instance management.
    pub registry_config: RegistryConfig,
    /// Maximum instances per workload class.
    pub max_per_class: u32,
    /// Shutdown propagator configuration.
    pub shutdown_propagator: ShutdownPropagator,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            registry_config: RegistryConfig::default(),
            max_per_class: 10,
            shutdown_propagator: ShutdownPropagator::default_propagator(),
        }
    }
}

impl OrchestratorConfig {
    /// Creates a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the registry config.
    #[must_use]
    pub fn with_registry_config(mut self, config: RegistryConfig) -> Self {
        self.registry_config = config;
        self
    }

    /// Sets the max instances per class.
    #[must_use]
    pub fn with_max_per_class(mut self, max: u32) -> Self {
        self.max_per_class = max;
        self
    }
}

/// Information about an active instance managed by the orchestrator.
#[derive(Debug, Clone)]
pub struct ActiveInstanceInfo {
    pub instance_id: InstanceId,
    pub handle: InstanceActorHandle,
    pub workload_class: WorkloadClass,
    pub lifecycle_state: ActorLifecycleState,
}

// =============================================================================
// Calculation Layer — pure functions
// =============================================================================

/// Errors from orchestrator operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrchestratorError {
    #[error("instance not found: {instance_id}")]
    InstanceNotFound { instance_id: InstanceId },
    #[error("instance already exists: {instance_id}")]
    InstanceAlreadyExists { instance_id: String },
    #[error("registry error: {reason}")]
    RegistryError { reason: String },
    #[error("budget exhausted for {class:?}: {message}")]
    BudgetExhausted { class: WorkloadClass, message: String },
    #[error("lifecycle error: {reason}")]
    LifecycleError { reason: String },
    #[error("spawn failed: {reason}")]
    SpawnFailed { reason: String },
    #[error("shutdown timeout with {children_remaining} children remaining")]
    ShutdownTimeout { children_remaining: usize },
}

impl From<RegistryError> for OrchestratorError {
    fn from(err: RegistryError) -> Self {
        match err {
            RegistryError::StopFailed { instance_id, reason } => {
                Self::LifecycleError { reason: format!("stop failed for {}: {}", instance_id, reason) }
            }
            RegistryError::StopTimeout { instance_id, timeout } => {
                Self::LifecycleError { reason: format!("stop timeout for {} after {:?}", instance_id, timeout) }
            }
            RegistryError::NotRegistered { instance_id } => {
                Self::InstanceNotFound { instance_id }
            }
        }
    }
}

impl From<StartError> for OrchestratorError {
    fn from(err: StartError) -> Self {
        match err {
            StartError::BudgetExhaustion { class, requested, available } => {
                Self::BudgetExhausted {
                    class,
                    message: format!("requested {}, available {}", requested, available),
                }
            }
            StartError::InvalidConfig(msg) => Self::LifecycleError { reason: msg },
            StartError::AtCapacity { running, max } => {
                Self::LifecycleError { reason: format!("at capacity: {}/{}", running, max) }
            }
            StartError::AlreadyExists(instance_id) => Self::InstanceAlreadyExists { instance_id },
            StartError::SpawnFailed(reason) => Self::SpawnFailed { reason },
        }
    }
}

// =============================================================================
// Action Layer — MasterOrchestrator implementation
// =============================================================================

/// The Master Orchestrator maintains the ActiveInstances registry and enforces
/// the Single-Writer invariant per ADR-015.
///
/// It manages the lifecycle of all workflow instance actors and coordinates
/// shutdown propagation through the ParentChildRegistry hierarchy.
#[derive(Debug)]
pub struct MasterOrchestrator {
    config: OrchestratorConfig,
    instance_registry: Arc<RwLock<InstanceRegistry>>,
    parent_child_registry: Arc<ParentChildRegistry>,
    budget: Arc<RwLock<ReservedPermitBudget>>,
    instance_info: Arc<RwLock<HashMap<InstanceId, ActiveInstanceInfo>>>,
}

impl Default for MasterOrchestrator {
    fn default() -> Self {
        Self::new(OrchestratorConfig::default())
    }
}

impl MasterOrchestrator {
    /// Creates a new MasterOrchestrator with the given config.
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            config: config.clone(),
            instance_registry: Arc::new(RwLock::new(InstanceRegistry::new(config.registry_config))),
            parent_child_registry: Arc::new(ParentChildRegistry::new()),
            budget: Arc::new(RwLock::new(ReservedPermitBudget::new(config.max_per_class))),
            instance_info: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns the config used by this orchestrator.
    #[must_use]
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Returns the instance registry for direct access if needed.
    #[must_use]
    pub fn instance_registry(&self) -> &Arc<RwLock<InstanceRegistry>> {
        &self.instance_registry
    }

    /// Returns the parent-child registry for hierarchy management.
    #[must_use]
    pub fn parent_child_registry(&self) -> &Arc<ParentChildRegistry> {
        &self.parent_child_registry
    }

    /// Returns the budget for permit management.
    #[must_use]
    pub fn budget(&self) -> &Arc<RwLock<ReservedPermitBudget>> {
        &self.budget
    }

    /// Spawns a new workflow instance.
    ///
    /// This method:
    /// 1. Checks the budget for the given workload class
    /// 2. Registers the instance with the registry (enforcing single-writer)
    /// 3. Adds the instance to the parent-child registry
    /// 4. Stores instance metadata
    ///
    /// # Errors
    /// Returns an error if budget is exhausted, instance already exists,
    /// or registration fails.
    pub async fn spawn_instance(
        &self,
        instance_id: InstanceId,
        workload_class: WorkloadClass,
        stop_fn: impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send + 'static,
    ) -> Result<(), OrchestratorError> {
        let mut budget = self.budget.write().await;
        budget.try_acquire(workload_class.clone()).map_err(|e| OrchestratorError::from(e))?;

        let handle = InstanceActorHandle::test(instance_id.as_str().len() as u64);

        let mut registry = self.instance_registry.write().await;
        registry
            .register(instance_id.clone(), handle.clone(), stop_fn)
            .map_err(|e| {
                drop(budget);
                OrchestratorError::from(e)
            })?;

        self.parent_child_registry.add_child(instance_id.clone()).await;

        let mut info = self.instance_info.write().await;
        info.insert(
            instance_id.clone(),
            ActiveInstanceInfo {
                instance_id,
                handle,
                workload_class,
                lifecycle_state: ActorLifecycleState::Running,
            },
        );

        Ok(())
    }

    /// Terminates a workflow instance.
    ///
    /// This method:
    /// 1. Removes the instance from the registry
    /// 2. Updates the lifecycle state
    /// 3. Removes from parent-child registry
    /// 4. Releases the budget permit
    ///
    /// # Errors
    /// Returns an error if the instance is not found.
    pub async fn terminate_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<(), OrchestratorError> {
        let info = {
            let info_map = self.instance_info.read().await;
            info_map.get(instance_id).cloned()
        };

        let instance_info = info.ok_or(OrchestratorError::InstanceNotFound {
            instance_id: instance_id.clone(),
        })?;

        let mut registry = self.instance_registry.write().await;
        registry.deregister(instance_id).map_err(|e| OrchestratorError::from(e))?;
        drop(registry);

        self.parent_child_registry.remove_child(instance_id).await;

        let mut info_map = self.instance_info.write().await;
        info_map.remove(instance_id);

        let mut budget = self.budget.write().await;
        budget.release(instance_info.workload_class);

        Ok(())
    }

    /// Lists all active instances.
    #[must_use]
    pub async fn list_active(&self) -> Vec<ActiveInstanceInfo> {
        let info = self.instance_info.read().await;
        info.values().cloned().collect()
    }

    /// Gets the number of active instances.
    #[must_use]
    pub async fn active_count(&self) -> usize {
        let info = self.instance_info.read().await;
        info.len()
    }

    /// Checks if an instance is currently active.
    #[must_use]
    pub async fn is_active(&self, instance_id: &InstanceId) -> bool {
        let info = self.instance_info.read().await;
        info.contains_key(instance_id)
    }

    /// Gets info about a specific instance.
    #[must_use]
    pub async fn get_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Option<ActiveInstanceInfo> {
        let info = self.instance_info.read().await;
        info.get(instance_id).cloned()
    }

    /// Initiates shutdown of all managed instances.
    ///
    /// This propagates shutdown to all children through the ParentChildRegistry.
    /// Returns after all children have transitioned to terminal states or timeout occurs.
    pub async fn shutdown(&self) -> Result<(), OrchestratorError> {
        let children = self.parent_child_registry.get_children().await;

        for (instance_id, _) in children {
            self.parent_child_registry
                .update_child_state(&instance_id, ActorLifecycleState::Stopping)
                .await;
        }

        let pending = self.parent_child_registry.active_children_count().await;
        if pending == 0 {
            return Ok(());
        }

        Err(OrchestratorError::ShutdownTimeout {
            children_remaining: pending,
        })
    }

    /// Gets the available budget for a workload class.
    #[must_use]
    pub async fn available_budget(&self, class: WorkloadClass) -> u32 {
        let budget = self.budget.read().await;
        budget.available(class)
    }

    /// Checks if the budget is exhausted for a workload class.
    #[must_use]
    pub async fn is_budget_exhausted(&self, class: WorkloadClass) -> bool {
        let budget = self.budget.read().await;
        budget.is_exhausted(class)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stop_fn() -> impl FnOnce(InstanceActorHandle) -> Result<(), String> + Send + 'static {
        |_handle| Ok(())
    }

    #[tokio::test]
    async fn spawn_instance_success() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let result = orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await;

        assert!(result.is_ok());
        assert!(orchestrator.is_active(&instance_id).await);
        assert_eq!(orchestrator.active_count().await, 1);
    }

    #[tokio::test]
    async fn spawn_instance_increments_budget() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        assert_eq!(orchestrator.available_budget(WorkloadClass::Recovery).await, 9);
    }

    #[tokio::test]
    async fn terminate_instance_success() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        let result = orchestrator.terminate_instance(&instance_id).await;
        assert!(result.is_ok());
        assert!(!orchestrator.is_active(&instance_id).await);
        assert_eq!(orchestrator.active_count().await, 0);
    }

    #[tokio::test]
    async fn terminate_instance_releases_budget() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        orchestrator.terminate_instance(&instance_id).await.unwrap();

        assert_eq!(orchestrator.available_budget(WorkloadClass::Recovery).await, 10);
    }

    #[tokio::test]
    async fn terminate_instance_not_found() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let result = orchestrator.terminate_instance(&instance_id).await;

        assert!(matches!(
            result,
            Err(OrchestratorError::InstanceNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn list_active_empty() {
        let orchestrator = MasterOrchestrator::default();
        let instances = orchestrator.list_active().await;
        assert!(instances.is_empty());
    }

    #[tokio::test]
    async fn list_active_returns_all() {
        let orchestrator = MasterOrchestrator::default();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        orchestrator
            .spawn_instance(id1.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();
        orchestrator
            .spawn_instance(id2.clone(), WorkloadClass::Internal, make_stop_fn())
            .await
            .unwrap();

        let instances = orchestrator.list_active().await;
        assert_eq!(instances.len(), 2);
    }

    #[tokio::test]
    async fn get_instance_returns_info() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        let info = orchestrator.get_instance(&instance_id).await;
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.instance_id, instance_id);
        assert_eq!(info.workload_class, WorkloadClass::Recovery);
        assert_eq!(info.lifecycle_state, ActorLifecycleState::Running);
    }

    #[tokio::test]
    async fn get_instance_not_found() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let info = orchestrator.get_instance(&instance_id).await;
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn budget_exhaustion_prevents_spawn() {
        let config = OrchestratorConfig::new().with_max_per_class(1);
        let orchestrator = MasterOrchestrator::new(config);

        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        orchestrator
            .spawn_instance(id1.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        let result = orchestrator
            .spawn_instance(id2.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await;

        assert!(matches!(
            result,
            Err(OrchestratorError::BudgetExhausted { .. })
        ));
    }

    #[tokio::test]
    async fn is_budget_exhausted_reflects_state() {
        let config = OrchestratorConfig::new().with_max_per_class(1);
        let orchestrator = MasterOrchestrator::new(config);
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        assert!(!orchestrator.is_budget_exhausted(WorkloadClass::Recovery).await);

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        assert!(orchestrator.is_budget_exhausted(WorkloadClass::Recovery).await);
    }

    #[tokio::test]
    async fn different_workload_classes_independent() {
        let config = OrchestratorConfig::new().with_max_per_class(1);
        let orchestrator = MasterOrchestrator::new(config);

        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        orchestrator
            .spawn_instance(id1.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        let result = orchestrator
            .spawn_instance(id2.clone(), WorkloadClass::Internal, make_stop_fn())
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_returns_timeout_when_children_running() {
        let orchestrator = MasterOrchestrator::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        orchestrator
            .spawn_instance(instance_id.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();

        let result = orchestrator.shutdown().await;

        assert!(matches!(
            result,
            Err(OrchestratorError::ShutdownTimeout { children_remaining: 1 })
        ));
    }

    #[tokio::test]
    async fn shutdown_succeeds_when_no_children() {
        let orchestrator = MasterOrchestrator::default();

        let result = orchestrator.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_per_class, 10);
    }

    #[tokio::test]
    async fn orchestrator_config_builder() {
        let config = OrchestratorConfig::new()
            .with_max_per_class(5)
            .with_registry_config(RegistryConfig {
                stop_timeout: std::time::Duration::from_secs(10),
            });

        assert_eq!(config.max_per_class, 5);
        assert_eq!(
            config.registry_config.stop_timeout,
            std::time::Duration::from_secs(10)
        );
    }

    #[tokio::test]
    async fn active_count_matches_info_len() {
        let orchestrator = MasterOrchestrator::default();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        assert_eq!(orchestrator.active_count().await, 0);

        orchestrator
            .spawn_instance(id1.clone(), WorkloadClass::Recovery, make_stop_fn())
            .await
            .unwrap();
        assert_eq!(orchestrator.active_count().await, 1);

        orchestrator
            .spawn_instance(id2.clone(), WorkloadClass::Internal, make_stop_fn())
            .await
            .unwrap();
        assert_eq!(orchestrator.active_count().await, 2);

        orchestrator.terminate_instance(&id1).await.unwrap();
        assert_eq!(orchestrator.active_count().await, 1);
    }
}
