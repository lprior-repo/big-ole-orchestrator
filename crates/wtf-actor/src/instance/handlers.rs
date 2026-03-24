//! Message handlers for WorkflowInstance actors.

pub(crate) mod snapshot;

use super::lifecycle::ParadigmState;
use super::procedural;
use super::state::InstanceState;
use crate::messages::{current_state_view, InstanceMsg, InstancePhaseView, InstanceStatusSnapshot};
use bytes::Bytes;
use ractor::{ActorProcessingErr, ActorRef, RpcReplyPort};
use wtf_common::{ActivityId, WorkflowEvent, WtfError};

pub async fn handle_msg(
    myself_ref: ActorRef<InstanceMsg>,
    msg: InstanceMsg,
    state: &mut InstanceState,
) -> Result<(), ActorProcessingErr> {
    match msg {
        InstanceMsg::InjectEvent { seq, event } => handle_inject_event_msg(state, seq, event).await,
        InstanceMsg::InjectSignal {
            signal_name,
            payload,
            reply,
        } => handle_signal(state, signal_name, payload, reply).await,
        InstanceMsg::Heartbeat => handle_heartbeat(state).await,
        InstanceMsg::Cancel { reason, reply } => {
            handle_cancel(myself_ref, state, reason, reply).await
        }
        InstanceMsg::GetProceduralCheckpoint {
            operation_id,
            reply,
        } => {
            procedural::handle_get_checkpoint(state, operation_id, reply).await;
            Ok(())
        }
        _ => handle_procedural_msg(myself_ref, msg, state).await,
    }
}

async fn handle_procedural_msg(
    myself_ref: ActorRef<InstanceMsg>,
    msg: InstanceMsg,
    state: &mut InstanceState,
) -> Result<(), ActorProcessingErr> {
    match msg {
        InstanceMsg::ProceduralDispatch {
            activity_type,
            payload,
            reply,
        } => {
            procedural::handle_dispatch(state, activity_type, payload, reply).await;
        }
        InstanceMsg::ProceduralSleep {
            operation_id,
            duration,
            reply,
        } => {
            procedural::handle_sleep(state, operation_id, duration, reply).await;
        }
        InstanceMsg::ProceduralNow {
            operation_id,
            reply,
        } => {
            procedural::handle_now(state, operation_id, reply).await;
        }
        InstanceMsg::ProceduralRandom {
            operation_id,
            reply,
        } => {
            procedural::handle_random(state, operation_id, reply).await;
        }
        InstanceMsg::ProceduralWaitForSignal {
            operation_id,
            signal_name,
            reply,
        } => {
            procedural::handle_wait_for_signal(state, operation_id, signal_name, reply).await;
        }
        InstanceMsg::ProceduralWorkflowCompleted => {
            procedural::handle_completed(myself_ref, state).await;
        }
        InstanceMsg::ProceduralWorkflowFailed(err) => {
            procedural::handle_failed(myself_ref, state, err).await;
        }
        InstanceMsg::GetStatus(reply) => {
            let _ = handle_get_status(state, reply);
        }
        _ => {
            return Err(ActorProcessingErr::from(
                "Unexpected message in procedural handler",
            ))
        }
    }
    Ok(())
}

async fn handle_inject_event_msg(
    state: &mut InstanceState,
    seq: u64,
    event: WorkflowEvent,
) -> Result<(), ActorProcessingErr> {
    inject_event(state, seq, &event).await?;

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

    if let WorkflowEvent::TimerFired { timer_id } = &event {
        let tid = wtf_common::TimerId::new(timer_id);
        if let Some(port) = state.pending_timer_calls.remove(&tid) {
            let _ = port.send(Ok::<(), WtfError>(()));
        }
    }

    if let WorkflowEvent::SignalReceived {
        signal_name,
        payload,
    } = &event
    {
        if let Some(port) = state.pending_signal_calls.remove(signal_name) {
            let _ = port.send(Ok::<Bytes, WtfError>(payload.clone()));
        }
    }

    Ok(())
}

