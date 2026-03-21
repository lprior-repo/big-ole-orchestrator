//! WorkflowInstance actor — per-instance ractor actor with two-phase lifecycle (ADR-016).
//!
//! # Two-Phase Lifecycle
//! 1. **Replay Phase** (`pre_start`): load snapshot from sled, create replay consumer,
//!    replay events from JetStream up to stream tail. No I/O effects during replay.
//! 2. **Live Phase** (after tail): re-subscribe to live JetStream events.
//!
//! # Snapshot trigger (ADR-019)
//! Every 100 events, the actor resets `events_since_snapshot` counter and triggers
//! a snapshot write (stub — full impl in wtf-flbh).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use wtf_common::WorkflowEvent;

use std::sync::Arc;
use std::collections::HashMap;
use ractor::RpcReplyPort;
use wtf_common::{ActivityId, WtfError};

pub mod lifecycle;

use self::lifecycle::ParadigmState;
use crate::messages::{
    InstanceArguments, InstanceMsg, InstancePhase, InstancePhaseView, InstanceStatusSnapshot,
    WorkflowParadigm,
};

/// In-memory state of a running WorkflowInstance.
#[derive(Debug)]
pub struct InstanceState {
    /// Immutable spawn arguments.
    pub args: InstanceArguments,
    /// Current execution phase (Replay or Live).
    pub phase: InstancePhase,
    /// Total events applied (monotonically increasing).
    pub total_events_applied: u64,
    /// Events since last snapshot (reset at SNAPSHOT_INTERVAL).
    pub events_since_snapshot: u32,
    /// Current state of the execution paradigm.
    pub paradigm_state: ParadigmState,

    /// Pending RPC calls from procedural workflows waiting for activity results.
    /// Keyed by ActivityId. Not persisted in snapshots.
    pub pending_activity_calls: HashMap<ActivityId, RpcReplyPort<Result<Bytes, WtfError>>>,

    /// Pending RPC calls from procedural workflows waiting for timers.
    /// Keyed by TimerId. Not persisted in snapshots.
    pub pending_timer_calls: HashMap<wtf_common::TimerId, RpcReplyPort<Result<(), WtfError>>>,

    /// Join handle for the procedural workflow task.
    pub procedural_task: Option<tokio::task::JoinHandle<()>>,
}

impl InstanceState {
    /// Create the initial state for a new workflow instance.
    #[must_use]
    pub fn initial(args: InstanceArguments) -> Self {
        let paradigm_state = initialize_paradigm_state(&args);
        Self {
            args,
            phase: InstancePhase::Replay,
            total_events_applied: 0,
            events_since_snapshot: 0,
            paradigm_state,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        }
    }
}

/// Write a snapshot every 100 events (ADR-019).
pub const SNAPSHOT_INTERVAL: u32 = 100;

/// The WorkflowInstance ractor actor.
pub struct WorkflowInstance;

#[async_trait]
impl Actor for WorkflowInstance {
    type Msg = InstanceMsg;
    type State = InstanceState;
    type Arguments = InstanceArguments;

    async fn pre_start(
        &self,
        myself: ActorRef<InstanceMsg>,
        args: InstanceArguments,
    ) -> Result<InstanceState, ActorProcessingErr> {
        tracing::info!(
            instance_id = %args.instance_id,
            namespace = %args.namespace,
            workflow_type = %args.workflow_type,
            paradigm = ?args.paradigm,
            "WorkflowInstance starting"
        );

        let mut state = InstanceState::initial(args);
        let mut event_log = Vec::new();

        // 1. Replay events from JetStream
        if let Some(nats) = &state.args.nats {
            let js = nats.jetstream();
            let mut consumer = wtf_storage::create_replay_consumer(
                js,
                &state.args.namespace,
                &state.args.instance_id,
                &wtf_storage::ReplayConfig::default(),
            )
            .await
            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;

            loop {
                match consumer.next_event().await {
                    Ok(wtf_storage::ReplayBatch::Event(replayed)) => {
                        event_log.push(replayed.event.clone());
                        state.paradigm_state = state
                            .paradigm_state
                            .apply_event(&replayed.event, replayed.seq, InstancePhase::Replay)
                            .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
                        state.total_events_applied += 1;
                    }
                    Ok(wtf_storage::ReplayBatch::TailReached) => break,
                    Err(e) => return Err(ActorProcessingErr::from(Box::new(e))),
                }
            }

            tracing::info!(
                instance_id = %state.args.instance_id,
                events = state.total_events_applied,
                "Replay complete"
            );

            // 2. Transition to Live phase
            let actions = lifecycle::compute_live_transition(
                &state.args.instance_id,
                state.args.paradigm,
                &state.paradigm_state,
                &event_log,
            );

            // 3. Execute transition side effects (re-dispatches, re-arm timers)
            if let Ok(timers_kv) = js.get_key_value(wtf_storage::bucket_names::TIMERS).await {
                lifecycle::execute_transition_actions(nats, &timers_kv, actions)
                    .await
                    .map_err(|e| ActorProcessingErr::from(Box::new(e)))?;
            }
        }

        state.phase = InstancePhase::Live;

        // 4. Start heartbeat timer
        myself.send_interval(std::time::Duration::from_secs(5), || InstanceMsg::Heartbeat);

        // 5. If procedural, spawn the workflow task
        if state.args.paradigm == WorkflowParadigm::Procedural {
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
        }

        Ok(state)
    }

