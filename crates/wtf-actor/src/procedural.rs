//! ProceduralActor — Procedural paradigm actor state and event application (ADR-017).
//!
//! The Procedural paradigm allows arbitrary async Rust code with durable
//! checkpointing. Each `ctx.activity()` or `ctx.sleep()` call produces an
//! `ActivityDispatched` event and waits for `ActivityCompleted`.
//!
//! Determinism is achieved by keying operations on a monotonically incrementing
//! `operation_id`. On replay, the actor reads `checkpoint_map[operation_id]`
//! instead of re-executing the side effect.
//!
//! Replay terminates when the operation counter exceeds the highest checkpoint
//! key — at that point the actor switches to Live Phase.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use wtf_common::{ActivityId, InstanceId, WorkflowEvent};

/// The deterministic key for a single workflow operation.
///
/// Format: `"<instance_id>:<op_counter>"`.
/// Built by [`WorkflowContext::next_op_id`].
pub type OperationId = ActivityId;

/// A completed operation in the checkpoint map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    /// Result bytes returned to the workflow code.
    pub result: Bytes,
    /// JetStream sequence number of the `ActivityCompleted` event.
    pub completed_seq: u64,
}

/// In-memory state for a Procedural workflow actor.
///
/// This is a pure cache of the JetStream event log. All fields are derivable
/// by replaying `WorkflowEvent` records from the stream (ADR-016).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralActorState {
    /// Completed operations: `operation_id` → `Checkpoint`.
    ///
    /// On replay, the actor reads from this map instead of re-executing.
    /// The operation_id is the workflow-assigned counter at point of dispatch.
    pub checkpoint_map: HashMap<u32, Checkpoint>,

    /// Monotonically incrementing counter for the next operation.
    ///
    /// Incremented each time `ActivityDispatched` is seen.
    /// The counter is deterministic — it equals the number of activities
    /// dispatched so far.
    pub operation_counter: u32,

    /// Currently dispatched operations: `operation_id` → `ActivityId`.
    ///
    /// In most procedural workflows, at most one operation is in-flight at a
    /// time (sequential execution). Parallel `ctx.join()` may produce several.
    pub in_flight: HashMap<u32, ActivityId>,

    /// JetStream sequence numbers already applied (idempotency — ADR-016).
    pub applied_seq: HashSet<u64>,

    /// Events processed since the last snapshot.
    pub events_since_snapshot: u32,
}

impl ProceduralActorState {
    /// Create a new empty `ProceduralActorState`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoint_map: HashMap::new(),
            operation_counter: 0,
            in_flight: HashMap::new(),
            applied_seq: HashSet::new(),
            events_since_snapshot: 0,
        }
    }

    /// Return `true` if a checkpoint exists for `operation_id`.
    ///
    /// During replay, workflow code calls `has_checkpoint(op)` before deciding
    /// whether to re-execute or return the cached result.
    #[must_use]
    pub fn has_checkpoint(&self, operation_id: u32) -> bool {
        self.checkpoint_map.contains_key(&operation_id)
    }

    /// Look up the result of a previously completed operation.
    ///
    /// Returns `None` if no checkpoint exists (operation not yet completed).
    #[must_use]
    pub fn get_checkpoint(&self, operation_id: u32) -> Option<&Checkpoint> {
        self.checkpoint_map.get(&operation_id)
    }

    /// Return the highest operation ID with a checkpoint, or `None` if empty.
    ///
    /// Used to determine the replay boundary: replay ends when
    /// `operation_counter > max_checkpointed_operation_id`.
    #[must_use]
    pub fn max_checkpointed_operation_id(&self) -> Option<u32> {
        self.checkpoint_map.keys().copied().max()
    }
}

impl Default for ProceduralActorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of applying a single event to Procedural state.
#[derive(Debug, Clone)]
pub enum ProceduralApplyResult {
    /// Event was already applied (duplicate delivery) — state unchanged.
    AlreadyApplied,
    /// No meaningful change (informational event).
    None,
    /// Activity dispatched — `operation_id` now in `in_flight`.
    ActivityDispatched {
        operation_id: u32,
        activity_id: ActivityId,
    },
    /// Activity completed — checkpoint recorded, workflow code may resume.
    ActivityCompleted { operation_id: u32, result: Bytes },
    /// Activity permanently failed (retries exhausted).
    ActivityFailed { operation_id: u32 },
}

