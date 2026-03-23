pub mod handlers;
pub mod registry;
pub mod state;

pub use self::registry::{WorkflowDefinition, WorkflowRegistry};
pub use self::state::{OrchestratorConfig, OrchestratorState};
use crate::messages::OrchestratorMsg;
use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef};

/// The MasterOrchestrator root supervisor actor.
pub struct MasterOrchestrator;

#[async_trait]
impl Actor for MasterOrchestrator {
    type Msg = OrchestratorMsg;
    type State = OrchestratorState;
    type Arguments = OrchestratorConfig;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        config: OrchestratorConfig,
    ) -> Result<OrchestratorState, ActorProcessingErr> {
        tracing::info!(
            max_instances = config.max_instances,
            node_id = %config.engine_node_id,
            "MasterOrchestrator starting"
        );
        Ok(OrchestratorState::new(config))
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: OrchestratorMsg,
        state: &mut OrchestratorState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            OrchestratorMsg::GetEventStore { reply } => {
                let _ = reply.send(state.config.event_store.clone());
            }
            OrchestratorMsg::GetStateStore { reply } => {
                let _ = reply.send(state.config.state_store.clone());
            }
            OrchestratorMsg::GetTaskQueue { reply } => {
                let _ = reply.send(state.config.task_queue.clone());
            }
            OrchestratorMsg::GetSnapshotDb { reply } => {
                let _ = reply.send(state.config.snapshot_db.clone());
            }
            _ => handle_other_msg(myself, msg, state).await,
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        _myself: ActorRef<Self::Msg>,
        evt: ractor::SupervisionEvent,
        state: &mut OrchestratorState,
    ) -> Result<(), ActorProcessingErr> {
        match &evt {
            ractor::SupervisionEvent::ActorTerminated(cell, _, reason) => {
                handle_child_termination(state, cell, reason);
            }
            ractor::SupervisionEvent::ActorFailed(cell, err) => {
                tracing::error!(error = %err, "WorkflowInstance crashed — deregistering zombie");
                handle_child_termination(state, cell, &Some(err.to_string()));
            }
            ractor::SupervisionEvent::ActorStarted(_)
            | ractor::SupervisionEvent::ProcessGroupChanged(_) => {}
        }
        Ok(())
    }
}

async fn handle_other_msg(
    myself: ActorRef<OrchestratorMsg>,
    msg: OrchestratorMsg,
    state: &mut OrchestratorState,
) {
    match msg {
        OrchestratorMsg::StartWorkflow {
            namespace,
            instance_id,
            workflow_type,
            paradigm,
            input,
            reply,
        } => {
            let params = handlers::StartWorkflowParams {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            };
            handlers::handle_start_workflow(myself, state, params).await;
        }
        OrchestratorMsg::Signal {
            instance_id,
            signal_name,
            payload,
            reply,
        } => {
            handlers::handle_signal(state, instance_id, signal_name, payload, reply);
        }
        OrchestratorMsg::Terminate {
            instance_id,
            reason,
            reply,
        } => {
            handlers::handle_terminate(state, instance_id, reason, reply).await;
        }
        OrchestratorMsg::GetStatus { instance_id, reply } => {
            let _ = reply.send(handlers::handle_get_status(state, &instance_id).await);
        }
        OrchestratorMsg::ListActive { reply } => {
            let _ = reply.send(handlers::handle_list_active(state).await);
        }
        OrchestratorMsg::HeartbeatExpired { instance_id } => {
            handlers::handle_heartbeat_expired(myself, state, instance_id).await;
        }
        // Exhaustiveness guard: Get* variants are handled by `handle()` before
        // delegation; this wildcard catches any future OrchestratorMsg additions.
        ref unhandled => {
            tracing::warn!(msg = ?unhandled, "MasterOrchestrator received unhandled message variant");
        }
    }
}

fn handle_child_termination(
    state: &mut OrchestratorState,
    cell: &ractor::ActorCell,
    reason: &Option<String>,
) {
    let stopped_id = state
        .active
        .iter()
        .find(|(_, r)| r.get_id() == cell.get_id())
        .map(|(id, _)| id.clone());

    if let Some(id) = stopped_id {
        tracing::info!(instance_id = %id, reason = ?reason, "WorkflowInstance stopped — deregistering");
        state.deregister(&id);
    }
}
