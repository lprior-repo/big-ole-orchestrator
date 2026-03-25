//! Initialization and replay logic for `WorkflowInstance` actors.

#![allow(clippy::missing_errors_doc, clippy::unused_async)]

use super::lifecycle::{
    compute_live_transition, deserialize_paradigm_state, execute_transition_actions, ParadigmState,
};
use super::state::InstanceState;
use crate::messages::{InstanceArguments, InstanceMsg, InstancePhase};
use ractor::{ActorProcessingErr, ActorRef};
use std::sync::Arc;
use wtf_common::storage::{ReplayBatch, ReplayStream, TaskQueue};
use wtf_common::WorkflowEvent;

pub async fn load_initial_state(
    args: InstanceArguments,
) -> Result<(InstanceState, u64), ActorProcessingErr> {
    let mut state = InstanceState::initial(args.clone());
    let mut from_seq = 1;

    if let Some(db) = &args.snapshot_db {
        if let Ok(Some(snap)) = wtf_storage::snapshots::read_snapshot(db, &args.instance_id) {
            state.paradigm_state = deserialize_paradigm_state(args.paradigm, &snap.state_bytes)?;
            state.total_events_applied = snap.seq;
            from_seq = snap
                .seq
                .checked_add(1)
                .ok_or_else(|| ActorProcessingErr::from("Snapshot sequence overflow"))?;
            tracing::info!(instance_id = %args.instance_id, seq = snap.seq, "Snapshot loaded");
        }
    }
    Ok((state, from_seq))
}

pub async fn replay_events(
    state: &mut InstanceState,
    from_seq: u64,
) -> Result<(Vec<WorkflowEvent>, Option<Box<dyn ReplayStream>>), ActorProcessingErr> {
    let store = state
        .args
        .event_store
        .as_ref()
        .ok_or_else(|| ActorProcessingErr::from("No event store available"))?;

    let mut consumer = store
        .open_replay_stream(&state.args.namespace, &state.args.instance_id, from_seq)
        .await
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

    let mut event_log = Vec::new();

    while let Some(event) = next_replayed_event(consumer.as_mut(), state).await? {
        event_log.push(event);
    }

    tracing::info!(
        instance_id = %state.args.instance_id,
        events = state.total_events_applied,
        "Replay complete"
    );
    Ok((event_log, Some(consumer)))
}

async fn next_replayed_event(
    consumer: &mut dyn ReplayStream,
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

pub async fn transition_to_live(
    args: &InstanceArguments,
    paradigm_state: &ParadigmState,
    event_log: &[WorkflowEvent],
    task_queue: &dyn TaskQueue,
) -> Result<(), ActorProcessingErr> {
    let actions =
        compute_live_transition(&args.instance_id, args.paradigm, paradigm_state, event_log);

    if let Some(store) = &args.state_store {
        execute_transition_actions(task_queue, store.as_ref(), actions)
            .await
            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
    }
    Ok(())
}

pub fn spawn_live_subscription(
    state: &mut InstanceState,
    myself: &ActorRef<InstanceMsg>,
    mut consumer: Box<dyn ReplayStream>,
) {
    let myself_clone = myself.clone();
    let handle = tokio::spawn(async move {
        while let Ok(replayed) = consumer.next_live_event().await {
            let _ = myself_clone.cast(InstanceMsg::InjectEvent {
                seq: replayed.seq,
                event: replayed.event,
            });
        }
    });
    state.live_subscription_task = Some(handle);
}

pub async fn start_procedural_workflow(
    state: &mut InstanceState,
    myself: &ActorRef<InstanceMsg>,
) -> Result<(), ActorProcessingErr> {
    if let Some(wf_fn) = &state.args.procedural_workflow {
        let ctx = crate::procedural::WorkflowContext::new(
            state.args.instance_id.clone(),
            0,
            myself.clone(),
        );
        let handle = tokio::spawn(run_procedural(Arc::clone(wf_fn), ctx, myself.clone()));
        state.procedural_task = Some(handle);
    }
    Ok(())
}

async fn run_procedural(
    wf_fn: Arc<dyn crate::procedural::WorkflowFn>,
    ctx: crate::procedural::WorkflowContext,
    myself: ActorRef<InstanceMsg>,
) {
    match wf_fn.execute(ctx).await {
        Ok(()) => {
            let _ = myself.cast(InstanceMsg::ProceduralWorkflowCompleted);
        }
        Err(e) => {
            let _ = myself.cast(InstanceMsg::ProceduralWorkflowFailed(e.to_string()));
        }
    }
}

/// Publish the `InstanceStarted` event for a fresh (non-replayed) instance.
/// Must be called AFTER `spawn_live_subscription` and BEFORE phase transitions to Live.
///
/// # Arguments
/// * `args` - The `InstanceArguments` containing namespace, `instance_id`, `workflow_type`, input
/// * `event_log` - The replayed events (empty = fresh instance, non-empty = crash recovery)
///
/// # Returns
/// * `Ok(())` - Event published successfully, or skipped (crash recovery)
/// * `Err(ActorProcessingErr)` - If no `event_store` is configured or publish fails
///
/// # Guards
/// - Returns `Ok(())` immediately if `event_log` is non-empty (crash recovery path)
/// - Returns `Err` if `args.event_store` is `None`
pub async fn publish_instance_started(
    args: &InstanceArguments,
    from_seq: u64,
    event_log: &[WorkflowEvent],
) -> Result<(), ActorProcessingErr> {
    if should_skip_instance_started(from_seq, event_log) {
        return Ok(());
    }

    let store = args
        .event_store
        .as_ref()
        .ok_or_else(|| ActorProcessingErr::from("No event store available for InstanceStarted publish"))?;

    let event = WorkflowEvent::InstanceStarted {
        instance_id: args.instance_id.to_string(),
        workflow_type: args.workflow_type.clone(),
        input: args.input.clone(),
    };

    let seq = store
        .publish(&args.namespace, &args.instance_id, event)
        .await
        .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

    debug_assert!(
        seq >= 1,
        "EventStore returned invalid sequence number: {seq} (must be >= 1)"
    );

    tracing::info!(
        instance_id = %args.instance_id,
        "InstanceStarted event published"
    );

    Ok(())
}

#[must_use]
pub fn should_skip_instance_started(from_seq: u64, event_log: &[WorkflowEvent]) -> bool {
    // from_seq == 0 is invalid: treat as skip to avoid blocking recovery
    // (DEFECT-2: explicit handling of illegal value)
    from_seq == 0 || from_seq > 1 || !event_log.is_empty()
}