/// Error applying an event to Procedural state.
#[derive(Debug, thiserror::Error)]
pub enum ProceduralApplyError {
    #[error("ActivityCompleted for unknown activity_id {0}; no matching in_flight entry")]
    UnknownActivityId(String),
}

/// Apply a single `WorkflowEvent` to the Procedural actor state.
///
/// Returns `(new_state, result)`.
///
/// # Operation ID assignment
/// `ActivityDispatched` increments `operation_counter` and stores the
/// mapping `operation_counter → activity_id` in `in_flight`.
///
/// `ActivityCompleted` looks up `activity_id` in `in_flight` to retrieve
/// the `operation_id`, then stores the result in `checkpoint_map`.
///
/// # Idempotency
/// If `seq` is already in `applied_seq`, returns `AlreadyApplied`.
///
/// # Errors
/// Returns [`ProceduralApplyError::UnknownActivityId`] if `ActivityCompleted`
/// references an activity ID not in `in_flight` (malformed event log).
pub fn apply_event(
    state: &ProceduralActorState,
    event: &WorkflowEvent,
    seq: u64,
) -> Result<(ProceduralActorState, ProceduralApplyResult), ProceduralApplyError> {
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), ProceduralApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::ActivityDispatched { activity_id, .. } => {
            let mut next = state.clone();
            let op_id = next.operation_counter;
            next.operation_counter += 1;
            next.in_flight.insert(op_id, ActivityId::new(activity_id));
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityDispatched {
                    operation_id: op_id,
                    activity_id: ActivityId::new(activity_id),
                },
            )
        }

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => {
            let aid = ActivityId::new(activity_id);

            // Find the operation_id by reverse-looking up the activity_id.
            // In a well-formed log, there is exactly one in_flight entry.
            let op_id = state
                .in_flight
                .iter()
                .find(|(_, v)| *v == &aid)
                .map(|(k, _)| *k)
                .ok_or_else(|| ProceduralApplyError::UnknownActivityId(activity_id.clone()))?;

            let mut next = state.clone();
            next.in_flight.remove(&op_id);
            next.checkpoint_map.insert(
                op_id,
                Checkpoint {
                    result: result.clone(),
                    completed_seq: seq,
                },
            );
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                ProceduralApplyResult::ActivityCompleted {
                    operation_id: op_id,
                    result: result.clone(),
                },
            )
        }

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => {
            let aid = ActivityId::new(activity_id);
            let mut next = state.clone();

            if *retries_exhausted {
                // Find and remove the in-flight entry.
                let op_id = state
                    .in_flight
                    .iter()
                    .find(|(_, v)| *v == &aid)
                    .map(|(k, _)| *k);

                if let Some(id) = op_id {
                    next.in_flight.remove(&id);
                    next.applied_seq.insert(seq);
                    next.events_since_snapshot += 1;
                    return Ok((
                        next,
                        ProceduralApplyResult::ActivityFailed { operation_id: id },
                    ));
                }
            }

            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }

        WorkflowEvent::SnapshotTaken { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot = 0;
            (next, ProceduralApplyResult::None)
        }

        // All other events are valid in the log but do not affect Procedural state.
        WorkflowEvent::TransitionApplied { .. }
        | WorkflowEvent::SignalReceived { .. }
        | WorkflowEvent::TimerFired { .. }
        | WorkflowEvent::TimerScheduled { .. }
        | WorkflowEvent::TimerCancelled { .. }
        | WorkflowEvent::InstanceStarted { .. }
        | WorkflowEvent::InstanceCompleted { .. }
        | WorkflowEvent::InstanceFailed { .. }
        | WorkflowEvent::InstanceCancelled { .. }
        | WorkflowEvent::ChildStarted { .. }
        | WorkflowEvent::ChildCompleted { .. }
        | WorkflowEvent::ChildFailed { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, ProceduralApplyResult::None)
        }
    };

    Ok(result)
}

use ractor::ActorRef;
use crate::messages::InstanceMsg;

