use ractor::{Actor, ActorRef, RpcReplyPort};
use wtf_common::{InstanceId, NamespaceId, WorkflowParadigm, InstanceMetadata};
use crate::messages::{InstanceArguments, OrchestratorMsg, StartError};
use crate::master::state::OrchestratorState;
use crate::instance::WorkflowInstance;

/// Handle a StartWorkflow message.
pub async fn handle_start_workflow(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    namespace: NamespaceId,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: WorkflowParadigm,
    input: bytes::Bytes,
    reply: RpcReplyPort<Result<InstanceId, StartError>>,
) {
    if let Err(e) = validate_request(state, &instance_id) {
        let _ = reply.send(Err(e));
        return;
    }

    let args = build_args(state, namespace, instance_id, workflow_type, paradigm, input);
    let result = spawn_and_register(myself, state, args).await;
    let _ = reply.send(result);
}

fn validate_request(state: &OrchestratorState, id: &InstanceId) -> Result<(), StartError> {
    if !state.has_capacity() {
        return Err(StartError::AtCapacity {
            running: state.active_count(),
            max: state.config.max_instances,
        });
    }
    if state.active.contains_key(id) {
        return Err(StartError::AlreadyExists(id.clone()));
    }
    Ok(())
}

fn build_args(
    state: &OrchestratorState,
    ns: NamespaceId,
    id: InstanceId,
    wtype: String,
    paradigm: WorkflowParadigm,
    input: bytes::Bytes,
) -> InstanceArguments {
    InstanceArguments {
        namespace: ns,
        instance_id: id,
        workflow_type: wtype.clone(),
        paradigm,
        input,
        engine_node_id: state.config.engine_node_id.clone(),
        event_store: state.config.event_store.clone(),
        state_store: state.config.state_store.clone(),
        task_queue: state.config.task_queue.clone(),
        snapshot_db: state.config.snapshot_db.clone(),
        procedural_workflow: state.registry.get_procedural(&wtype),
        workflow_definition: state.registry.get_definition(&wtype),
    }
}

async fn spawn_and_register(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    args: InstanceArguments,
) -> Result<InstanceId, StartError> {
    let id = args.instance_id.clone();
    let name = format!("wf-{}", id.as_str());
    let (actor_ref, _) = WorkflowInstance::spawn_linked(
        Some(name), WorkflowInstance, args.clone(), myself.into()
    ).await.map_err(|e| StartError::SpawnFailed(e.to_string()))?;

    persist_metadata(state, &args).await;
    state.register(id.clone(), actor_ref);
    Ok(id)
}

async fn persist_metadata(state: &OrchestratorState, args: &InstanceArguments) {
    let Some(store) = &state.config.state_store else {
        return;
    };

    let metadata = InstanceMetadata {
        namespace: args.namespace.clone(),
        instance_id: args.instance_id.clone(),
        workflow_type: args.workflow_type.clone(),
        paradigm: args.paradigm,
        engine_node_id: state.config.engine_node_id.clone(),
    };

    let _ = store.put_instance_metadata(metadata).await;
}

#[cfg(test)]
mod tests {
    use super::validate_request;
    use crate::master::state::{OrchestratorConfig, OrchestratorState};
    use wtf_common::InstanceId;

    #[test]
    fn validate_request_rejects_when_at_capacity() {
        let state = OrchestratorState::new(OrchestratorConfig {
            max_instances: 0,
            ..OrchestratorConfig::default()
        });
        let result = validate_request(&state, &InstanceId::new("inst-1"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_request_accepts_when_capacity_available() {
        let state = OrchestratorState::new(OrchestratorConfig {
            max_instances: 10,
            ..OrchestratorConfig::default()
        });
        let result = validate_request(&state, &InstanceId::new("inst-1"));
        assert!(result.is_ok());
    }
}