    async fn handle(
        &self,
        _myself: ActorRef<InstanceMsg>,
        msg: InstanceMsg,
        state: &mut InstanceState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            InstanceMsg::InjectEvent { seq, event } => {
                handle_inject_event(state, seq, &event).await?;

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
                let checkpoint = if let ParadigmState::Procedural(s) = &state.paradigm_state {
                    s.get_checkpoint(operation_id).cloned()
                } else {
                    None
                };
                let _ = reply.send(checkpoint);
            }
            InstanceMsg::ProceduralDispatch {
                activity_type,
                payload,
                reply,
            } => {
                if let ParadigmState::Procedural(_) = &state.paradigm_state {
                    // 1. Generate ActivityId
                    let op_id = if let ParadigmState::Procedural(s) = &state.paradigm_state {
                        s.operation_counter
                    } else {
                        0
                    };
                    let activity_id = ActivityId::procedural(&state.args.instance_id, op_id);

                    // 2. Append ActivityDispatched to JetStream
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
                                // 3. Store reply port for later
                                state.pending_activity_calls.insert(activity_id, reply);
                                // 4. Apply event to state
                                let _ = handle_inject_event(state, seq, &event).await;
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
            InstanceMsg::ProceduralSleep { duration, reply } => {
                if let ParadigmState::Procedural(_) = &state.paradigm_state {
                    // 1. Generate TimerId
                    let timer_id = wtf_common::TimerId::new(ulid::Ulid::new().to_string());
                    let fire_at = chrono::Utc::now() + chrono::Duration::from_std(duration)
                        .unwrap_or_else(|_| chrono::Duration::zero());

                    // 2. Append TimerScheduled to JetStream
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
                                // 3. Store reply port
                                state.pending_timer_calls.insert(timer_id, reply);
                                // 4. Apply event
                                let _ = handle_inject_event(state, seq, &event).await;
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
            InstanceMsg::ProceduralWorkflowCompleted => {
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
                // Stop the actor
                _myself.stop(Some("workflow completed".to_string()));
            }
            InstanceMsg::ProceduralWorkflowFailed(err) => {
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
                _myself.stop(Some("workflow failed".to_string()));
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

async fn handle_inject_event(
    state: &mut InstanceState,
    _seq: u64,
    _event: &WorkflowEvent,
) -> Result<(), ActorProcessingErr> {
    state.total_events_applied += 1;
    state.events_since_snapshot += 1;

    if state.events_since_snapshot >= SNAPSHOT_INTERVAL {
        tracing::debug!(
            instance_id = %state.args.instance_id,
            total = state.total_events_applied,
            "snapshot trigger (stub — see wtf-flbh)"
        );
        state.events_since_snapshot = 0;
    }

    Ok(())
}

fn initialize_paradigm_state(args: &InstanceArguments) -> ParadigmState {
    match args.paradigm {
        WorkflowParadigm::Fsm => ParadigmState::Fsm(crate::fsm::FsmActorState::new("Initial")),
        WorkflowParadigm::Dag => ParadigmState::Dag(crate::dag::DagActorState::new(std::collections::HashMap::new())),
        WorkflowParadigm::Procedural => ParadigmState::Procedural(crate::procedural::ProceduralActorState::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wtf_common::InstanceId;

    fn test_args(paradigm: WorkflowParadigm) -> InstanceArguments {
        InstanceArguments {
            namespace: wtf_common::NamespaceId::new("test"),
            instance_id: wtf_common::InstanceId::new("inst-01"),
            workflow_type: "order_flow".into(),
            paradigm,
            input: bytes::Bytes::from_static(b"{}"),
            engine_node_id: "node-1".into(),
            nats: None,
            procedural_workflow: None,
        }
    }

    #[test]
    fn snapshot_interval_is_100() {
        assert_eq!(SNAPSHOT_INTERVAL, 100);
    }

    #[test]
    fn initialize_paradigm_state_returns_valid_variant() {
        let args = test_args(WorkflowParadigm::Fsm);
        let s = initialize_paradigm_state(&args);
        assert!(matches!(s, ParadigmState::Fsm(_)));
    }

    #[tokio::test]
    async fn handle_inject_event_increments_counters() {
        let args = test_args(WorkflowParadigm::Fsm);
        let mut state = InstanceState {
            paradigm_state: initialize_paradigm_state(&args),
            args,
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: 0,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        };
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handle_inject_event(&mut state, 1, &event)
            .await
            .expect("ok");
        assert_eq!(state.total_events_applied, 1);
        assert_eq!(state.events_since_snapshot, 1);
    }

    #[tokio::test]
    async fn snapshot_resets_counter_at_interval() {
        let args = test_args(WorkflowParadigm::Fsm);
        let mut state = InstanceState {
            paradigm_state: initialize_paradigm_state(&args),
            args,
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: SNAPSHOT_INTERVAL - 1,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        };
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handle_inject_event(&mut state, 1, &event)
            .await
            .expect("ok");
        assert_eq!(state.events_since_snapshot, 0);
        assert_eq!(state.total_events_applied, 1);
    }

    #[tokio::test]
    async fn snapshot_does_not_reset_before_interval() {
        let args = test_args(WorkflowParadigm::Dag);
        let mut state = InstanceState {
            paradigm_state: initialize_paradigm_state(&args),
            args,
            phase: InstancePhase::Live,
            total_events_applied: 50,
            events_since_snapshot: 50,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        };
        let event = WorkflowEvent::SnapshotTaken {
            seq: 51,
            checksum: 0,
        };
        handle_inject_event(&mut state, 51, &event)
            .await
            .expect("ok");
        assert_eq!(state.events_since_snapshot, 51);
        assert_eq!(state.total_events_applied, 51);
    }
}