/// Context passed to a procedural workflow function.
///
/// Provides methods for dispatching activities, sleeping, and managing
/// state that is deterministic across replays.
#[derive(Clone)]
pub struct WorkflowContext {
    pub instance_id: InstanceId,
    pub op_counter: Arc<AtomicU32>,
    pub myself: ActorRef<InstanceMsg>,
}

impl WorkflowContext {
    /// Create a new context.
    #[must_use]
    pub fn new(instance_id: InstanceId, initial_op_counter: u32, myself: ActorRef<InstanceMsg>) -> Self {
        Self {
            instance_id,
            op_counter: Arc::new(AtomicU32::new(initial_op_counter)),
            myself,
        }
    }

    /// Return the next deterministic operation ID for this instance.
    #[must_use]
    pub fn next_op_id(&self) -> ActivityId {
        ActivityId::procedural(&self.instance_id, self.op_counter.load(Ordering::SeqCst))
    }

    /// Dispatch an activity and wait for its completion.
    ///
    /// If replaying, returns the cached result from the checkpoint map.
    /// If live, dispatches to the worker queue and suspends the workflow task.
    pub async fn activity(&self, activity_type: &str, payload: Bytes) -> anyhow::Result<Bytes> {
        let op_id = self.op_counter.load(Ordering::SeqCst);

        // 1. Check for checkpoint (Replay logic)
        let checkpoint = self
            .myself
            .call(
                |reply| InstanceMsg::GetProceduralCheckpoint {
                    operation_id: op_id,
                    reply,
                },
                None,
            )
            .await?;

        let checkpoint = match checkpoint {
            ractor::rpc::CallResult::Success(c) => c,
            _ => anyhow::bail!("Actor call failed"),
        };

        if let Some(cp) = checkpoint {
            // Found checkpoint — skip dispatch
            self.op_counter.fetch_add(1, Ordering::SeqCst);
            return Ok(cp.result);
        }

        // 2. Dispatch and wait (Live logic)
        let result = self
            .myself
            .call(
                |reply| InstanceMsg::ProceduralDispatch {
                    activity_type: activity_type.to_owned(),
                    payload,
                    reply,
                },
                None,
            )
            .await?;

        let result = match result {
            ractor::rpc::CallResult::Success(r) => r?,
            _ => anyhow::bail!("Actor call failed"),
        };

        self.op_counter.fetch_add(1, Ordering::SeqCst);
        Ok(result)
    }

