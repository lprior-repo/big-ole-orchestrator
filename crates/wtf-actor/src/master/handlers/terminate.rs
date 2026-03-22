use ractor::rpc::CallResult;
use ractor::RpcReplyPort;
use wtf_common::InstanceId;
use crate::messages::{InstanceMsg, TerminateError};
use crate::master::state::OrchestratorState;
use std::time::Duration;

const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_millis(500);

pub async fn handle_terminate(
    state: &mut OrchestratorState,
    instance_id: InstanceId,
    reason: String,
    reply: RpcReplyPort<Result<(), TerminateError>>,
) {
    match state.get(&instance_id) {
        None => { let _ = reply.send(Err(TerminateError::NotFound(instance_id))); }
        Some(actor_ref) => {
            let res = call_cancel(actor_ref, reason).await;
            let _ = reply.send(res);
        }
    }
}

async fn call_cancel(
    actor_ref: &ractor::ActorRef<InstanceMsg>,
    reason: String,
) -> Result<(), TerminateError> {
    let call_result = actor_ref
        .call(
            |tx| InstanceMsg::Cancel { reason, reply: tx },
            Some(INSTANCE_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Ok(CallResult::Success(inner)) => inner.map_err(|e: wtf_common::WtfError| TerminateError::Failed(e.to_string())),
        Ok(CallResult::Timeout) => Err(TerminateError::Failed("cancel timed out".into())),
        Ok(CallResult::SenderError) => Err(TerminateError::Failed("actor dropped reply".into())),
        Err(e) => Err(TerminateError::Failed(format!("send failed: {e}"))),
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
            assert!(matches!(reply, Err(crate::messages::TerminateError::NotFound(id)) if id == instance_id));
        }
    }
}
