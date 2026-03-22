bead_id: wtf-2bbn
bead_title: integration test: Procedural checkpoint — ctx.activity() result survives crash
phase: martin-fowler-tests
updated_at: 2026-03-22T03:00:00Z

# Martin Fowler Test Plan: Procedural Checkpoint Crash Recovery

## Test Class: ProceduralCrashReplay

### Test: checkpoint_persists_across_crash

**Given** a procedural workflow with 3 sequential ctx.activity() calls (validate_order, charge_card, send_confirmation)

**When** I complete operations 0 and 1 via worker and then kill the engine after op 1 is ACKed

**Then** after restarting the engine, operations 0 and 1 are NOT re-dispatched (checkpoint_map has their results)

**And** operation 2 (send_confirmation) IS dispatched

**And** after completing op 2, the instance reaches InstanceCompleted state

### Test: checkpoint_map_hydration_from_jetstream

**Given** a workflow that has been checkpointed with ops 0 and 1 complete

**When** the engine restarts and replays from JetStream

**Then** the checkpoint_map MUST be populated with the results of op 0 and op 1

**And** the op_counter MUST be at 2

### Test: exactly_once_activity_dispatch

**Given** a checkpointed workflow with completed activities

**When** the engine replays

**Then** each activity MUST be dispatched at most once

**And** no activity result in checkpoint_map should be recomputed

### Test: deterministic_op_counter

**Given** a checkpoint with ops 0 and 1 complete

**When** the engine replays

**Then** the op_counter MUST be exactly 2

**And** the next dispatched activity MUST be op 2

### Test: instance_completes_after_all_ops

**Given** a checkpointed workflow where ops 0 and 1 are complete

**When** I restart the engine and complete op 2

**Then** the instance MUST reach InstanceCompleted terminal state

**And** no further activities should be dispatched

## Given-When-Then Scenarios

### Scenario 1: Normal Crash Recovery

**Given** workflow with 3 sequential activities
**When** engine crashes after op 1 ACK
**And** engine restarts
**Then** ops 0-1 NOT re-dispatched
**And** op 2 IS dispatched
**And** instance completes

### Scenario 2: All Ops Complete Before Crash

**Given** workflow with 3 sequential activities
**When** all 3 ops complete before crash
**And** engine crashes after op 2 ACK
**Then** on restart, no activities dispatched
**And** instance immediately reaches InstanceCompleted

### Scenario 3: Crash During Op Execution

**Given** workflow with 3 sequential activities
**When** engine crashes during op execution (not yet ACKed)
**Then** on restart, that op IS re-dispatched (not in checkpoint_map yet)

## Edge Cases

1. Crash between op 1 ACK and persisting checkpoint - Verify idempotency
2. Multiple rapid restarts - Verify checkpoint not corrupted
3. Worker dies and comes back - Verify exactly-once semantics held