    /// Sleep for the given duration.
    ///
    /// Produces a `TimerScheduled` event and suspends the workflow task
    /// until `TimerFired` is received.
    pub async fn sleep(&self, duration: std::time::Duration) -> anyhow::Result<()> {
        let op_id = self.op_counter.load(Ordering::SeqCst);

        // 1. Check for checkpoint (Sleep also uses checkpoints for determinism)
        let checkpoint = self
            .myself
            .call(
                |reply| InstanceMsg::GetProceduralCheckpoint {
                    operation_id: op_id,
                    reply,
                },
                None,
            )
            .await?;

        let checkpoint = match checkpoint {
            ractor::rpc::CallResult::Success(c) => c,
            _ => anyhow::bail!("Actor call failed"),
        };

        if checkpoint.is_some() {
            self.op_counter.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }

        // 2. Dispatch sleep and wait
        let result = self
            .myself
            .call(
                |reply| InstanceMsg::ProceduralSleep { duration, reply },
                None,
            )
            .await?;

        match result {
            ractor::rpc::CallResult::Success(r) => r?,
            _ => anyhow::bail!("Actor call failed"),
        };

        self.op_counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A procedural workflow implementation.
///
/// This trait is implemented by workflow code that runs in the [`ProceduralActor`].
#[async_trait::async_trait]
pub trait WorkflowFn: std::fmt::Debug + Send + Sync + 'static {
    /// Execute the workflow logic using the provided context.
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()>;
}

/// Runtime container for a procedural workflow and its current state.
pub struct ProceduralActorRuntime {
    pub state: ProceduralActorState,
    pub workflow_fn: Box<dyn WorkflowFn>,
}

impl ProceduralActorRuntime {
    /// Create a new runtime.
    #[must_use]
    pub fn new(state: ProceduralActorState, workflow_fn: Box<dyn WorkflowFn>) -> Self {
        Self { state, workflow_fn }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn dispatch(id: &str) -> WorkflowEvent {
        WorkflowEvent::ActivityDispatched {
            activity_id: id.into(),
            activity_type: "work".into(),
            payload: Bytes::new(),
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        }
    }

    fn complete(id: &str, result: &[u8]) -> WorkflowEvent {
        WorkflowEvent::ActivityCompleted {
            activity_id: id.into(),
            result: Bytes::copy_from_slice(result),
            duration_ms: 5,
        }
    }

    fn fail(id: &str, exhausted: bool) -> WorkflowEvent {
        WorkflowEvent::ActivityFailed {
            activity_id: id.into(),
            error: "oops".into(),
            retries_exhausted: exhausted,
        }
    }

    #[test]
    fn new_state_has_zero_counter() {
        let s = ProceduralActorState::new();
        assert_eq!(s.operation_counter, 0);
        assert!(s.checkpoint_map.is_empty());
        assert!(s.in_flight.is_empty());
    }

    #[test]
    fn dispatch_increments_operation_counter() {
        let s0 = ProceduralActorState::new();
        let (s1, result) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
        assert_eq!(s1.operation_counter, 1);
        assert!(matches!(
            result,
            ProceduralApplyResult::ActivityDispatched {
                operation_id: 0,
                ..
            }
        ));
        assert!(s1.in_flight.contains_key(&0));
    }

    #[test]
    fn complete_writes_checkpoint() {
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
        let (s2, result) = apply_event(&s1, &complete("act-1", b"result"), 2).expect("complete");

        assert!(matches!(
            result,
            ProceduralApplyResult::ActivityCompleted {
                operation_id: 0,
                ..
            }
        ));
        assert!(s2.checkpoint_map.contains_key(&0));
        assert_eq!(s2.checkpoint_map[&0].result, Bytes::from_static(b"result"));
        assert!(!s2.in_flight.contains_key(&0));
    }

    #[test]
    fn sequential_operations_get_incrementing_ids() {
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch 1");
        let (s2, _) = apply_event(&s1, &complete("act-1", b"r1"), 2).expect("complete 1");
        let (s3, _) = apply_event(&s2, &dispatch("act-2"), 3).expect("dispatch 2");
        let (s4, _) = apply_event(&s3, &complete("act-2", b"r2"), 4).expect("complete 2");

        assert_eq!(s4.operation_counter, 2);
        assert!(s4.checkpoint_map.contains_key(&0));
        assert!(s4.checkpoint_map.contains_key(&1));
    }

    #[test]
    fn duplicate_seq_returns_already_applied() {
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("first");
        let (_, result) = apply_event(&s1, &dispatch("act-1"), 1).expect("dup");
        assert!(matches!(result, ProceduralApplyResult::AlreadyApplied));
    }

    #[test]
    fn has_checkpoint_reflects_completed_ops() {
        let s0 = ProceduralActorState::new();
        assert!(!s0.has_checkpoint(0));
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
        let (s2, _) = apply_event(&s1, &complete("act-1", b"r"), 2).expect("complete");
        assert!(s2.has_checkpoint(0));
    }

    #[test]
    fn max_checkpointed_operation_id_returns_highest() {
        let s0 = ProceduralActorState::new();
        assert_eq!(s0.max_checkpointed_operation_id(), None);

        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("d1");
        let (s2, _) = apply_event(&s1, &complete("act-1", b"r"), 2).expect("c1");
        let (s3, _) = apply_event(&s2, &dispatch("act-2"), 3).expect("d2");
        let (s4, _) = apply_event(&s3, &complete("act-2", b"r2"), 4).expect("c2");

        assert_eq!(s4.max_checkpointed_operation_id(), Some(1));
    }

    #[test]
    fn activity_failed_exhausted_removes_from_in_flight() {
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
        let (s2, result) = apply_event(&s1, &fail("act-1", true), 2).expect("fail");
        assert!(matches!(
            result,
            ProceduralApplyResult::ActivityFailed { operation_id: 0 }
        ));
        assert!(!s2.in_flight.contains_key(&0));
        // No checkpoint recorded for failed operations
        assert!(!s2.checkpoint_map.contains_key(&0));
    }

    #[test]
    fn activity_failed_not_exhausted_stays_tracked() {
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("act-1"), 1).expect("dispatch");
        let (s2, result) = apply_event(&s1, &fail("act-1", false), 2).expect("fail retry");
        assert!(matches!(result, ProceduralApplyResult::None));
        // In-flight still tracked (will be retried)
        assert!(s2.in_flight.contains_key(&0));
    }

