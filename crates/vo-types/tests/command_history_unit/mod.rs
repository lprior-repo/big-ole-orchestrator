//! Command History Unit Tests - Module Root
//!
//! Split from single 1005-line file into focused submodules by test domain.
//! All 52 behaviors (B-001 to B-052) and 13 invariants (INV-001 to INV-013) preserved.

use vo_types::command_history::{
    CommandHistory, CommandHistoryError, CommandKind, ExtensionApplyMode, HistoryEntryStatus,
    WorkflowSnapshot, MAX_HISTORY_DEPTH,
};
use vo_types::{DagNode, Edge, EdgeCondition, NodeName, RetryPolicy};

// ---------------------------------------------------------------------------
// Shared test helpers
// ---------------------------------------------------------------------------

fn make_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, 1000, 2.0).unwrap()
}

fn make_node(name: &str) -> DagNode {
    DagNode {
        node_name: NodeName::parse(name).unwrap(),
        retry_policy: make_retry_policy(),
        compensation_policy: None,
        capability: Default::default(),
    }
}

fn make_snapshot(workflow_name: &str, nodes: Vec<DagNode>, edges: Vec<Edge>) -> WorkflowSnapshot {
    WorkflowSnapshot::new(workflow_name.into(), nodes, edges)
}

fn test_snapshot() -> WorkflowSnapshot {
    make_snapshot("test-workflow", vec![make_node("test-node")], vec![])
}

// ---------------------------------------------------------------------------
// Submodules
// ---------------------------------------------------------------------------

mod types_exhaustiveness;
mod snapshots;
mod history_init;
mod save_undo;
mod undo;
mod redo;
mod undo_redo;
mod apply_command;
mod invariants;
mod errors;
mod display;
mod capacity;
mod command_ids;
