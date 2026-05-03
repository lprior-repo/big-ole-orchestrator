//! Actor lifecycle management — direct instance operations and Actor implementation.

use std::collections::BTreeMap;

use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use vo_types::InstanceId;

use crate::actor_messages::{
    CompensateError, InstanceSnapshot, OrchestratorMsg, SignalError, TerminateError,
};

use super::{MasterOrchestrator, MasterState, RuntimeInstanceKey, signal_event_increment};

impl MasterState {
    // ── Status and listing ───────────────────────────────────────────────

    fn get_status(&self, namespace: String, instance_id: InstanceId) -> Option<InstanceSnapshot> {
        self.active
            .get(&RuntimeInstanceKey::new(namespace, instance_id))
            .map(|record| record.snapshot.clone())
    }

    fn list_active(&self) -> Vec<InstanceSnapshot> {
        self.active
            .values()
            .map(|record| {
                (
                    format!(
                        "{}/{}",
                        record.snapshot.namespace, record.snapshot.instance_id
                    ),
                    record.snapshot.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect()
    }

    // ── Direct termination ───────────────────────────────────────────────

    fn terminate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        reason: &str,
    ) -> Result<(), TerminateError> {
        if reason.is_empty() {
            return Err(TerminateError::Failed(
                "termination reason must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.active
            .remove(&key)
            .map(|_| ())
            .ok_or_else(|| TerminateError::NotFound(key.display()))
    }

    // ── Direct signal ────────────────────────────────────────────────────

    fn signal(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        signal_name: &str,
        payload: Bytes,
    ) -> Result<(), SignalError> {
        if signal_name.is_empty() {
            return Err(SignalError::Failed(
                "signal_name must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        let record = self
            .active
            .get_mut(&key)
            .ok_or_else(|| SignalError::NotFound(key.display()))?;
        record.signals_received = record
            .signals_received
            .checked_add(1)
            .ok_or_else(|| SignalError::Failed("signal counter overflow".to_string()))?;
        record.snapshot.events_applied = record
            .snapshot
            .events_applied
            .checked_add(signal_event_increment(&payload))
            .ok_or_else(|| SignalError::Failed("event counter overflow".to_string()))?;
        Ok(())
    }

    // ── Direct compensation ──────────────────────────────────────────────

    fn compensate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
    ) -> Result<(), CompensateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        let record = self
            .active
            .get_mut(&key)
            .ok_or_else(|| CompensateError::NotFound(key.display()))?;
        record.compensation_requested = true;
        record.snapshot.events_applied = record
            .snapshot
            .events_applied
            .checked_add(1)
            .ok_or_else(|| CompensateError::Failed("event counter overflow".to_string()))?;
        Ok(())
    }
}

// ── Actor implementation ─────────────────────────────────────────────────────

impl Actor for MasterOrchestrator {
    type Msg = OrchestratorMsg;
    type State = MasterState;
    type Arguments = super::OrchestratorConfig;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(MasterState::from_config(args))
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            OrchestratorMsg::StartWorkflow {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.commit_workflow_start(namespace, instance_id, workflow_type, paradigm, input),
            ),
            OrchestratorMsg::ReserveWorkflowStart {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.reserve_workflow_start(
                    namespace,
                    instance_id,
                    workflow_type,
                    paradigm,
                    input,
                ),
            ),
            OrchestratorMsg::CommitWorkflowStart {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.commit_workflow_start(namespace, instance_id, workflow_type, paradigm, input),
            ),
            OrchestratorMsg::AbortWorkflowStart {
                namespace,
                instance_id,
                reply,
            } => {
                let _: () = state.abort_workflow_start(namespace, instance_id);
                send_reply(reply, ())
            }
            OrchestratorMsg::GetStatus {
                namespace,
                instance_id,
                reply,
            } => {
                send_reply(reply, state.get_status(namespace, instance_id));
            }
            OrchestratorMsg::Terminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(reply, state.terminate(namespace, instance_id, &reason)),
            OrchestratorMsg::ReserveTerminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(
                reply,
                state.reserve_terminate(namespace, instance_id, reason),
            ),
            OrchestratorMsg::CommitTerminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(
                reply,
                state.commit_terminate(namespace, instance_id, reason),
            ),
            OrchestratorMsg::AbortWorkflowTransition {
                namespace,
                instance_id,
                reply,
            } => {
                let _: () = state.abort_workflow_transition(namespace, instance_id);
                send_reply(reply, ())
            }
            OrchestratorMsg::ListActive { reply } => send_reply(reply, state.list_active()),
            OrchestratorMsg::Compensate {
                namespace,
                instance_id,
                reply,
            } => {
                send_reply(reply, state.compensate(namespace, instance_id));
            }
            OrchestratorMsg::ReserveCompensate {
                namespace,
                instance_id,
                reply,
            } => send_reply(reply, state.reserve_compensate(namespace, instance_id)),
            OrchestratorMsg::CommitCompensate {
                namespace,
                instance_id,
                reply,
            } => send_reply(reply, state.commit_compensate(namespace, instance_id)),
            OrchestratorMsg::Signal {
                namespace,
                instance_id,
                signal_name,
                payload,
                reply,
            } => send_reply(
                reply,
                state.signal(namespace, instance_id, &signal_name, payload),
            ),
            OrchestratorMsg::ReserveSignal {
                namespace,
                instance_id,
                signal_name,
                reply,
            } => send_reply(
                reply,
                state.reserve_signal(namespace, instance_id, signal_name),
            ),
            OrchestratorMsg::CommitSignal {
                namespace,
                instance_id,
                signal_name,
                payload,
                reply,
            } => send_reply(
                reply,
                state.commit_signal(namespace, instance_id, signal_name, payload),
            ),
        }
        Ok(())
    }
}

fn send_reply<T: Send + 'static>(reply: ractor::port::RpcReplyPort<T>, value: T) {
    if let Err(error) = reply.send(value) {
        tracing::warn!(?error, "orchestrator reply receiver dropped");
    }
}
