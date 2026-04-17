//! Instance actor module.
//!
//! This module provides the InstanceActor struct which wraps a ractor::ActorRef
//! and provides workflow instance management capabilities.

use std::sync::Arc;

use ractor::{ActorRef, rpc::CallResult};
use vo_types::InstanceId;

use crate::instance_registry::{InstanceActorHandle, InstanceRegistry, RegistryConfig};
use crate::lifecycle::ActorLifecycleState;
use crate::actor_messages::InstanceActorMessage;
use crate::{InstancePhaseView, InstanceSnapshot, StartError};

#[derive(Debug, Clone)]
pub struct InstanceActor {
    instance_id: InstanceId,
    actor_ref: ActorRef<InstanceActorMessage>,
    lifecycle_state: ActorLifecycleState,
}

impl InstanceActor {
    pub fn new(instance_id: InstanceId, actor_ref: ActorRef<InstanceActorMessage>) -> Self {
        Self {
            instance_id,
            actor_ref,
            lifecycle_state: ActorLifecycleState::Pending,
        }
    }

    pub fn with_lifecycle(
        instance_id: InstanceId,
        actor_ref: ActorRef<InstanceActorMessage>,
        lifecycle_state: ActorLifecycleState,
    ) -> Self {
        Self {
            instance_id,
            actor_ref,
            lifecycle_state,
        }
    }

    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    pub fn actor_ref(&self) -> &ActorRef<InstanceActorMessage> {
        &self.actor_ref
    }

    pub fn lifecycle_state(&self) -> ActorLifecycleState {
        self.lifecycle_state
    }

    pub fn set_lifecycle_state(&mut self, state: ActorLifecycleState) {
        self.lifecycle_state = state;
    }

    pub async fn send_message(&self, message: InstanceActorMessage) -> Result<(), InstanceActorError> {
        self.actor_ref
            .send_message(message)
            .map_err(|e| InstanceActorError::SendFailed(e.to_string()))
    }

    pub async fn send_message_and_await<R>(
        &self,
        message: InstanceActorMessage,
    ) -> Result<R, InstanceActorError>
    where
        R: Send + 'static,
    {
        self.actor_ref
            .send_message_and_await_reply(message)
            .await
            .map_err(|e| InstanceActorError::RpcFailed(e.to_string()))
    }

    pub fn phase(&self) -> InstancePhaseView {
        match self.lifecycle_state {
            ActorLifecycleState::Pending | ActorLifecycleState::Running => InstancePhaseView::Replay,
            ActorLifecycleState::Stopping | ActorLifecycleState::Stopped | ActorLifecycleState::Failed => {
                InstancePhaseView::Live
            }
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
    registry: Arc<InstanceRegistry>,
    registry_config: RegistryConfig,
}

impl InstanceActorSpawner {
    pub fn new(registry: Arc<InstanceRegistry>) -> Self {
        Self {
            registry,
            registry_config: RegistryConfig::default(),
        }
    }

    pub fn with_config(mut self, config: RegistryConfig) -> Self {
        self.registry_config = config;
        self
    }

    pub async fn spawn(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstanceActor, InstanceActorError> {
        let (actor_ref, handle) = ractor::Actor::spawn(
            Some(instance_id.to_string()),
            InstanceActor::new(instance_id.clone(), ractor::ActorRef::new()),
            ractor::ActorProperties::default(),
        )
        .await
        .map_err(|e| InstanceActorError::SendFailed(format!("spawn failed: {}", e)))?;

        self.registry
            .register(instance_id.clone(), handle)
            .await
            .map_err(|e| InstanceActorError::RegistryError(e.to_string()))?;

        Ok(InstanceActor::new(instance_id, actor_ref))
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

    #[test]
    fn instance_actor_creation() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor_ref = ractor::ActorRef::new();
        let actor = InstanceActor::new(instance_id.clone(), actor_ref);

        assert_eq!(actor.instance_id(), &instance_id);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Pending);
    }

    #[test]
    fn instance_actor_lifecycle_state_transition() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor_ref = ractor::ActorRef::new();
        let mut actor = InstanceActor::new(instance_id.clone(), actor_ref);

        actor.set_lifecycle_state(ActorLifecycleState::Running);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Running);

        actor.set_lifecycle_state(ActorLifecycleState::Stopping);
        assert_eq!(actor.lifecycle_state(), ActorLifecycleState::Stopping);
    }

    #[test]
    fn instance_actor_phase_replay_when_running() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor_ref = ractor::ActorRef::new();
        let actor = InstanceActor::with_lifecycle(
            instance_id,
            actor_ref,
            ActorLifecycleState::Running,
        );

        assert_eq!(actor.phase(), InstancePhaseView::Replay);
    }

    #[test]
    fn instance_actor_phase_live_when_stopped() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor_ref = ractor::ActorRef::new();
        let actor = InstanceActor::with_lifecycle(
            instance_id,
            actor_ref,
            ActorLifecycleState::Stopped,
        );

        assert_eq!(actor.phase(), InstancePhaseView::Live);
    }
}