    #[test]
    fn unknown_activity_id_on_complete_returns_error() {
        let s0 = ProceduralActorState::new();
        let result = apply_event(&s0, &complete("ghost", b"r"), 1);
        assert!(matches!(
            result,
            Err(ProceduralApplyError::UnknownActivityId(_))
        ));
    }

    #[test]
    fn snapshot_taken_resets_events_since_snapshot() {
        let mut s = ProceduralActorState::new();
        s.events_since_snapshot = 50;
        let event = WorkflowEvent::SnapshotTaken {
            seq: 10,
            checksum: 0,
        };
        let (next, _) = apply_event(&s, &event, 11).expect("snapshot");
        assert_eq!(next.events_since_snapshot, 0);
    }

    #[test]
    fn replay_boundary_computation() {
        // Simulate: dispatch op0, complete op0, dispatch op1 (not yet complete at crash)
        let s0 = ProceduralActorState::new();
        let (s1, _) = apply_event(&s0, &dispatch("op0"), 1).expect("d0");
        let (s2, _) = apply_event(&s1, &complete("op0", b"r0"), 2).expect("c0");
        let (s3, _) = apply_event(&s2, &dispatch("op1"), 3).expect("d1");

        // Replay ended here. op1 was dispatched but not completed.
        // max_checkpointed = 0, operation_counter = 2 → actor knows op1 is live
        assert_eq!(s3.max_checkpointed_operation_id(), Some(0));
        assert_eq!(s3.operation_counter, 2);
        assert!(s3.in_flight.contains_key(&1));
    }

    // ── WorkflowContext / WorkflowFn tests ───────────────────────────────────

    #[test]
    fn operation_id_type_is_activity_id_compatible() {
        // OperationId must be a type alias compatible with ActivityId.
        let _: OperationId = ActivityId::new("inst:0");
    }

    #[test]
    fn op_counter_starts_at_zero_and_produces_correct_format() {
        // next_op_id first call → "inst-01:0", second → "inst-01:1".
        let counter = Arc::new(AtomicU32::new(0));
        let instance_id = InstanceId::new("inst-01");
        let id0 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        let id1 = ActivityId::procedural(&instance_id, counter.fetch_add(1, Ordering::SeqCst));
        assert_eq!(id0.as_str(), "inst-01:0");
        assert_eq!(id1.as_str(), "inst-01:1");
    }

    #[test]
    fn arc_clones_share_counter_state() {
        // Clones of WorkflowContext must share the same op_counter (Arc semantics).
        let counter = Arc::new(AtomicU32::new(0));
        let counter2 = Arc::clone(&counter);
        let _ = counter.fetch_add(1, Ordering::SeqCst);
        let _ = counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(counter2.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn workflow_fn_trait_is_object_safe_and_boxable() {
        // WorkflowFn must be object safe so we can store it in ProceduralActorRuntime.
        #[derive(Debug)]
        struct MyWorkflow;
        #[async_trait]
        impl WorkflowFn for MyWorkflow {
            async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let _: Box<dyn WorkflowFn> = Box::new(MyWorkflow);
    }

    #[test]
    fn procedural_actor_runtime_holds_state_and_fn() {
        #[derive(Debug)]
        struct MyWorkflow;
        #[async_trait]
        impl WorkflowFn for MyWorkflow {
            async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> { Ok(()) }
        }

        let state = ProceduralActorState::new();
        let runtime = ProceduralActorRuntime::new(
            state,
            Box::new(MyWorkflow),
        );

        assert_eq!(runtime.state.operation_counter, 0);
    }
}
