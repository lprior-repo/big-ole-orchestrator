//! The `WorkflowInstance` ractor actor implementation.

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef};

use super::handlers;
use super::init;
use super::state::InstanceState;
use crate::messages::{InstanceMsg, InstancePhase};
use wtf_common::WorkflowParadigm;

/// The `WorkflowInstance` ractor actor.
pub struct WorkflowInstance;

#[async_trait]
impl Actor for WorkflowInstance {
    type Msg = InstanceMsg;
    type State = InstanceState;
    type Arguments = crate::messages::InstanceArguments;

    async fn pre_start(
        &self,
        myself: ActorRef<InstanceMsg>,
        args: Self::Arguments,
    ) -> Result<InstanceState, ActorProcessingErr> {
        tracing::info!(
            instance_id = %args.instance_id,
            namespace = %args.namespace,
            workflow_type = %args.workflow_type,
            paradigm = ?args.paradigm,
            "WorkflowInstance starting"
        );

        let (mut state, from_seq) = init::load_initial_state(args).await?;
        let (event_log, consumer) = init::replay_events(&mut state, from_seq).await?;

        if let Some(queue) = &state.args.task_queue {
            init::transition_to_live(
                &state.args,
                &state.paradigm_state,
                &event_log,
                queue.as_ref(),
            )
            .await?;
        }

        if let Some(c) = consumer {
            init::spawn_live_subscription(&mut state, &myself, c);
        }

        init::publish_instance_started(&state.args, from_seq, &event_log).await?;

        state.phase = InstancePhase::Live;

        // Start heartbeat timer
        myself.send_interval(std::time::Duration::from_secs(5), || InstanceMsg::Heartbeat);

        // If procedural, spawn the workflow task
        if state.args.paradigm == WorkflowParadigm::Procedural {
            init::start_procedural_workflow(&mut state, &myself).await?;
        }

        Ok(state)
    }

    async fn handle(
        &self,
        myself_ref: ActorRef<InstanceMsg>,
        msg: InstanceMsg,
        state: &mut InstanceState,
    ) -> Result<(), ActorProcessingErr> {
        handlers::handle_msg(myself_ref, msg, state).await
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        tracing::info!(instance_id = %state.args.instance_id, "WorkflowInstance stopping");
        if let Some(handle) = state.procedural_task.take() {
            handle.abort();
        }
        if let Some(handle) = state.live_subscription_task.take() {
            handle.abort();
        }
        Ok(())
    }
}
