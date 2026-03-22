//! The WorkflowInstance ractor actor implementation.

use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::sync::Arc;
use wtf_common::{ActivityId, WtfError, WorkflowEvent};

use crate::messages::{
    InstanceArguments, InstanceMsg, InstancePhase, InstancePhaseView, InstanceStatusSnapshot,
    WorkflowParadigm,
};
use wtf_storage::{ReplayBatch, ReplayConfig};
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

        let (mut state, from_seq) = load_initial_state(args).await?;
        let event_log = replay_events(&mut state, from_seq).await?;

        if let Some(nats) = &state.args.nats {
            transition_to_live(&state.args, &state.paradigm_state, &event_log, nats).await?;
        }

        state.phase = InstancePhase::Live;

        // 4. Start heartbeat timer
        myself.send_interval(std::time::Duration::from_secs(5), || InstanceMsg::Heartbeat);

        // 5. If procedural, spawn the workflow task
        if state.args.paradigm == WorkflowParadigm::Procedural {
            start_procedural_workflow(&mut state, &myself).await?;
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
            InstanceMsg::ProceduralNow { reply } => {
                procedural::handle_now(state, reply).await;
            }
            InstanceMsg::ProceduralRandom { reply } => {
                procedural::handle_random(state, reply).await;
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

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn load_initial_state(
    args: InstanceArguments,
) -> Result<(InstanceState, u64), ActorProcessingErr> {
    let mut state = InstanceState::initial(args.clone());
    let mut from_seq = 1;

    if let Some(db) = &args.snapshot_db {
        if let Ok(Some(snap)) = wtf_storage::read_snapshot(db, &args.instance_id) {
            state.paradigm_state = deserialize_paradigm_state(args.paradigm, &snap.state_bytes)?;
            state.total_events_applied = snap.seq;
            from_seq = snap.seq + 1;
            tracing::info!(instance_id = %args.instance_id, seq = snap.seq, "Snapshot loaded");
        }
    }
    Ok((state, from_seq))
}

pub fn deserialize_paradigm_state(
    paradigm: WorkflowParadigm,
    bytes: &[u8],
) -> Result<lifecycle::ParadigmState, ActorProcessingErr> {
    use lifecycle::ParadigmState;
    match paradigm {
        WorkflowParadigm::Fsm => Ok(ParadigmState::Fsm(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
        WorkflowParadigm::Dag => Ok(ParadigmState::Dag(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
        WorkflowParadigm::Procedural => Ok(ParadigmState::Procedural(
            rmp_serde::from_slice(bytes).map_err(|e| ActorProcessingErr::from(Box::new(e)))?,
        )),
    }
}

async fn replay_events(
    state: &mut InstanceState,
    from_seq: u64,
) -> Result<Vec<WorkflowEvent>, ActorProcessingErr> {
    let mut event_log = Vec::new();
    let nats = match &state.args.nats {
        Some(n) => n,
        None => return Ok(event_log),
    };
    let mut consumer = create_consumer(nats, &state.args, from_seq).await?;

    while let Some(event) = next_replayed_event(&mut consumer, state).await? {
        event_log.push(event);
    }

    tracing::info!(
        instance_id = %state.args.instance_id,
        events = state.total_events_applied,
        "Replay complete"
    );
    Ok(event_log)
}

async fn create_consumer(
    nats: &wtf_storage::NatsClient,
    args: &InstanceArguments,
    from_seq: u64,
) -> Result<wtf_storage::ReplayConsumer, ActorProcessingErr> {
    let config = ReplayConfig {
        from_seq,
        ..Default::default()
    };
    wtf_storage::create_replay_consumer(
        nats.jetstream(),
        &args.namespace,
        &args.instance_id,
        &config,
    )
    .await
    .map_err(|e| ActorProcessingErr::from(Box::new(e)))
}

async fn next_replayed_event(
    consumer: &mut wtf_storage::ReplayConsumer,
    state: &mut InstanceState,
) -> Result<Option<WorkflowEvent>, ActorProcessingErr> {
    match consumer.next_event().await {
        Ok(ReplayBatch::Event(replayed)) => {
            state.paradigm_state = state
                .paradigm_state
                .apply_event(&replayed.event, replayed.seq, InstancePhase::Replay)
                .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
            state.total_events_applied += 1;
            Ok(Some(replayed.event))
        }
        Ok(ReplayBatch::TailReached) => Ok(None),
        Err(e) => Err(ActorProcessingErr::from(Box::new(e))),
    }
}

async fn transition_to_live(
    args: &InstanceArguments,
    paradigm_state: &lifecycle::ParadigmState,
    event_log: &[WorkflowEvent],
    nats: &wtf_storage::NatsClient,
) -> Result<(), ActorProcessingErr> {
    let js = nats.jetstream();
    let actions = lifecycle::compute_live_transition(
        &args.instance_id,
        args.paradigm,
        paradigm_state,
        event_log,
    );

    if let Ok(timers_kv) = js.get_key_value(wtf_storage::bucket_names::TIMERS).await {
        lifecycle::execute_transition_actions(nats, &timers_kv, actions)
            .await
            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
    }
    Ok(())
}

async fn start_procedural_workflow(
    state: &mut InstanceState,
    myself: &ActorRef<InstanceMsg>,
) -> Result<(), ActorProcessingErr> {
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
    Ok(())
}
