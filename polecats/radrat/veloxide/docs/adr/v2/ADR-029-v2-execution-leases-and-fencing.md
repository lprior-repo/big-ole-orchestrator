# ADR 029 (v2): Execution Leases and Fencing

## Status
Accepted

## Context
Even with a single-active-instance invariant, stale actors, late subprocesses, and crash-recovery races can produce duplicate completions. Without a monotonic ownership token, the Engine cannot prove which execution attempt is allowed to commit.

## Decision
We introduce durable execution leases with monotonic fence tokens.

### 1. Fence Ownership
For every logical `(instance_id, step_id)` pair, the Engine stores the current fence token in the `leases` partition.

Before scheduling a step, the Engine increments or acquires the fence and persists `StepScheduled { fence }`.

### 2. Commit Validation
All completion paths carry the fence:
1. child output,
2. `EffectPrepared`,
3. `EffectCommitted`,
4. `StepCompleted`,
5. `StepFailed`.

The `DbWriterActor` only commits a completion if the fence matches the latest durable lease. Stale fence results are ignored.

### 3. Recovery and Timeouts
If a timeout fires or crash recovery decides to retry a step, the Engine advances the fence before retrying. The old child may still eventually exit, but its completion cannot win.

## Consequences
- **Positive:** Late or duplicated subprocess completions can no longer double-commit the control plane.
- **Positive:** Exact-once state transitions remain valid even when child processes behave badly or finish late.
- **Negative:** Every step now carries more metadata and one more partition must participate in atomic transitions.
