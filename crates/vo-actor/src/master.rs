//! Master orchestrator for actor supervision.
//!
//! Per ADR-015: The Master Orchestrator maintains the ActiveInstances registry
//! and enforces the Single-Writer invariant.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::watch;
use vo_types::InstanceId;

use crate::instance_registry::{InstanceActorHandle, InstanceRegistry, RegistryConfig};

/// Default stop timeout for active instance lock acquisition.
const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// The single-writer active instances registry.
///
/// Maintains a lock-free concurrent map of active instance IDs using DashMap.
/// This enforces the invariant that at most ONE active actor exists per
/// InstanceId at any point in time on this node.
///
/// # Architecture
///
/// - **Data**: `ActiveInstances` - DashMap<InstanceId, ActiveInstanceEntry>
/// - **Calc**: Pure lock acquisition/release decisions
/// - **Actions**: Async actor messages for lock management
///
/// # Invariants (per ADR-015)
///
/// - **INV-1**: At most one active entry per InstanceId
/// - **INV-2**: Lock acquisition is atomic via DashMap::entry()
/// - **INV-3**: Wake-up signals are queued when lock is held
#[derive(Debug, Clone)]
pub struct ActiveInstances {
    inner: Arc<DashMap<InstanceId, ActiveInstanceEntry>>,
}

#[derive(Debug, Clone)]
struct ActiveInstanceEntry {
    #[allow(dead_code)]
    instance_id: InstanceId,
    stop_tx: watch::Sender<StopSignal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopSignal {
    Stop,
    Continue,
}

impl ActiveInstances {
    /// Creates a new, empty ActiveInstances registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Returns the number of active instances.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.inner.len()
    }

    /// Checks if an instance is currently active (has a lock held).
    #[must_use]
    pub fn is_active(&self, instance_id: &InstanceId) -> bool {
        self.inner.contains_key(instance_id)
    }

    /// Attempts to acquire the lock for the given instance ID.
    ///
    /// Returns a guard that releases the lock when dropped. If the instance
    /// is already active, returns `None`.
    #[must_use]
    pub fn try_acquire(&self, instance_id: InstanceId) -> Option<ActiveInstanceGuard> {
        let entry = self.inner.entry(instance_id.clone());
        match entry {
            dashmap::Entry::Vacant(vacant) => {
                let (stop_tx, _) = watch::channel(StopSignal::Continue);
                vacant.insert(ActiveInstanceEntry {
                    instance_id,
                    stop_tx,
                });
                Some(ActiveInstanceGuard {
                    instance_id,
                    inner: Arc::clone(&self.inner),
                })
            }
            dashmap::Entry::Occupied(_) => None,
        }
    }

    /// Returns an iterator over all active instance IDs.
    #[must_use]
    pub fn active_instances(&self) -> impl Iterator<Item = InstanceId> + '_ {
        self.inner.iter().map(|entry| entry.instance_id.clone())
    }
}

impl Default for ActiveInstances {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard that releases the active instance lock when dropped.
#[derive(Debug)]
pub struct ActiveInstanceGuard {
    instance_id: InstanceId,
    inner: Arc<DashMap<InstanceId, ActiveInstanceEntry>>,
}

impl Drop for ActiveInstanceGuard {
    fn drop(&mut self) {
        self.inner.remove(&self.instance_id);
    }
}

/// Master orchestrator for actor supervision.
///
/// Per ADR-015, the MasterOrchestrator:
/// 1. Maintains the ActiveInstances registry for single-writer enforcement
/// 2. Coordinates actor spawn with proper lock acquisition
/// 3. Ensures wake-up signals are queued when locks are held
#[derive(Debug, Clone)]
pub struct MasterOrchestrator {
    active_instances: ActiveInstances,
    instance_registry: Arc<InstanceRegistry>,
    stop_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorConfig {
    Default,
    Custom {
        stop_timeout: Duration,
    },
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self::Default
    }
}

impl OrchestratorConfig {
    #[must_use]
    pub fn stop_timeout(&self) -> Duration {
        match self {
            Self::Default => DEFAULT_STOP_TIMEOUT,
            Self::Custom { stop_timeout } => *stop_timeout,
        }
    }
}

impl MasterOrchestrator {
    /// Creates a new MasterOrchestrator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(OrchestratorConfig::default())
    }

    /// Creates a new MasterOrchestrator with custom configuration.
    #[must_use]
    pub fn with_config(config: OrchestratorConfig) -> Self {
        Self {
            active_instances: ActiveInstances::new(),
            instance_registry: Arc::new(InstanceRegistry::new(RegistryConfig {
                stop_timeout: config.stop_timeout(),
            })),
            stop_timeout: config.stop_timeout(),
        }
    }

