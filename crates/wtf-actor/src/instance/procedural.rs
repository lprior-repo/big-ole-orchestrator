//! Procedural-specific message handlers for WorkflowInstance.

use bytes::Bytes;
use ractor::ActorRef;
use wtf_common::{ActivityId, WtfError, WorkflowEvent};
use crate::messages::InstanceMsg;
use super::state::InstanceState;
use super::lifecycle::ParadigmState;

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
    if let ParadigmState::Procedural(_) = &state.paradigm_state {
        let op_id = if let ParadigmState::Procedural(s) = &state.paradigm_state {
            s.operation_counter
        } else {
            0
        };
        let activity_id = ActivityId::procedural(&state.args.instance_id, op_id);

        let event = WorkflowEvent::ActivityDispatched {
            activity_id: activity_id.to_string(),
            activity_type,
            payload,
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        };

        if let Some(nats) = &state.args.nats {
            let js = nats.jetstream();
            match wtf_storage::append_event(
                js,
                &state.args.namespace,
                &state.args.instance_id,
                &event,
            )
            .await
            {
                Ok(seq) => {
                    state.pending_activity_calls.insert(activity_id, reply);
                    let _ = super::handle_inject_event(state, seq, &event).await;
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        } else {
            let _ = reply.send(Err(WtfError::nats_publish("NATS missing")));
        }
    }
}

pub async fn handle_sleep(
    state: &mut InstanceState,
    duration: std::time::Duration,
    reply: ractor::RpcReplyPort<Result<(), WtfError>>,
) {
    if let ParadigmState::Procedural(_) = &state.paradigm_state {
        let timer_id = wtf_common::TimerId::new(ulid::Ulid::new().to_string());
        let fire_at = chrono::Utc::now() + chrono::Duration::from_std(duration)
            .unwrap_or_else(|_| chrono::Duration::zero());

        let event = WorkflowEvent::TimerScheduled {
            timer_id: timer_id.to_string(),
            fire_at,
        };

        if let Some(nats) = &state.args.nats {
            let js = nats.jetstream();
            match wtf_storage::append_event(
                js,
                &state.args.namespace,
                &state.args.instance_id,
                &event,
            )
            .await
            {
                Ok(seq) => {
                    state.pending_timer_calls.insert(timer_id, reply);
                    let _ = super::handle_inject_event(state, seq, &event).await;
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        } else {
            let _ = reply.send(Err(WtfError::nats_publish("NATS missing")));
        }
    }
}

pub async fn handle_completed(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
) {
    tracing::info!(instance_id = %state.args.instance_id, "Procedural workflow completed");
    let event = WorkflowEvent::InstanceCompleted {
        output: bytes::Bytes::new(),
    };
    if let Some(nats) = &state.args.nats {
        let _ = wtf_storage::append_event(
            nats.jetstream(),
            &state.args.namespace,
            &state.args.instance_id,
            &event,
        )
        .await;
    }
    myself_ref.stop(Some("workflow completed".to_string()));
}

pub async fn handle_failed(
    myself_ref: ActorRef<InstanceMsg>,
    state: &InstanceState,
    err: String,
) {
    tracing::error!(instance_id = %state.args.instance_id, error = %err, "Procedural workflow failed");
    let event = WorkflowEvent::InstanceFailed { error: err };
    if let Some(nats) = &state.args.nats {
        let _ = wtf_storage::append_event(
            nats.jetstream(),
            &state.args.namespace,
            &state.args.instance_id,
            &event,
        )
        .await;
    }
    myself_ref.stop(Some("workflow failed".to_string()));
}
