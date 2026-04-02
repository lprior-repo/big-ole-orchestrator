# ADR 042 (v2): Signal Matching and Wake-Up Semantics

## Status
Accepted

## Context
Signals, human approvals, continuations, and external callbacks are one of the hardest exact-once surfaces in a durable workflow engine.

Without a precise matching model, the system can accidentally deliver the same signal to multiple epochs, apply a signal to the wrong wait state, or lose intent across `continue-as-new` rollover.

## Decision
Signals follow a deterministic lineage-aware matching contract.

### 1. Matching Dimensions
Every signal is matched against:
1. `workflow_lineage_id`
2. target `instance_id` when explicitly epoch-scoped
3. `wait_key` or `signal_key`
4. current lifecycle state (`Suspended::WaitingForSignal` or equivalent)
5. `command_id` / dedupe key

### 2. Default Scope
Signals are lineage-routed by default.

This means:
1. the signal targets the currently active epoch within the lineage,
2. if the active epoch rolled over via `continue-as-new`, the signal follows the lineage routing map,
3. an explicitly epoch-scoped signal must fail if the targeted epoch is no longer eligible.

### 3. Wait-State Matching
A signal may only resume a workflow if the active epoch is currently waiting on a matching `wait_key`.

If no matching wait is active, policy is explicit per signal node:
1. `Reject` - return a structured mismatch error,
2. `BufferOne` - store exactly one pending signal for the matching key,
3. `BufferMany` - store a bounded queue of pending signals for the matching key.

Unbounded signal buffering is forbidden.

### 4. Dedupe Scope
Signal dedupe is keyed by:
`(workflow_lineage_id, wait_key, command_id)` by default.

Nodes may opt into stricter epoch-scoped dedupe if the business flow requires it.

### 5. Resume Semantics
When a matching signal is accepted, the Engine atomically:
1. records `SignalAccepted`,
2. consumes or clears the matching wait state,
3. updates the instance summary,
4. resumes the actor under the appropriate fairness budget.

## Consequences
- **Positive:** Signals remain deterministic across retries, restarts, and `continue-as-new` rollover.
- **Positive:** Human approvals and callbacks can now participate honestly in exact-once workflows.
- **Negative:** Signal nodes and UI forms must define more explicit matching policy than a naive webhook handler.