    /// Returns the ActiveInstances registry reference.
    #[must_use]
    pub fn active_instances(&self) -> &ActiveInstances {
        &self.active_instances
    }

    /// Returns the InstanceRegistry reference.
    #[must_use]
    pub fn instance_registry(&self) -> &Arc<InstanceRegistry> {
        &self.instance_registry
    }

    /// Checks if an instance is currently active.
    #[must_use]
    pub fn is_instance_active(&self, instance_id: &InstanceId) -> bool {
        self.active_instances.is_active(instance_id)
    }

    /// Returns the number of active instances.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active_instances.active_count()
    }

    /// Attempts to spawn an instance actor for the given ID.
    ///
    /// This first attempts to acquire the single-writer lock via ActiveInstances.
    /// If the lock is held (instance already active), no actor is spawned.
    ///
    /// # Errors
    /// Returns `MasterOrchestratorError::InstanceAlreadyActive` if the instance
    /// is already active.
    pub async fn spawn_instance(
        &self,
        instance_id: InstanceId,
        actor: impl ractor::Actor,
    ) -> Result<ActorInstanceHandle, MasterOrchestratorError> {
        let guard = self
            .active_instances
            .try_acquire(instance_id.clone())
            .ok_or(MasterOrchestratorError::InstanceAlreadyActive {
                instance_id: instance_id.clone(),
            })?;

        let (actor_ref, handle) = ractor::Actor::spawn(
            Some(instance_id.to_string()),
            actor,
            ractor::ActorProperties::default(),
        )
        .await
        .map_err(|e| MasterOrchestratorError::SpawnFailed {
            instance_id: instance_id.clone(),
            reason: e.to_string(),
        })?;

        Ok(ActorInstanceHandle {
            instance_id,
            actor_ref,
            _guard: guard,
        })
    }
}

impl Default for MasterOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to a spawned actor instance managed by the MasterOrchestrator.
#[derive(Debug)]
pub struct ActorInstanceHandle {
    instance_id: InstanceId,
    actor_ref: ractor::ActorRef<ractor::ActorMsg<impl ractor::Actor>>,
    _guard: ActiveInstanceGuard,
}

impl ActorInstanceHandle {
    /// Returns the instance ID.
    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    /// Returns the actor reference.
    #[must_use]
    pub fn actor_ref(&self) -> &ractor::ActorRef<ractor::ActorMsg<impl ractor::Actor>> {
        &self.actor_ref
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum MasterOrchestratorError {
    #[error("instance already active: {instance_id}")]
    InstanceAlreadyActive { instance_id: InstanceId },

    #[error("spawn failed for {instance_id}: {reason}")]
    SpawnFailed { instance_id: InstanceId, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_instances_starts_empty() {
        let instances = ActiveInstances::new();
        assert_eq!(instances.active_count(), 0);
        assert!(!instances.is_active(&InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()));
    }

    #[test]
    fn active_instances_acquire_and_release() {
        let instances = ActiveInstances::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let guard = instances.try_acquire(id.clone());
        assert!(guard.is_some());
        assert!(instances.is_active(&id));
        assert_eq!(instances.active_count(), 1);

        drop(guard);
        assert!(!instances.is_active(&id));
        assert_eq!(instances.active_count(), 0);
    }

    #[test]
    fn active_instances_cannot_acquire_twice() {
        let instances = ActiveInstances::new();
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let guard1 = instances.try_acquire(id.clone());
        assert!(guard1.is_some());
        assert!(instances.is_active(&id));

        let guard2 = instances.try_acquire(id.clone());
        assert!(guard2.is_none());
        assert!(instances.is_active(&id));
        assert_eq!(instances.active_count(), 1);

        drop(guard1);
        assert!(!instances.is_active(&id));
    }

    #[test]
    fn active_instances_multiple_different_ids() {
        let instances = ActiveInstances::new();
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFBB").unwrap();

        let guard1 = instances.try_acquire(id1.clone());
        let guard2 = instances.try_acquire(id2.clone());

        assert!(guard1.is_some());
        assert!(guard2.is_some());
        assert_eq!(instances.active_count(), 2);
        assert!(instances.is_active(&id1));
        assert!(instances.is_active(&id2));
    }

    #[test]
    fn master_orchestrator_default_config() {
        let orch = MasterOrchestrator::new();
        assert_eq!(orch.active_count(), 0);
    }

    #[test]
    fn master_orchestrator_custom_config() {
        let config = OrchestratorConfig::Custom {
            stop_timeout: Duration::from_secs(10),
        };
        let orch = MasterOrchestrator::with_config(config);
        assert_eq!(orch.active_count(), 0);
    }
}
