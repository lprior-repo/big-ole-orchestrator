use super::INSTANCE_CALL_TIMEOUT;
use crate::master::state::OrchestratorState;
use crate::messages::{GetStatusError, InstanceMsg, InstanceStatusSnapshot};
use ractor::rpc::CallResult;
use wtf_common::InstanceId;

pub async fn handle_get_status(
    state: &OrchestratorState,
    instance_id: &InstanceId,
) -> Result<Option<InstanceStatusSnapshot>, GetStatusError> {
    let actor_ref = match state.get(instance_id) {
        Some(r) => r,
        None => return Ok(None),
    };
    match actor_ref
        .call(InstanceMsg::GetStatus, Some(INSTANCE_CALL_TIMEOUT))
        .await
    {
        Ok(CallResult::Success(snapshot)) => Ok(Some(snapshot)),
        Ok(CallResult::Timeout) => Err(GetStatusError::Timeout),
        Ok(CallResult::SenderError) => Err(GetStatusError::ActorDied),
        Err(_) => Err(GetStatusError::ActorDied),
    }
}
