use crate::instance::WorkflowInstance;
use crate::master::state::OrchestratorState;
use crate::messages::{InstanceArguments, InstanceSeed, OrchestratorMsg, StartError};
use ractor::{Actor as _, ActorRef, RpcReplyPort};
use wtf_common::{InstanceId, InstanceMetadata, NamespaceId, WorkflowParadigm, WtfError};

/// Groups the per-request parameters for starting a new workflow instance.
pub struct StartWorkflowParams {
    pub namespace: NamespaceId,
    pub instance_id: InstanceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub input: bytes::Bytes,
    pub reply: RpcReplyPort<Result<InstanceId, StartError>>,
}

/// Handle a StartWorkflow message.
pub async fn handle_start_workflow(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    params: StartWorkflowParams,
) {
    if let Err(e) = validate_request(state, &params.instance_id) {
        let _ = params.reply.send(Err(e));
        return;
    }

    let seed = InstanceSeed {
        namespace: params.namespace,
        instance_id: params.instance_id,
        workflow_type: params.workflow_type.clone(),
        paradigm: params.paradigm,
        input: params.input,
    };
    let args = state.build_instance_args(seed);
    let result = spawn_and_register(myself, state, args).await;
    let _ = params.reply.send(result);
}

fn validate_request(state: &OrchestratorState, id: &InstanceId) -> Result<(), StartError> {
    if !state.capacity_check() {
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

async fn spawn_and_register(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    args: InstanceArguments,
) -> Result<InstanceId, StartError> {
    let id = args.instance_id.clone();
    let name = format!("wf-{}", id.as_str());
    let (actor_ref, _) =
        WorkflowInstance::spawn_linked(Some(name), WorkflowInstance, args.clone(), myself.into())
            .await
            .map_err(|e| StartError::SpawnFailed(e.to_string()))?;

    if let Err(e) = persist_metadata(state, &args).await {
        tracing::error!(
            instance_id = id.as_str(),
            namespace = args.namespace.as_str(),
            error = %e,
            "metadata persistence failed — killing spawned actor"
        );
        actor_ref.stop(Some("metadata persistence failed".into()));
        return Err(StartError::PersistenceFailed(e.to_string()));
    }
    state.register(id.clone(), actor_ref);
    Ok(id)
}

async fn persist_metadata(
    state: &OrchestratorState,
    args: &InstanceArguments,
) -> Result<(), WtfError> {
    let Some(store) = &state.config.state_store else {
        return Ok(());
    };

    let metadata = InstanceMetadata {
        namespace: args.namespace.clone(),
        instance_id: args.instance_id.clone(),
        workflow_type: args.workflow_type.clone(),
        paradigm: args.paradigm,
        engine_node_id: state.config.engine_node_id.clone(),
    };

    store.put_instance_metadata(metadata).await
}

#[cfg(test)]
mod tests {
    use super::validate_request;
    use crate::master::state::{OrchestratorConfig, OrchestratorState};
    use crate::messages::{InstanceMsg, StartError};
    use ractor::{Actor as _, ActorRef};
    use wtf_common::InstanceId;

    /// Minimal actor that discards all messages — used to obtain a valid `ActorRef<InstanceMsg>`.
    struct NullActor;

    #[async_trait::async_trait]
    impl ractor::Actor for NullActor {
        type Msg = InstanceMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _: ActorRef<Self::Msg>,
            _: Self::Arguments,
        ) -> Result<(), ractor::ActorProcessingErr> {
            Ok(())
        }
    }

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

    #[tokio::test]
    async fn validate_request_rejects_when_instance_already_exists() {
        let mut state = OrchestratorState::new(OrchestratorConfig {
            max_instances: 10,
            ..OrchestratorConfig::default()
        });
        let id = InstanceId::new("inst-1");
        let (actor_ref, _handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id.clone(), actor_ref);
        let result = validate_request(&state, &id);
        assert!(matches!(result, Err(StartError::AlreadyExists(_))));
    }
}
