use super::INSTANCE_CALL_TIMEOUT;
use crate::master::state::OrchestratorState;
use crate::messages::{InstanceMsg, TerminateError};
use ractor::rpc::CallResult;
use ractor::RpcReplyPort;
use wtf_common::InstanceId;

pub async fn handle_terminate(
    state: &mut OrchestratorState,
    instance_id: InstanceId,
    reason: String,
    reply: RpcReplyPort<Result<(), TerminateError>>,
) {
    match state.get(&instance_id) {
        None => {
            let _ = reply.send(Err(TerminateError::NotFound(instance_id)));
        }
        Some(actor_ref) => {
            let res = call_cancel(actor_ref, &instance_id, reason).await;
            let _ = reply.send(res);
        }
    }
}

async fn call_cancel(
    actor_ref: &ractor::ActorRef<InstanceMsg>,
    instance_id: &InstanceId,
    reason: String,
) -> Result<(), TerminateError> {
    let call_result = actor_ref
        .call(
            |tx| InstanceMsg::Cancel { reason, reply: tx },
            Some(INSTANCE_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Ok(CallResult::Success(Ok(()))) => Ok(()),
        Ok(CallResult::Success(Err(_))) => {
            // The Cancel handler always replies Ok(()) — this arm is defensive but
            // unreachable given the current handler implementation.
            Ok(())
        }
        Ok(CallResult::Timeout) => Err(TerminateError::Timeout(instance_id.clone())),
        Ok(CallResult::SenderError) => Err(TerminateError::NotFound(instance_id.clone())),
        Err(_) => Err(TerminateError::NotFound(instance_id.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::handle_terminate;
    use crate::master::state::{OrchestratorConfig, OrchestratorState};
    use ractor::concurrency::oneshot;
    use wtf_common::InstanceId;

    #[tokio::test]
    async fn terminate_returns_not_found_for_unknown_instance() {
        let mut state = OrchestratorState::new(OrchestratorConfig::default());
        let (tx, rx) = oneshot();
        let instance_id = InstanceId::new("missing-inst");

        handle_terminate(
            &mut state,
            instance_id.clone(),
            "test".to_owned(),
            tx.into(),
        )
        .await;

        let reply = rx.await;
        assert!(reply.is_ok());
        if let Ok(reply) = reply {
            assert!(
                matches!(reply, Err(crate::messages::TerminateError::NotFound(id)) if id == instance_id)
            );
        }
    }
}
