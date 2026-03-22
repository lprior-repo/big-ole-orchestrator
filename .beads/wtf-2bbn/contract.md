bead_id: wtf-2bbn
bead_title: integration test: Procedural checkpoint — ctx.activity() result survives crash
phase: contract
updated_at: 2026-03-22T03:00:00Z

# Contract Specification: Procedural Checkpoint Crash Recovery

## Overview

This bead implements an integration test that verifies the checkpoint map correctly persists activity results across engine crashes, ensuring exactly-once activity dispatch semantics.

## Test Scenario

1. Start procedural workflow with 3 sequential `ctx.activity()` calls
2. Complete op 0 (validate_order) and op 1 (charge_card) via worker
3. Kill engine after op 1 ACKed
4. Restart engine
5. Assert op 0 and op 1 NOT re-dispatched (checkpoint_map has results)
6. Assert op 2 (send_confirmation) IS dispatched
7. Complete op 2
8. Assert InstanceCompleted

## Contract: Checkpoint Persistence

### Preconditions
- Workflow instance created with 3 activity nodes in sequence
- Activities 0 and 1 completed successfully
- Engine shutdown occurs after op 1 ACK received

### Postconditions
- `checkpoint_map` contains results for op 0 and op 1
- `op_counter` is at 2 (next to execute)
- On restart, only op 2 is dispatched
- No duplicate activity execution occurs
- Instance completes successfully after op 2

## State Machine Invariants

1. **Deterministic Op Counter**: After completing ops 0 and 1, `op_counter` MUST be 2
2. **Checkpoint Hydration**: On replay, checkpoint_map MUST be populated from JetStream replay
3. **Exactly-Once Dispatch**: Activity MUST NOT be dispatched if already in checkpoint_map
4. **Sequential Dispatch**: op 2 only dispatched after checkpoint_map has ops 0 and 1

## Error Taxonomy

| Error Type | Condition | Expected Behavior |
|------------|-----------|-------------------|
| REPLAY_DUPLICATE | Activity dispatched during replay when in checkpoint_map | FAIL - must not dispatch |
| CHECKPOINT_MISS | Activity result not found in checkpoint_map on replay | FAIL - checkpoint must hydrate |
| OP_COUNTER_INVALID | op_counter not at expected value after replay | FAIL - counter must be deterministic |
| INSTANCE_NOT_COMPLETED | Instance fails to complete after all ops done | FAIL - instance must reach terminal state |

## Acceptance Criteria

- [ ] Test file created at `tests/integration/procedural_crash_replay.rs`
- [ ] Test kills/restarts engine process
- [ ] No duplicate activity execution detected
- [ ] Checkpoint map correctly hydrated from JetStream
- [ ] Instance completes successfully
- [ ] All assertions pass
