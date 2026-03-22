# Implementation Summary

bead_id: wtf-bj9
bead_title: bead: handle StartWorkflow message
phase: implementation
updated_at: 2026-03-21T03:50:00Z

## Files Changed

### `crates/wtf-actor/src/master.rs`
- Added `validate_workflow_name()` - validates non-empty workflow names
- Added `capacity_check()` - checks if running_count < max_concurrent
- Added `generate_invocation_id()` - generates ULID-based invocation IDs
- Added `spawn_workflow()` - spawns linked WorkflowInstance actors
- Added `handle_start_workflow()` - implements StartWorkflow handling per contract
- Added `handle_terminate()` - implements Terminate handling
- Added `handle_list_workflows()` - implements ListWorkflows handling
- Updated Actor impl with `handle()` method to dispatch messages

### `crates/wtf-actor/src/instance.rs`
- Added `WorkflowInstance` struct with `new()` constructor
- Added `InstanceState` for actor state
- Added Actor trait implementation for WorkflowInstance

### `crates/wtf-core/src/types.rs`
- Updated `InstanceConfig` to remove storage field (storage bead not yet implemented)

## Contract Clause Mapping

| Contract Clause | Implementation |
|-----------------|----------------|
| P1 (max_concurrent > 0) | Enforced in `MasterOrchestrator::new()` |
| P2 (running_count accessible) | `OrchestratorState.running_count` field |
| P3 (name non-empty) | `validate_workflow_name()` returns `StartError::EmptyWorkflowName` |
| P4 (input is Vec<u8>) | Compile-time enforcement via type system |
| P5 (reply channel open) | Handled by ractor framework |
| Q1 (AtCapacity error) | `handle_start_workflow()` checks capacity and returns error |
| Q2 (spawn_workflow called) | `spawn_workflow()` called after capacity check |
| Q3 (running_count incremented) | `state.running_count += 1` after successful spawn |
| Q4 (invocation_id replied) | `reply.send(Ok(invocation_id))` with generated ULID |
| Q5 (error propagated) | Spawn failures return `StartError::SpawnFailed` without incrementing count |
| Q6 (returns Ok(())) | Function always returns `Ok(())` |

## Test Coverage

Added unit tests:
- `test_validate_workflow_name_accepts_non_empty` - Happy path validation
- `test_validate_workflow_name_rejects_empty` - Error case validation
- `test_capacity_check_returns_true_when_under_limit` - Capacity check
- `test_capacity_check_returns_false_when_at_limit` - At capacity case
- `test_generate_invocation_id_returns_valid_ulid` - ULID generation

All 7 existing tests continue to pass.

## Implementation Notes

1. **Storage abstraction deferred**: InstanceConfig storage field removed since the storage abstraction (StorageDb) is not yet implemented. This is a temporary simplification.

2. **Supervisor handling**: The handle_supervisor_evt for decrementing running_count on actor termination is not yet implemented - will be added in a subsequent bead.

3. **Other message variants**: GetStatus, Signal, and other OrchestratorMsg variants are not yet implemented - the handle method has placeholder cases for them.

## Iteration 2 (contract hardening)

- StartWorkflow validation path now has explicit unit tests ensuring capacity guard behavior and acceptance behavior under available capacity.
- Verification rerun via targeted actor handler tests.
