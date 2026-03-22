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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};
    use crate::instance::state::InstanceState;
    use crate::instance::lifecycle::ParadigmState;
    use bytes::Bytes;
    use std::collections::HashMap;
    use wtf_common::{NamespaceId, InstanceId};

    #[tokio::test]
    async fn get_checkpoint_returns_none_for_empty_state() {
        let args = InstanceArguments {
            namespace: NamespaceId::new("ns"),
            instance_id: InstanceId::new("i1"),
            workflow_type: "wf".into(),
            paradigm: WorkflowParadigm::Procedural,
            input: Bytes::new(),
            engine_node_id: "n1".into(),
            nats: None,
            snapshot_db: None,
            procedural_workflow: None,
            workflow_definition: None,
        };
        let state = InstanceState {
            paradigm_state: ParadigmState::Procedural(crate::procedural::ProceduralActorState::new()),
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: 0,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
            args,
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        handle_get_checkpoint(&state, 0, tx.into()).await;
        let result = rx.await.expect("reply");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_checkpoint_returns_some_after_activity_completed() {
        use crate::procedural::state::apply_event;
        use wtf_common::WorkflowEvent;

        let dispatch_ev = WorkflowEvent::ActivityDispatched {
            activity_id: "i1:0".into(),
            activity_type: "work".into(),
            payload: Bytes::new(),
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        };
        let complete_ev = WorkflowEvent::ActivityCompleted {
            activity_id: "i1:0".into(),
            result: Bytes::from_static(b"done"),
            duration_ms: 1,
        };

        let s0 = crate::procedural::ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch_ev, 1).expect("dispatch");
        let (s2, _) = apply_event(&s1, &complete_ev, 2).expect("complete");

        let args = InstanceArguments {
            namespace: NamespaceId::new("ns"),
            instance_id: InstanceId::new("i1"),
            workflow_type: "wf".into(),
            paradigm: WorkflowParadigm::Procedural,
            input: Bytes::new(),
            engine_node_id: "n1".into(),
            nats: None,
            snapshot_db: None,
            procedural_workflow: None,
            workflow_definition: None,
        };
        let state = InstanceState {
            paradigm_state: ParadigmState::Procedural(s2),
            phase: InstancePhase::Live,
            total_events_applied: 2,
            events_since_snapshot: 2,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
            args,
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        handle_get_checkpoint(&state, 0, tx.into()).await;
        let result = rx.await.expect("reply");
        assert!(result.is_some(), "checkpoint must be present after ActivityCompleted");
        assert_eq!(result.expect("checkpoint present").result, Bytes::from_static(b"done"));
    }
}
