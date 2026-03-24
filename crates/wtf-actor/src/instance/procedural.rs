//! Procedural-specific message handlers for WorkflowInstance.

use super::handlers;
use super::lifecycle::ParadigmState;
use super::state::InstanceState;
use bytes::Bytes;
use wtf_common::{ActivityId, WorkflowEvent, WtfError};

pub use super::procedural_utils::{handle_completed, handle_failed, handle_now, handle_random};

pub async fn handle_get_checkpoint(
    state: &InstanceState,
    operation_id: u32,
    reply: ractor::RpcReplyPort<Option<crate::procedural::Checkpoint>>,
) {
    let checkpoint = if let ParadigmState::Procedural(s) = &state.paradigm_state {
        s.get_checkpoint(operation_id).cloned()
    } else {
        None
    };
    let _ = reply.send(checkpoint);
}

pub async fn handle_dispatch(
    state: &mut InstanceState,
    activity_type: String,
    payload: Bytes,
    reply: ractor::RpcReplyPort<Result<Bytes, WtfError>>,
) {
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let activity_id = ActivityId::procedural(&state.args.instance_id, s.operation_counter);
        let event = WorkflowEvent::ActivityDispatched {
            activity_id: activity_id.to_string(),
            activity_type,
            payload,
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        };
        append_and_inject_event(state, event, Some(activity_id), reply).await;
    }
}

async fn append_and_inject_event(
    state: &mut InstanceState,
    event: WorkflowEvent,
    activity_id: Option<ActivityId>,
    reply: ractor::RpcReplyPort<Result<Bytes, WtfError>>,
) {
    let store = match &state.args.event_store {
        Some(s) => s,
        None => {
            let _ = reply.send(Err(WtfError::nats_publish("Event store missing")));
            return;
        }
    };

    match store
        .publish(
            &state.args.namespace,
            &state.args.instance_id,
            event.clone(),
        )
        .await
    {
        Ok(seq) => {
            if let Some(aid) = activity_id {
                state.pending_activity_calls.insert(aid, reply);
            }
            let _ = handlers::inject_event(state, seq, &event).await;
        }
        Err(e) => {
            let _ = reply.send(Err(e));
        }
    }
}

pub async fn handle_sleep(
    state: &mut InstanceState,
    operation_id: u32,
    duration: std::time::Duration,
    reply: ractor::RpcReplyPort<Result<(), WtfError>>,
) {
    if let ParadigmState::Procedural(_) = &state.paradigm_state {
        let timer_id = wtf_common::TimerId::procedural(&state.args.instance_id, operation_id);
        let fire_at = chrono::Utc::now()
            + chrono::Duration::from_std(duration).unwrap_or_else(|_| chrono::Duration::zero());

        let event = WorkflowEvent::TimerScheduled {
            timer_id: timer_id.to_string(),
            fire_at,
        };

        append_and_inject_timer_event(state, event, timer_id, reply).await;
    }
}

pub async fn handle_wait_for_signal(
    state: &mut InstanceState,
    operation_id: u32,
    signal_name: String,
    reply: ractor::RpcReplyPort<Result<Bytes, WtfError>>,
) {
    if let ParadigmState::Procedural(s) = &mut state.paradigm_state {
        // Check if a buffered signal exists for this name.
        if let Some(queue) = s.received_signals.get_mut(&signal_name) {
            if !queue.is_empty() {
                let payload_to_return = queue.remove(0);
                if queue.is_empty() {
                    s.received_signals.remove(&signal_name);
                }
                // Publish the SignalReceived event so it gets checkpointed.
                publish_signal_event(
                    state,
                    signal_name.clone(),
                    payload_to_return.clone(),
                    operation_id,
                )
                .await;
                let _ = reply.send(Ok(payload_to_return));
                return;
            }
        }
        // No buffered signal — register as a pending waiter.
        state
            .pending_signal_calls
            .insert(signal_name, reply);
    }
}

async fn publish_signal_event(
    state: &mut InstanceState,
    signal_name: String,
    payload: Bytes,
    _operation_id: u32,
) {
    let event = WorkflowEvent::SignalReceived {
        signal_name,
        payload,
    };
    if let Some(store) = &state.args.event_store {
        if let Ok(seq) = store
            .publish(&state.args.namespace, &state.args.instance_id, event.clone())
            .await
        {
            let _ = handlers::inject_event(state, seq, &event).await;
        }
    }
}

async fn append_and_inject_timer_event(
    state: &mut InstanceState,
    event: WorkflowEvent,
    timer_id: wtf_common::TimerId,
    reply: ractor::RpcReplyPort<Result<(), WtfError>>,
) {
    let store = match &state.args.event_store {
        Some(s) => s,
        None => {
            let _ = reply.send(Err(WtfError::nats_publish("Event store missing")));
            return;
        }
    };

    match store
        .publish(
            &state.args.namespace,
            &state.args.instance_id,
            event.clone(),
        )
        .await
    {
        Ok(seq) => {
            state.pending_timer_calls.insert(timer_id, reply);
            let _ = handlers::inject_event(state, seq, &event).await;
        }
        Err(e) => {
            let _ = reply.send(Err(e));
        }
    }
}

#[cfg(test)]
#[path = "procedural_tests.rs"]
mod tests;