pub(crate) async fn handle_signal(
    state: &mut InstanceState,
    signal_name: String,
    payload: Bytes,
    reply: RpcReplyPort<Result<(), WtfError>>,
) -> Result<(), ActorProcessingErr> {
    let store = match &state.args.event_store {
        Some(s) => s,
        None => {
            let _ = reply.send(Err(WtfError::nats_publish("Event store missing")));
            return Ok(());
        }
    };

    let event = WorkflowEvent::SignalReceived {
        signal_name: signal_name.clone(),
        payload: payload.clone(),
    };

    match store
        .publish(&state.args.namespace, &state.args.instance_id, event.clone())
        .await
    {
        Ok(seq) => {
            // Deliver to pending RPC port if one is registered
            if let Some(port) = state.pending_signal_calls.remove(&signal_name) {
                let _ = port.send(Ok(payload));
            } else if let ParadigmState::Procedural(s) = &mut state.paradigm_state {
                // Buffer the signal for a future wait_for_signal call
                s.received_signals
                    .entry(signal_name)
                    .or_default()
                    .push(payload);
            }
            let _ = inject_event(state, seq, &event).await;
            let _ = reply.send(Ok(()));
        }
        Err(e) => {
            let _ = reply.send(Err(e));
        }
    }

    Ok(())
}

async fn handle_heartbeat(state: &InstanceState) -> Result<(), ActorProcessingErr> {
    if let Some(store) = &state.args.state_store {
        if let Err(e) = store
            .put_heartbeat(&state.args.engine_node_id, &state.args.instance_id)
            .await
        {
            tracing::error!(error = %e, "heartbeat persistence failed");
        }
    }
    Ok(())
}

pub(crate) async fn handle_cancel(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
    reason: String,
    reply: RpcReplyPort<Result<(), WtfError>>,
) -> Result<(), ActorProcessingErr> {
    tracing::info!(
        instance_id = %state.args.instance_id,
        reason = %reason,
        "cancellation requested"
    );

    let event = WorkflowEvent::InstanceCancelled {
        reason: reason.clone(),
    };
    if let Some(store) = &state.args.event_store {
        if let Err(e) = store
            .publish(&state.args.namespace, &state.args.instance_id, event)
            .await
        {
            tracing::error!(
                instance_id = %state.args.instance_id,
                error = %e,
                "failed to persist InstanceCancelled event — \
                 recovery may resurrect this workflow"
            );
        }
    }

    let _ = reply.send(Ok(()));
    myself_ref.stop(Some(reason));
    Ok(())
}

fn handle_get_status(
    state: &InstanceState,
    reply: RpcReplyPort<InstanceStatusSnapshot>,
) -> Result<(), ActorProcessingErr> {
    let _ = reply.send(InstanceStatusSnapshot {
        instance_id: state.args.instance_id.clone(),
        namespace: state.args.namespace.clone(),
        workflow_type: state.args.workflow_type.clone(),
        paradigm: state.args.paradigm,
        phase: InstancePhaseView::from(state.phase),
        events_applied: state.total_events_applied,
        current_state: current_state_view(&state.paradigm_state),
    });
    Ok(())
}

/// Write a snapshot every 100 events (ADR-019).
pub const SNAPSHOT_INTERVAL: u32 = 100;

pub async fn inject_event(
    state: &mut InstanceState,
    seq: u64,
    event: &WorkflowEvent,
) -> Result<(), ActorProcessingErr> {
    state.paradigm_state = state
        .paradigm_state
        .apply_event(event, seq, state.phase)
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

    state.total_events_applied += 1;
    state.events_since_snapshot += 1;

    if state.events_since_snapshot >= SNAPSHOT_INTERVAL {
        snapshot::handle_snapshot_trigger(state).await?;
    }

    Ok(())
}
