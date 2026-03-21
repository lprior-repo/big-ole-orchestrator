//! The WorkflowInstance ractor actor implementation.

use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::sync::Arc;
use wtf_common::{ActivityId, WtfError, WorkflowEvent};

use crate::messages::{
    InstanceMsg, InstancePhase, InstancePhaseView, InstanceStatusSnapshot,
    WorkflowParadigm,
};
use super::state::InstanceState;
use super::lifecycle;
use super::procedural;

/// The WorkflowInstance ractor actor.
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

        let mut state = InstanceState::initial(args);
        let mut event_log = Vec::new();

        // 1. Replay events from JetStream
        if let Some(nats) = &state.args.nats {
            let js = nats.jetstream();
            let mut consumer = wtf_storage::create_replay_consumer(
                js,
                &state.args.namespace,
                &state.args.instance_id,
                &wtf_storage::ReplayConfig::default(),
            )
            .await
            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

            loop {
                match consumer.next_event().await {
                    Ok(wtf_storage::ReplayBatch::Event(replayed)) => {
                        event_log.push(replayed.event.clone());
                        state.paradigm_state = state
                            .paradigm_state
                            .apply_event(&replayed.event, replayed.seq, InstancePhase::Replay)
                            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
                        state.total_events_applied += 1;
                    }
                    Ok(wtf_storage::ReplayBatch::TailReached) => break,
                    Err(e) => return Err(ActorProcessingErr::from(Box::new(e))),
                }
            }

            tracing::info!(
                instance_id = %state.args.instance_id,
                events = state.total_events_applied,
                "Replay complete"
            );

            // 2. Transition to Live phase
            let actions = lifecycle::compute_live_transition(
                &state.args.instance_id,
                state.args.paradigm,
                &state.paradigm_state,
                &event_log,
            );

            // 3. Execute transition side effects (re-dispatches, re-arm timers)
            if let Ok(timers_kv) = js.get_key_value(wtf_storage::bucket_names::TIMERS).await {
                lifecycle::execute_transition_actions(nats, &timers_kv, actions)
                    .await
                    .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
            }
        }

        state.phase = InstancePhase::Live;

        // 4. Start heartbeat timer
        myself.send_interval(std::time::Duration::from_secs(5), || InstanceMsg::Heartbeat);

        // 5. If procedural, spawn the workflow task
        if state.args.paradigm == WorkflowParadigm::Procedural {
            if let Some(wf_fn) = &state.args.procedural_workflow {
                let ctx = crate::procedural::WorkflowContext::new(
                    state.args.instance_id.clone(),
                    state.paradigm_state.operation_counter(),
                    myself.clone(),
                );
                let wf_fn = Arc::clone(wf_fn);
                let myself_clone = myself.clone();
                let handle = tokio::spawn(async move {
                    match wf_fn.execute(ctx).await {
                        Ok(_) => {
                            let _ = myself_clone.cast(InstanceMsg::ProceduralWorkflowCompleted);
                        }
                        Err(e) => {
                            let _ = myself_clone.cast(InstanceMsg::ProceduralWorkflowFailed(e.to_string()));
                        }
                    }
                });
                state.procedural_task = Some(handle);
            }
        }

        Ok(state)
    }

    async fn handle(
        &self,
        myself_ref: ActorRef<InstanceMsg>,
        msg: InstanceMsg,
        state: &mut InstanceState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            InstanceMsg::InjectEvent { seq, event } => {
                super::handle_inject_event(state, seq, &event).await?;

                // If ActivityCompleted, check for pending procedural RPC call waiting for this result.
                if let WorkflowEvent::ActivityCompleted {
                    activity_id,
                    result,
                    ..
                } = &event
                {
                    let aid = ActivityId::new(activity_id);
                    if let Some(port) = state.pending_activity_calls.remove(&aid) {
                        let _ = port.send(Ok::<Bytes, WtfError>(result.clone()));
                    }
                }

                // If TimerFired, check for pending procedural RPC call waiting for this timer.
                if let WorkflowEvent::TimerFired { timer_id } = &event {
                    let tid = wtf_common::TimerId::new(timer_id);
                    if let Some(port) = state.pending_timer_calls.remove(&tid) {
                        let _ = port.send(Ok::<(), WtfError>(()));
                    }
                }
            }
            InstanceMsg::InjectSignal {
                signal_name,
                payload,
                reply,
            } => {
                tracing::debug!(
                    instance_id = %state.args.instance_id,
                    signal = %signal_name,
                    "signal received (stub)"
                );
                drop(payload);
                let _ = reply.send(Ok(()));
            }
            InstanceMsg::Heartbeat => {
                if let Some(nats) = &state.args.nats {
                    let js = nats.jetstream();
                    if let Ok(hb_kv) = js.get_key_value(wtf_storage::bucket_names::HEARTBEATS).await {
                        let _ = wtf_storage::write_heartbeat(
                            &hb_kv,
                            &state.args.instance_id,
                            &state.args.engine_node_id,
                        ).await;
                    }
                }
            }
            InstanceMsg::Cancel { reason, reply } => {
                tracing::info!(
                    instance_id = %state.args.instance_id,
                    reason = %reason,
                    "cancellation requested"
                );
                let _ = reply.send(Ok(()));
            }
            InstanceMsg::GetProceduralCheckpoint { operation_id, reply } => {
                procedural::handle_get_checkpoint(state, operation_id, reply).await;
            }
            InstanceMsg::ProceduralDispatch {
                activity_type,
                payload,
                reply,
            } => {
                procedural::handle_dispatch(state, activity_type, payload, reply).await;
            }
            InstanceMsg::ProceduralSleep { duration, reply } => {
                procedural::handle_sleep(state, duration, reply).await;
            }
            InstanceMsg::ProceduralWorkflowCompleted => {
                procedural::handle_completed(myself_ref, state).await;
            }
            InstanceMsg::ProceduralWorkflowFailed(err) => {
                procedural::handle_failed(myself_ref, state, err).await;
            }
            InstanceMsg::GetStatus(reply) => {
                let _ = reply.send(InstanceStatusSnapshot {
                    instance_id: state.args.instance_id.clone(),
                    namespace: state.args.namespace.clone(),
                    workflow_type: state.args.workflow_type.clone(),
                    paradigm: state.args.paradigm,
                    phase: InstancePhaseView::from(state.phase),
                    events_applied: state.total_events_applied,
                });
            }
        }
        Ok(())
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
        Ok(())
    }
}
