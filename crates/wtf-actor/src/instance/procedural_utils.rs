//! Utilities and completion handlers for procedural workflows.

use super::handlers;
use super::lifecycle::ParadigmState;
use super::state::InstanceState;
use crate::messages::InstanceMsg;
use ractor::ActorRef;
use wtf_common::WorkflowEvent;

pub async fn handle_now(
    state: &mut InstanceState,
    operation_id: u32,
    reply: ractor::RpcReplyPort<chrono::DateTime<chrono::Utc>>,
) {
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let op_id = operation_id;
        if let Some(cp) = s.get_checkpoint(op_id) {
            let millis = i64::from_le_bytes(cp.result.as_ref().try_into().unwrap_or([0u8; 8]));
            let ts =
                chrono::DateTime::from_timestamp_millis(millis).unwrap_or_else(chrono::Utc::now);
            let _ = reply.send(ts);
            return;
        }
        let ts = chrono::Utc::now();
        let event = WorkflowEvent::NowSampled {
            operation_id: op_id,
            ts,
        };
        if let Some(store) = &state.args.event_store {
            if let Ok(seq) = store
                .publish(
                    &state.args.namespace,
                    &state.args.instance_id,
                    event.clone(),
                )
                .await
            {
                let _ = handlers::inject_event(state, seq, &event).await;
                let _ = reply.send(ts);
            }
            // If publish failed, drop reply — caller will timeout/error rather than get a
            // non-deterministic value that wasn't persisted.
        }
        // If no event_store, drop reply — operation cannot be made deterministic.
    }
}

pub async fn handle_random(
    state: &mut InstanceState,
    operation_id: u32,
    reply: ractor::RpcReplyPort<u64>,
) {
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let op_id = operation_id;
        if let Some(cp) = s.get_checkpoint(op_id) {
            let value = u64::from_le_bytes(cp.result.as_ref().try_into().unwrap_or([0u8; 8]));
            let _ = reply.send(value);
            return;
        }
        let value: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as u64 ^ (d.as_secs() << 32));
        let event = WorkflowEvent::RandomSampled {
            operation_id: op_id,
            value,
        };
        if let Some(store) = &state.args.event_store {
            if let Ok(seq) = store
                .publish(
                    &state.args.namespace,
                    &state.args.instance_id,
                    event.clone(),
                )
                .await
            {
                let _ = handlers::inject_event(state, seq, &event).await;
                let _ = reply.send(value);
            }
            // If publish failed, drop reply — caller will timeout/error rather than get a
            // non-deterministic value that wasn't persisted.
        }
        // If no event_store, drop reply — operation cannot be made deterministic.
    }
}

pub async fn handle_completed(myself_ref: ActorRef<InstanceMsg>, state: &InstanceState) {
    tracing::info!(instance_id = %state.args.instance_id, "Procedural workflow completed");
    let event = WorkflowEvent::InstanceCompleted {
        output: bytes::Bytes::new(),
    };
    if let Some(store) = &state.args.event_store {
        let _ = store
            .publish(&state.args.namespace, &state.args.instance_id, event)
            .await;
    }
    myself_ref.stop(Some("workflow completed".to_string()));
}

pub async fn handle_failed(myself_ref: ActorRef<InstanceMsg>, state: &InstanceState, err: String) {
    tracing::error!(instance_id = %state.args.instance_id, error = %err, "Procedural workflow failed");
    let event = WorkflowEvent::InstanceFailed { error: err };
    if let Some(store) = &state.args.event_store {
        let _ = store
            .publish(&state.args.namespace, &state.args.instance_id, event)
            .await;
    }
    myself_ref.stop(Some("workflow failed".to_string()));
}
