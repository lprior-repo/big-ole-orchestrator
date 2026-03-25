//! Utilities and completion handlers for procedural workflows.

#![allow(clippy::missing_errors_doc)]

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
) -> Result<(), ractor::ActorProcessingErr> {
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let op_id = operation_id;
        if let Some(cp) = s.get_checkpoint(op_id) {
            let millis = i64::from_le_bytes(cp.result.as_ref().try_into().unwrap_or([0u8; 8]));
            let ts =
                chrono::DateTime::from_timestamp_millis(millis).unwrap_or_else(chrono::Utc::now);
            let _ = reply.send(ts);
            return Ok(());
        }
        let ts = chrono::Utc::now();
        let event = WorkflowEvent::NowSampled {
            operation_id: op_id,
            ts,
        };
        if let Some(store) = &state.args.event_store {
            match store
                .publish(
                    &state.args.namespace,
                    &state.args.instance_id,
                    event.clone(),
                )
                .await
            {
                Ok(seq) => {
                    let _ = handlers::inject_event(state, seq, &event).await;
                    let _ = reply.send(ts);
                    Ok(())
                }
                Err(e) => {
                    // Log error but drop reply — caller will timeout/error rather than get a
                    // non-deterministic value that wasn't persisted.
                    tracing::error!(error = %e, "handle_now failed to publish event");
                    Err(ractor::ActorProcessingErr::from(Box::new(e)))
                }
            }
        } else {
            // No event_store — operation cannot be made deterministic.
            // Drop reply — caller will timeout/error rather than get a non-persisted value.
            tracing::error!("handle_now requires event_store for deterministic operation");
            Err(ractor::ActorProcessingErr::from(
                "Event store missing",
            ))
        }
    } else {
        Ok(())
    }
}

pub async fn handle_random(
    state: &mut InstanceState,
    operation_id: u32,
    reply: ractor::RpcReplyPort<u64>,
) -> Result<(), ractor::ActorProcessingErr> {
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let op_id = operation_id;
        if let Some(cp) = s.get_checkpoint(op_id) {
            let value = u64::from_le_bytes(cp.result.as_ref().try_into().unwrap_or([0u8; 8]));
            let _ = reply.send(value);
            return Ok(());
        }
        let value: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::from(d.subsec_nanos()) ^ (d.as_secs() << 32));
        let event = WorkflowEvent::RandomSampled {
            operation_id: op_id,
            value,
        };
        if let Some(store) = &state.args.event_store {
            match store
                .publish(
                    &state.args.namespace,
                    &state.args.instance_id,
                    event.clone(),
                )
                .await
            {
                Ok(seq) => {
                    let _ = handlers::inject_event(state, seq, &event).await;
                    let _ = reply.send(value);
                    Ok(())
                }
                Err(e) => {
                    // Log error but drop reply — caller will timeout/error rather than get a
                    // non-deterministic value that wasn't persisted.
                    tracing::error!(error = %e, "handle_random failed to publish event");
                    Err(ractor::ActorProcessingErr::from(Box::new(e)))
                }
            }
        } else {
            // No event_store — operation cannot be made deterministic.
            // Drop reply — caller will timeout/error rather than get a non-persisted value.
            tracing::error!("handle_random requires event_store for deterministic operation");
            Err(ractor::ActorProcessingErr::from(
                "Event store missing",
            ))
        }
    } else {
        Ok(())
    }
}

pub async fn handle_completed(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
) -> Result<(), ractor::ActorProcessingErr> {
    tracing::info!(instance_id = %state.args.instance_id, "Procedural workflow completed");
    let event = WorkflowEvent::InstanceCompleted {
        output: bytes::Bytes::new(),
    };
    if let Some(store) = &state.args.event_store {
        match store
            .publish(&state.args.namespace, &state.args.instance_id, event)
            .await
        {
            Ok(_) => {
                myself_ref.stop(Some("workflow completed".to_string()));
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "handle_completed failed to publish event");
                Err(ractor::ActorProcessingErr::from(Box::new(e)))
            }
        }
    } else {
        Err(ractor::ActorProcessingErr::from(
            "Event store missing",
        ))
    }
}

pub async fn handle_failed(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
    err: String,
) -> Result<(), ractor::ActorProcessingErr> {
    tracing::error!(instance_id = %state.args.instance_id, error = %err, "Procedural workflow failed");
    let event = WorkflowEvent::InstanceFailed { error: err };
    if let Some(store) = &state.args.event_store {
        match store
            .publish(&state.args.namespace, &state.args.instance_id, event)
            .await
        {
            Ok(_) => {
                myself_ref.stop(Some("workflow failed".to_string()));
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "handle_failed failed to publish event");
                Err(ractor::ActorProcessingErr::from(Box::new(e)))
            }
        }
    } else {
        Err(ractor::ActorProcessingErr::from(
            "Event store missing",
        ))
    }
}
