use ractor::{Actor, ActorProcessingErr, ActorRef};
use vo_types::state::LifecycleState;
use vo_types::InstanceId;

pub struct InstanceState {
    pub instance_id: InstanceId,
    pub lifecycle_state: LifecycleState,
    pub events_applied: usize,
}

pub struct InstanceActor;

impl Actor for InstanceActor {
    type Msg = crate::actor_messages::InstanceActorMessage;
    type State = InstanceState;
    type Arguments = InstanceId;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        instance_id: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(InstanceState {
            instance_id,
            lifecycle_state: LifecycleState::Pending,
            events_applied: 0,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        _message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }
}
