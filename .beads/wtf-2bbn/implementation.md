bead_id: wtf-2bbn
bead_title: integration test: Procedural checkpoint — ctx.activity() result survives crash
phase: implementation
updated_at: 2026-03-22T03:10:00Z

# Implementation: Procedural Crash Recovery Integration Tests

## Test File Location
`crates/wtf-actor/tests/procedural_crash_replay.rs`

## Tests Implemented

### 1. `checkpoint_persists_across_crash_state_machine`
Verifies that after completing ops 0 and 1, the checkpoint_map correctly stores their results with the expected operation_counter of 2.

### 2. `op_counter_deterministic_after_replay`
Verifies that the operation_counter is deterministic - replaying the same sequence of events produces the same final counter value.

### 3. `exactly_once_activity_dispatch_via_checkpoint_map`
Verifies that when an activity is re-dispatched at a sequence that was already applied, the state machine returns `AlreadyApplied` - ensuring exactly-once semantics.

### 4. `instance_completes_after_all_ops_checkpointed`
Verifies that after all 3 operations complete, all checkpoints are stored and the operation_counter reaches 3.

### 5. `replay_after_crash_restores_checkpoint_state`
Simulates crash recovery by applying events up to op 2 dispatch, then re-applying the same dispatch and verifying idempotency.

### 6. `checkpoint_map_sequential_ops_correct_order`
Verifies that sequential operations get incrementing operation IDs (0, 1, 2) and checkpoints are recorded in order.

### 7. `crash_recovery_skips_completed_ops_and_dispatches_next`
Verifies that after completing ops 0 and 1 (with checkpoints), dispatching op 2 correctly returns `ActivityDispatched` with `operation_id: 2`.

## Key Behaviors Verified

1. **Checkpoint Persistence**: Completed operations are stored in `checkpoint_map` with their results
2. **Deterministic Op Counter**: `operation_counter` is deterministic based on event sequence
3. **Exactly-Once Dispatch**: Already-applied events return `AlreadyApplied` (idempotency)
4. **Sequential Operation IDs**: Operations get IDs 0, 1, 2, ... in dispatch order
5. **Crash Recovery**: After crash, checkpointed ops are NOT re-dispatched, next op IS dispatched

## Test Results
All 7 tests pass.
