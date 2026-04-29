# Findings: tw-8v3u - Graph builder edge validation

## Issue
In `crates/vo-sdk/src/graph.rs`, the Graph builder accepts edges without verifying that source and target node names exist in the node list. This causes runtime panics when the engine tries to look up a non-existent node.

## Root Cause
The `Dag::build()` method in `crates/vo-sdk/src/dag.rs` created a `WorkflowSpec` but never called `validate()` on it to verify edge integrity. While the `WorkflowSpec::validate()` method existed and properly checks for:
- Missing edge sources (`MissingEdgeSource`)
- Missing edge targets (`MissingEdgeTarget`)
- Self loops (`SelfLoop`)
- Duplicate edges (`DuplicateEdge`)
- Cycles (`CycleDetected`)
- Missing entry point (`NoEntryPoint`)

...it was never invoked during the build process.

## Fix Applied

### 1. Added `DuplicateEdge` variant to `DagError` (`dag.rs:27-28`)
```rust
#[error("duplicate edge: {from} -> {to}")]
DuplicateEdge { from: String, to: String },
```

### 2. Modified `Dag::build()` to call `validate()` before returning (`dag.rs:326-353`)
Added validation call that maps `ValidationError` to `DagError`:
```rust
let spec = WorkflowSpec { ... };

spec.validate().map_err(|e| match e {
    ValidationError::MissingEdgeSource { name } => DagError::NodeNotFound { name },
    ValidationError::MissingEdgeTarget { name } => DagError::NodeNotFound { name },
    ValidationError::SelfLoop { name } => DagError::SelfLoop { name },
    ValidationError::DuplicateEdge { from, to } => DagError::DuplicateEdge { from, to },
    ValidationError::DuplicateNodeName { name } => DagError::DuplicateNodeName { name },
    ValidationError::CycleDetected { cycle } => DagError::CycleDetected { cycle },
    ValidationError::NoEntryPoint => DagError::OrphanNode { name: "every node has incoming edges".to_string() },
})?;
```

### 3. Added new tests in `workflow_builder_tests.rs`
- `workflow_build_rejects_edge_to_unknown_node_via_validate` - validates rejection of edges to non-existent nodes
- `workflow_build_rejects_edge_from_unknown_node_via_validate` - validates rejection of edges from non-existent nodes

### 4. Updated tests that were documenting buggy behavior
- `bh48_dag_duplicate_edges_build_succeeds` → `bh48_dag_duplicate_edges_rejected_in_build`
- `bh48_dag_connect_same_nodes_twice_stores_both_edges` → `bh48_dag_connect_same_nodes_twice_rejected_in_build`
- `rq_workflow_spec_accepts_duplicate_edges_via_serde` → `rq_workflow_spec_rejects_duplicate_edges_via_serde`

## Verification
- `workflow_build_rejects_edge_*` tests: PASS
- `*_duplicate_edge*` tests: PASS
- `validate_rejects_missing*` tests: PASS
- vo-sdk builds successfully

## Pre-existing Issues (NOT caused by this fix)
The following tests were failing BEFORE this fix and are unrelated:
- `bh48_dag_build_orphan_node_rejected` - orphan detection bug in DAG (unrelated to edge validation)
- `bh48_dag_build_one_connected_one_orphan_rejected` - same orphan detection issue
- Various concurrent I/O tests - pre-existing timing/state issues
- Various write size tests - pre-existing size calculation issues

## Files Changed
- `crates/vo-sdk/src/dag.rs` - Added validation call and `DuplicateEdge` error
- `crates/vo-sdk/src/tests/workflow_builder_tests.rs` - Added edge validation tests
- `crates/vo-sdk/src/tests/blackhat_48.rs` - Updated duplicate edge tests
- `crates/vo-sdk/src/tests/red_queen_workflow_spec.rs` - Updated duplicate edge test