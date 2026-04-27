//! Instance actor module.
//!
//! This module provides the InstanceActor struct which wraps an actor reference
//! and provides workflow instance management capabilities.

use std::sync::{Arc, Mutex};
use vo_types::InstanceId;

use crate::instance_registry::{InstanceActorHandle, InstanceRegistry, RegistryConfig};
use crate::lifecycle::ActorLifecycleState;
use crate::InstancePhaseView;

#[derive(Debug, Clone)]
pub struct InstanceActor {
    instance_id: InstanceId,
    lifecycle_state: ActorLifecycleState,
}

impl InstanceActor {
    pub fn new(instance_id: InstanceId) -> Self {
        Self {
            instance_id,
            lifecycle_state: ActorLifecycleState::Pending,
        }
    }

    pub fn with_lifecycle(instance_id: InstanceId, lifecycle_state: ActorLifecycleState) -> Self {
        Self {
            instance_id,
            lifecycle_state,
        }
    }

    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    pub fn lifecycle_state(&self) -> ActorLifecycleState {
        self.lifecycle_state
    }

    pub fn set_lifecycle_state(&mut self, state: ActorLifecycleState) {
        self.lifecycle_state = state;
    }

    pub fn phase(&self) -> InstancePhaseView {
        match self.lifecycle_state {
            ActorLifecycleState::Pending | ActorLifecycleState::Running => {
                InstancePhaseView::Replay
            }
            ActorLifecycleState::Stopping
            | ActorLifecycleState::Stopped
            | ActorLifecycleState::Failed => InstancePhaseView::Live,
        }
    }

    pub fn events_applied(&self) -> u64 {
        0
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum InstanceActorError {
    #[error("instance not found: {0}")]
    NotFound(InstanceId),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("RPC failed: {0}")]
    RpcFailed(String),
    #[error("registry error: {0}")]
    RegistryError(String),
    #[error("lifecycle error: {0}")]
    LifecycleError(String),
}

pub struct InstanceActorSpawner {
    registry: Arc<Mutex<InstanceRegistry>>,
}

impl InstanceActorSpawner {
    pub fn new(registry: Arc<InstanceRegistry>) -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry.as_ref().clone())),
        }
    }

    pub fn with_registry(registry: Arc<Mutex<InstanceRegistry>>) -> Self {
        Self { registry }
    }

    #[allow(clippy::unused_async)]
    pub async fn spawn(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstanceActor, InstanceActorError> {
        let handle = InstanceActorHandle::test(0);

        {
            let mut registry = self
                .registry
                .lock()
                .map_err(|e| InstanceActorError::RegistryError(format!("lock poisoned: {}", e)))?;
            registry
                .register(instance_id.clone(), handle, |_| Ok(()))
                .map_err(|e| InstanceActorError::RegistryError(e.to_string()))?;
        }

        Ok(InstanceActor::new(instance_id))
    }
}

impl Default for InstanceActorSpawner {
    fn default() -> Self {
        Self::new(Arc::new(InstanceRegistry::new(RegistryConfig::default())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::InstanceId;

    #[test]
    fn instance_actor_creation() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = InstanceActor::new(instance_id.clone());

        assert_eq!(actor.instance_id(), &instance_id);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Pending);
    }

    #[test]
    fn instance_actor_lifecycle_state_transition() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let mut actor = InstanceActor::new(instance_id.clone());

        actor.set_lifecycle_state(ActorLifecycleState::Running);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Running);

        actor.set_lifecycle_state(ActorLifecycleState::Stopping);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Stopping);
    }

    #[test]
    fn instance_actor_phase_replay_when_running() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = InstanceActor::with_lifecycle(instance_id, ActorLifecycleState::Running);

        assert_eq!(actor.phase(), InstancePhaseView::Replay);
    }

    #[test]
    fn instance_actor_phase_live_when_stopped() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor = InstanceActor::with_lifecycle(instance_id, ActorLifecycleState::Stopped);

        assert_eq!(actor.phase(), InstancePhaseView::Live);
    }

    #[tokio::test]
    async fn given_no_active_actor_when_starting_instance_then_registry_is_acquired_first() {
        let registry = Arc::new(Mutex::new(InstanceRegistry::new(RegistryConfig::default())));
        let spawner = InstanceActorSpawner::with_registry(registry.clone());
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        assert!(
            !registry.lock().unwrap().is_active(&instance_id),
            "precondition: no actor should be active for instance"
        );

        let result = spawner.spawn(instance_id.clone()).await;

        let is_active_after_spawn = registry.lock().unwrap().is_active(&instance_id);
        assert!(
            is_active_after_spawn,
            "registry should record instance as active after spawn completes"
        );

        assert!(
            result.is_ok(),
            "spawn should succeed when no prior actor exists"
        );
    }
}
