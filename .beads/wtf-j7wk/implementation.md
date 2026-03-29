# Implementation Summary

bead_id: vo-j7wk
bead_title: "vo-frontend: Simulate Mode Procedural — step through ctx calls, show checkpoint map"
phase: implementation
updated_at: 2026-03-21T17:45:00Z

## Implementation Overview

Created `crates/vo-frontend/src/ui/simulate_mode.rs` with:

### Core Types

- **`SimProceduralState`**: UI state struct containing:
  - `checkpoint_map: HashMap<String, String>` — accumulated key→value pairs
  - `current_op: u32` — index into ordered ops list
  - `event_log: Vec<SimWorkflowEvent>` — append-only event log

- **`SimWorkflowEvent`**: Event enum (matches vo_common::WorkflowEvent structure):
  - `ActivityDispatched`, `ActivityCompleted`, `ActivityFailed`
  - `TimerScheduled`, `TimerFired`
  - `SignalReceived`

- **`SimOp`**: Operation enum representing ctx calls:
  - `CtxActivity { activity_id, activity_type }`
  - `CtxSleep { timer_id, duration_ms }`
  - `CtxWaitSignal { signal_name }`

- **`SimError`**: Error enum:
  - `EmptyResult` — result input was empty
  - `AlreadyCompleted` — simulation at terminal state
  - `NoOpsAvailable` — workflow has zero ctx operations

### API

- `SimProceduralState::new()` — creates empty state
- `SimProceduralState::provide_result(result, activity_id, total_ops)` — advances simulation
- `SimProceduralState::can_advance(total_ops)` — checks if more ops available
- `SimProceduralState::current_op_index()` — returns current op index
- `extract_ctx_ops_from_workflow(workflow)` — extracts ctx ops from workflow graph (placeholder)

### Design Decisions

1. Used `Bytes` for payload fields to match `vo_common::WorkflowEvent`
2. Used `chrono::DateTime<Utc>` for timer timestamps
3. `provide_result` validates preconditions before mutating state
4. All operations are fallible and return `Result`

### Module Integration

- Added `pub mod simulate_mode;` to `ui/mod.rs`
- Exported `SimError, SimOp, SimProceduralState, SimWorkflowEvent` from ui module

### Tests

- 17 unit tests covering all contract requirements
- Tests verify initial state, provide_result behavior, error cases, and invariants

### Notes

- The `extract_ctx_ops_from_workflow` function is a placeholder (returns empty Vec)
- The Dioxus UI component (SimulateProcedural) will be implemented in a subsequent bead
- The `SimWorkflowEvent` type mirrors `vo_common::WorkflowEvent` for compatibility
