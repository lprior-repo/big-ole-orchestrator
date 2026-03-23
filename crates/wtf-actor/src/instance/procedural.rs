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
mod tests {
    use super::*;
    use crate::instance::lifecycle::ParadigmState;
    use crate::instance::state::InstanceState;
    use crate::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};
    use bytes::Bytes;
    use std::collections::HashMap;
    use wtf_common::{InstanceId, NamespaceId};

    #[tokio::test]
    async fn get_checkpoint_returns_none_for_empty_state() {
        let args = InstanceArguments {
            namespace: NamespaceId::new("ns"),
            instance_id: InstanceId::new("i1"),
            workflow_type: "wf".into(),
            paradigm: WorkflowParadigm::Procedural,
            input: Bytes::new(),
            engine_node_id: "n1".into(),
            event_store: None,
            state_store: None,
            task_queue: None,
            snapshot_db: None,
            procedural_workflow: None,
            workflow_definition: None,
        };
        let state = InstanceState {
            paradigm_state: ParadigmState::Procedural(
                crate::procedural::ProceduralActorState::new(),
            ),
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: 0,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
            live_subscription_task: None,
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
            event_store: None,
            state_store: None,
            task_queue: None,
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
            live_subscription_task: None,
            args,
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        handle_get_checkpoint(&state, 0, tx.into()).await;
        let result = rx.await.expect("reply");
        assert!(
            result.is_some(),
            "checkpoint must be present after ActivityCompleted"
        );
        assert_eq!(
            result.expect("checkpoint present").result,
            Bytes::from_static(b"done")
        );
    }
}
