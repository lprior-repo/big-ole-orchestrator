//! Message handlers for `WorkflowInstance` actors.

#![allow(clippy::missing_errors_doc, clippy::manual_let_else, clippy::unused_async)]

pub(crate) mod snapshot;

use super::lifecycle::ParadigmState;
use super::procedural;
use super::state::InstanceState;
use crate::messages::{current_state_view, InstanceMsg, InstancePhaseView, InstanceStatusSnapshot};
use bytes::Bytes;
use ractor::{ActorProcessingErr, ActorRef, RpcReplyPort};
use std::time::Duration;
use thiserror::Error;
use wtf_common::{ActivityId, EventStore, WorkflowEvent, WtfError};

/// Saga compensation errors for cancellation.
#[derive(Debug, Error)]
#[expect(dead_code)]
pub(crate) enum CancelError {
    #[error("publish failed after {0} retries: {1}")]
    PublishFailed(u32, WtfError),
    #[error("outbox full (capacity {0})")]
    OutboxFull(usize),
    #[error("outbox drain failed: {0}")]
    OutboxDrainFailed(WtfError),
    #[error("cancellation timeout after {0} attempts")]
    CancellationTimeout(u32),
    #[error("actor not running")]
    ActorNotRunning,
}

/// Maximum retry attempts for publishing cancellation events.
const MAX_PUBLISH_RETRIES: u32 = 3;
/// Initial backoff delay in milliseconds.
const INITIAL_BACKOFF_MS: u64 = 100;
/// Backoff multiplier for exponential retry.
const BACKOFF_MULTIPLIER: u64 = 2;
/// Maximum outbox capacity before fail-fast.
const OUTBOX_CAPACITY: usize = 100;

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
            let _ = procedural::handle_now(state, operation_id, reply).await;
        }
        InstanceMsg::ProceduralRandom {
            operation_id,
            reply,
        } => {
            let _ = procedural::handle_random(state, operation_id, reply).await;
        }
        InstanceMsg::ProceduralWaitForSignal {
            operation_id,
            signal_name,
            reply,
        } => {
            procedural::handle_wait_for_signal(state, operation_id, signal_name, reply).await;
        }
        InstanceMsg::ProceduralWorkflowCompleted => {
            let _ = procedural::handle_completed(myself_ref, state).await;
        }
        InstanceMsg::ProceduralWorkflowFailed(err) => {
            let _ = procedural::handle_failed(myself_ref, state, err).await;
        }
        InstanceMsg::GetStatus(reply) => {
            handle_get_status(state, reply);
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
    let Some(store) = &state.args.event_store else {
        let _ = reply.send(Err(WtfError::nats_publish("Event store missing")));
        return Ok(());
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
    state: &mut InstanceState,
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

    // I-1: publish must happen before actor stop
    // Saga pattern: publish with compensation - retry, then outbox, then fail-safe
    let publish_result = if let Some(store) = state.args.event_store.clone() {
        let namespace = state.args.namespace.clone();
        let instance_id = state.args.instance_id.clone();
        let store_ref = store.as_ref();

        // Retry loop with exponential backoff
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        let mut attempt = 1;
        loop {
            match store_ref.publish(&namespace, &instance_id, event.clone()).await {
                Ok(_) => {
                    // Success - drain outbox if any pending events
                    if !state.outbox.is_empty() {
                        if let Err(e) = drain_outbox(state, store_ref).await {
                            break Err(e); // e is already CancelError
                        }
                    }
                    break Ok(());
                }
                Err(e) if attempt < MAX_PUBLISH_RETRIES => {
                    tracing::warn!(
                        instance_id = %instance_id,
                        attempt,
                        error = %e,
                        "publish failed, retrying in {}ms",
                        backoff_ms
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= BACKOFF_MULTIPLIER;
                    attempt += 1;
                }
                Err(e) => {
                    // Final attempt failed - try outbox fallback
                    tracing::error!(
                        instance_id = %instance_id,
                        attempt,
                        error = %e,
                        "all publish retries exhausted"
                    );

                    // Try to drain outbox first to make room
                    if !state.outbox.is_empty() {
                        if let Err(e) = drain_outbox(state, store_ref).await {
                            break Err(e); // e is already CancelError
                        }
                    }

                    // Now try outbox fallback
                    if state.outbox.len() >= OUTBOX_CAPACITY {
                        break Err(CancelError::OutboxFull(OUTBOX_CAPACITY));
                    }
                    state.outbox.push(event.clone());

                    // Last resort: try publish one more time after outbox drain
                    match store_ref.publish(&namespace, &instance_id, event).await {
                        Ok(_) => {
                            // Success after outbox fallback
                            if let Err(e) = drain_outbox(state, store_ref).await {
                                break Err(e); // e is already CancelError
                            }
                            break Ok(());
                        }
                        Err(e) => {
                            break Err(CancelError::PublishFailed(MAX_PUBLISH_RETRIES, e));
                        }
                    }
                }
            }
        }
    } else {
        // No store - store in outbox for later recovery
        state.outbox.push(event);
        Ok(())
    };

    if let Err(CancelError::OutboxFull(_)) = publish_result {
        // This should never happen with proper sizing - fail fast
        tracing::error!(
            instance_id = %state.args.instance_id,
            "outbox overflow during cancellation - bug in sizing"
        );
        // Do not stop the actor - let it be rescued by recovery
        let _ = reply.send(Err(WtfError::sled_error("outbox overflow during cancel")));
        return Err(ActorProcessingErr::from("outbox overflow during cancel"));
    }

    match publish_result {
        Ok(()) => {
            tracing::debug!(instance_id = %state.args.instance_id, "InstanceCancelled persisted");
        }
        Err(CancelError::ActorNotRunning) => {
            // Actor already dead - this is fine for idempotent cancel
            tracing::debug!(instance_id = %state.args.instance_id, "actor already stopped");
        }
        Err(e) => {
            // Data loss scenario - log but do not fail
            tracing::error!(
                instance_id = %state.args.instance_id,
                error = %e,
                "cancellation event loss - recovery may resurrect workflow"
            );
        }
    }

    let _ = reply.send(Ok(()));
    myself_ref.stop(Some(reason));
    Ok(())
}

/// Drain events from outbox to the store.
async fn drain_outbox(
    state: &mut InstanceState,
    store: &dyn EventStore,
) -> Result<(), CancelError> {
    let namespace = state.args.namespace.clone();
    let instance_id = state.args.instance_id.clone();

    let events = state.outbox.drain(..).collect::<Vec<_>>();
    for event in events {
        store
            .publish(&namespace, &instance_id, event)
            .await
            .map_err(CancelError::OutboxDrainFailed)?;
    }
    Ok(())
}

fn handle_get_status(
    state: &InstanceState,
    reply: RpcReplyPort<InstanceStatusSnapshot>,
) {
    let _ = reply.send(InstanceStatusSnapshot {
        instance_id: state.args.instance_id.clone(),
        namespace: state.args.namespace.clone(),
        workflow_type: state.args.workflow_type.clone(),
        paradigm: state.args.paradigm,
        phase: InstancePhaseView::from(state.phase),
        events_applied: state.total_events_applied,
        current_state: current_state_view(&state.paradigm_state),
    });
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
