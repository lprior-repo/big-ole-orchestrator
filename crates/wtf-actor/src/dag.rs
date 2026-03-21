//! DagActor — DAG paradigm actor state and event application (ADR-017).
//!
//! The DAG paradigm dispatches activities in dependency order. A node becomes
//! ready when ALL its predecessors appear in `completed`. Multiple nodes may be
//! ready simultaneously — the actor dispatches them in parallel.
//!
//! On replay: `ActivityCompleted` events rebuild `completed`; `ActivityDispatched`
//! rebuilds `in_flight`. `ready_nodes()` is called after replay to determine
//! what was in-flight at crash time (already dispatched, waiting for results).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use wtf_common::{ActivityId, WorkflowEvent};

/// A node in the workflow DAG.
///
/// `predecessors` lists the `NodeId`s that must complete before this node
/// may be dispatched. An empty list means the node is a root (runs immediately).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DagNode {
    /// The activity type to dispatch (e.g. `"charge_card"`).
    pub activity_type: String,
    /// IDs of nodes that must complete before this node can be dispatched.
    pub predecessors: Vec<NodeId>,
}

/// Stable, author-assigned identifier for a node in the workflow DAG.
///
/// In events, this corresponds to `activity_id` in `ActivityDispatched`.
/// The workflow definition assigns these IDs; they are stable across replay.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&ActivityId> for NodeId {
    fn from(id: &ActivityId) -> Self {
        Self(id.as_str().to_owned())
    }
}

/// In-memory state for a DAG workflow actor.
///
/// This is a pure cache of the JetStream event log. All fields are derivable
/// by replaying `WorkflowEvent` records from the stream (ADR-016).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagActorState {
    /// DAG topology: every node and its predecessors.
    ///
    /// Set once at instance creation from the workflow definition.
    /// Immutable during replay and live execution.
    pub nodes: HashMap<NodeId, DagNode>,

    /// Completed nodes: `NodeId` → result bytes.
    pub completed: HashMap<NodeId, Bytes>,

    /// Nodes dispatched but not yet completed.
    pub in_flight: HashSet<NodeId>,

    /// Permanently failed nodes (retries exhausted). These block successors.
    pub failed: HashSet<NodeId>,

    /// JetStream sequence numbers already applied (idempotency — ADR-016).
    pub applied_seq: HashSet<u64>,

    /// Events processed since the last snapshot.
    pub events_since_snapshot: u32,
}

impl DagActorState {
    /// Create a new DAG actor state with the given topology.
    ///
    /// `nodes` is built from the workflow definition before the actor starts.
    #[must_use]
    pub fn new(nodes: HashMap<NodeId, DagNode>) -> Self {
        Self {
            nodes,
            completed: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            applied_seq: HashSet::new(),
            events_since_snapshot: 0,
        }
    }
}

/// Compute the set of nodes that are ready to dispatch.
///
/// A node is ready when:
/// - It is not already completed, in-flight, or permanently failed.
/// - ALL of its predecessors are in `completed`.
///
/// Returns node IDs in deterministic sorted order (for reproducible dispatch).
#[must_use]
pub fn ready_nodes(state: &DagActorState) -> Vec<NodeId> {
    let mut ready: Vec<NodeId> = state
        .nodes
        .iter()
        .filter(|(id, node)| {
            !state.completed.contains_key(*id)
                && !state.in_flight.contains(*id)
                && !state.failed.contains(*id)
                && node
                    .predecessors
                    .iter()
                    .all(|pred| state.completed.contains_key(pred))
        })
        .map(|(id, _)| id.clone())
        .collect();

    // Deterministic order — important for reproducible dispatch sequences.
    ready.sort_by(|a, b| a.0.cmp(&b.0));
    ready
}

/// Check whether the DAG has reached a terminal state.
///
/// Returns `true` if all nodes are completed (success) or if any node
/// has permanently failed (failure). Use `is_failed(state)` to distinguish.
#[must_use]
pub fn is_terminal(state: &DagActorState) -> bool {
    is_succeeded(state) || is_failed(state)
}

/// Returns `true` if all nodes completed successfully.
#[must_use]
pub fn is_succeeded(state: &DagActorState) -> bool {
    state
        .nodes
        .keys()
        .all(|id| state.completed.contains_key(id))
}

/// Returns `true` if any node has permanently failed (blocking the DAG).
#[must_use]
pub fn is_failed(state: &DagActorState) -> bool {
    !state.failed.is_empty()
}

/// Result of applying a single event to DAG state.
#[derive(Debug, Clone)]
pub enum DagApplyResult {
    /// Event was already applied (duplicate delivery) — state unchanged.
    AlreadyApplied,
    /// No meaningful change (informational event).
    None,
    /// Activity completed — caller should check `ready_nodes()` for new work.
    ActivityCompleted { node_id: NodeId, result: Bytes },
    /// Activity permanently failed — DAG cannot complete.
    ActivityFailed { node_id: NodeId },
}

/// Error applying an event.
#[derive(Debug, thiserror::Error)]
pub enum DagApplyError {
    #[error("activity_completed for unknown node: {0}")]
    UnknownNode(String),
}

/// Apply a single `WorkflowEvent` to the DAG actor state.
///
/// Returns `(new_state, result)`. The caller checks `ready_nodes()` after
/// `ActivityCompleted` to discover newly unblocked nodes.
///
/// # Idempotency
/// If `seq` is already in `applied_seq`, returns `AlreadyApplied` without
/// mutating state.
///
/// # Errors
/// Returns [`DagApplyError::UnknownNode`] if `ActivityCompleted` references
/// a node not in `state.nodes` (indicates a malformed event log).
pub fn apply_event(
    state: &DagActorState,
    event: &WorkflowEvent,
    seq: u64,
) -> Result<(DagActorState, DagApplyResult), DagApplyError> {
    if state.applied_seq.contains(&seq) {
        return Ok((state.clone(), DagApplyResult::AlreadyApplied));
    }

    let result = match event {
        WorkflowEvent::ActivityDispatched { activity_id, .. } => {
            let mut next = state.clone();
            next.in_flight.insert(NodeId::new(activity_id));
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;
            (next, DagApplyResult::None)
        }

        WorkflowEvent::ActivityCompleted {
            activity_id,
            result,
            ..
        } => {
            let node_id = NodeId::new(activity_id);

            // Validate the node exists (guards against corrupted event logs).
            if !state.nodes.contains_key(&node_id) {
                return Err(DagApplyError::UnknownNode(activity_id.clone()));
            }

            let mut next = state.clone();
            next.in_flight.remove(&node_id);
            next.completed.insert(node_id.clone(), result.clone());
            next.applied_seq.insert(seq);
            next.events_since_snapshot += 1;

            (
                next,
                DagApplyResult::ActivityCompleted {
                    node_id,
                    result: result.clone(),
                },
            )
        }

        WorkflowEvent::ActivityFailed {
            activity_id,
            retries_exhausted,
            ..
        } => {
            let node_id = NodeId::new(activity_id);
            let mut next = state.clone();

            if *retries_exhausted {
                next.in_flight.remove(&node_id);
                next.failed.insert(node_id.clone());
                next.applied_seq.insert(seq);
                next.events_since_snapshot += 1;
                (next, DagApplyResult::ActivityFailed { node_id })
            } else {
                // Not exhausted — will be retried; stay in-flight logically.
                next.applied_seq.insert(seq);
                next.events_since_snapshot += 1;
                (next, DagApplyResult::None)
            }
        }

        WorkflowEvent::SnapshotTaken { .. } => {
            let mut next = state.clone();
            next.applied_seq.insert(seq);
            next.events_since_snapshot = 0;
            (next, DagApplyResult::None)
        }

        // All other events are valid in the log but do not affect DAG state.
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
            (next, DagApplyResult::None)
        }
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a linear A → B → C DAG.
    fn linear_dag() -> HashMap<NodeId, DagNode> {
        let mut nodes = HashMap::new();
        nodes.insert(
            NodeId::new("A"),
            DagNode {
                activity_type: "task_a".into(),
                predecessors: vec![],
            },
        );
        nodes.insert(
            NodeId::new("B"),
            DagNode {
                activity_type: "task_b".into(),
                predecessors: vec![NodeId::new("A")],
            },
        );
        nodes.insert(
            NodeId::new("C"),
            DagNode {
                activity_type: "task_c".into(),
                predecessors: vec![NodeId::new("B")],
            },
        );
        nodes
    }

    /// Build a parallel DAG: A and B run in parallel, C waits for both.
    fn parallel_dag() -> HashMap<NodeId, DagNode> {
        let mut nodes = HashMap::new();
        nodes.insert(
            NodeId::new("A"),
            DagNode {
                activity_type: "task_a".into(),
                predecessors: vec![],
            },
        );
        nodes.insert(
            NodeId::new("B"),
            DagNode {
                activity_type: "task_b".into(),
                predecessors: vec![],
            },
        );
        nodes.insert(
            NodeId::new("C"),
            DagNode {
                activity_type: "task_c".into(),
                predecessors: vec![NodeId::new("A"), NodeId::new("B")],
            },
        );
        nodes
    }

    fn completed_event(id: &str) -> WorkflowEvent {
        WorkflowEvent::ActivityCompleted {
            activity_id: id.into(),
            result: Bytes::from_static(b"ok"),
            duration_ms: 10,
        }
    }

    fn dispatched_event(id: &str) -> WorkflowEvent {
        WorkflowEvent::ActivityDispatched {
            activity_id: id.into(),
            activity_type: "task".into(),
            payload: Bytes::new(),
            retry_policy: wtf_common::RetryPolicy::default(),
            attempt: 1,
        }
    }

    fn failed_event(id: &str, exhausted: bool) -> WorkflowEvent {
        WorkflowEvent::ActivityFailed {
            activity_id: id.into(),
            error: "boom".into(),
            retries_exhausted: exhausted,
        }
    }

    #[test]
    fn root_nodes_ready_on_empty_state() {
        let state = DagActorState::new(linear_dag());
        let ready = ready_nodes(&state);
        assert_eq!(ready, vec![NodeId::new("A")]);
    }

    #[test]
    fn parallel_roots_both_ready() {
        let state = DagActorState::new(parallel_dag());
        let ready = ready_nodes(&state);
        // Sorted order: A, B (C not ready — has predecessors)
        assert_eq!(ready, vec![NodeId::new("A"), NodeId::new("B")]);
    }

    #[test]
    fn in_flight_node_not_ready() {
        let state = DagActorState::new(linear_dag());
        let event = dispatched_event("A");
        let (s1, _) = apply_event(&state, &event, 1).expect("apply");
        let ready = ready_nodes(&s1);
        // A is in-flight, B blocked, C blocked
        assert!(ready.is_empty());
    }

    #[test]
    fn completed_unblocks_successor() {
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &dispatched_event("A"), 1).expect("dispatch A");
        let (s2, _) = apply_event(&s1, &completed_event("A"), 2).expect("complete A");
        let ready = ready_nodes(&s2);
        assert_eq!(ready, vec![NodeId::new("B")]);
    }

    #[test]
    fn parallel_waits_for_both_predecessors() {
        let state = DagActorState::new(parallel_dag());
        let (s1, _) = apply_event(&state, &completed_event("A"), 1).expect("complete A");
        let ready = ready_nodes(&s1);
        // B still not done, C blocked
        assert!(ready.iter().any(|n| n == &NodeId::new("B")));
        assert!(!ready.iter().any(|n| n == &NodeId::new("C")));

        let (s2, _) = apply_event(&s1, &completed_event("B"), 2).expect("complete B");
        let ready2 = ready_nodes(&s2);
        assert_eq!(ready2, vec![NodeId::new("C")]);
    }

    #[test]
    fn duplicate_seq_returns_already_applied() {
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &completed_event("A"), 1).expect("first");
        let (_, result) = apply_event(&s1, &completed_event("A"), 1).expect("duplicate");
        assert!(matches!(result, DagApplyResult::AlreadyApplied));
    }

    #[test]
    fn activity_failed_exhausted_adds_to_failed() {
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &dispatched_event("A"), 1).expect("dispatch");
        let (s2, result) = apply_event(&s1, &failed_event("A", true), 2).expect("fail exhausted");
        assert!(matches!(result, DagApplyResult::ActivityFailed { .. }));
        assert!(s2.failed.contains(&NodeId::new("A")));
        assert!(!s2.in_flight.contains(&NodeId::new("A")));
    }

    #[test]
    fn activity_failed_not_exhausted_stays_in_flight_logically() {
        // Retry failure doesn't add to failed set — activity will be retried.
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &dispatched_event("A"), 1).expect("dispatch");
        let (s2, _) = apply_event(&s1, &failed_event("A", false), 2).expect("fail retry");
        assert!(!s2.failed.contains(&NodeId::new("A")));
    }

    #[test]
    fn failed_node_blocks_successors() {
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &failed_event("A", true), 1).expect("fail A");
        let ready = ready_nodes(&s1);
        // A is failed, B blocked, C blocked — nothing ready
        assert!(ready.is_empty());
        assert!(is_failed(&s1));
    }

    #[test]
    fn is_succeeded_when_all_complete() {
        let state = DagActorState::new(linear_dag());
        let (s1, _) = apply_event(&state, &completed_event("A"), 1).expect("A");
        let (s2, _) = apply_event(&s1, &completed_event("B"), 2).expect("B");
        let (s3, _) = apply_event(&s2, &completed_event("C"), 3).expect("C");
        assert!(is_succeeded(&s3));
        assert!(is_terminal(&s3));
        assert!(!is_failed(&s3));
    }

    #[test]
    fn snapshot_taken_resets_events_since_snapshot() {
        let mut state = DagActorState::new(linear_dag());
        state.events_since_snapshot = 77;
        let event = WorkflowEvent::SnapshotTaken {
            seq: 5,
            checksum: 0,
        };
        let (next, _) = apply_event(&state, &event, 6).expect("snapshot");
        assert_eq!(next.events_since_snapshot, 0);
    }

    #[test]
    fn unknown_node_completion_returns_error() {
        let state = DagActorState::new(linear_dag());
        let result = apply_event(&state, &completed_event("GHOST"), 1);
        assert!(matches!(result, Err(DagApplyError::UnknownNode(_))));
    }
}
