# ADR 027 (v2): Deterministic Replay and Exactly-Once Core Semantics

## Status
Accepted

## Context
When the Engine crashes or restarts, in-flight workflow instances must resume exactly where they left off: no lost work, no duplicated control-plane decisions, and no phantom state.

Veloxide's architecture differs fundamentally from Temporal and Restate. The engine's decision logic is not imperative user code replayed inside an SDK sandbox. The Engine itself is the orchestrator. It traverses a declarative DAG, pinned to a binary hash and canonical `WorkflowSpec`, while child processes remain opaque compute boundaries communicating over FD3/FD4.

The old contract of "at-least-once invocation with cached results" is insufficient for the v2 goal. We need deterministic replay plus an honest exactly-once model for the engine core.

## Decision

### 1. Guarantee Model
Veloxide provides the following guarantees:
1. **Exactly-once admission** for supported external triggers and signals with stable dedupe keys (ADR-028).
2. **Exactly-once control-plane transitions** inside the Engine via atomic batches, single-active-instance invariants, and fencing.
3. **Exactly-once managed effects** for supported connectors (ADR-030).
4. **Exactly-once externally observable semantics** for Pure Steps, even if a deterministic computation is physically recomputed during recovery.
5. **At-least-once only** for explicitly `Unsafe` activities, which are forbidden in exact workflows.

### 2. Replay Strategy: Event-Sourced State Reconstruction
The Engine does **not** re-execute imperative workflow decision code during replay. Instead:
1. Read events for an instance from the `events` partition bounded by the latest snapshot (ADR-016).
2. Apply each event through the pure `apply()` state machine to reconstruct the current `LifecycleState`.
3. Load the canonical `WorkflowSpec` from `workflow_versions` using the pinned binary hash and workflow version reference.
4. Use reconstructed state + canonical workflow topology + recorded routing projections to determine the next legal action.

This is deterministic because `apply()` is pure and the Engine's decision logic must not depend on wall-clock time, random iteration order, or mutable external state.

The lifecycle model itself is hierarchical so that replay cannot enter impossible hybrid states such as "compensating while still owning the old execution fence" (ADR-039).

### 3. Fence-Before-Commit Contract
Before the Engine spawns a child process, it MUST:
1. Acquire or advance the current fence token for `(instance_id, step_id)` (ADR-029).
2. Persist `StepScheduled { step_id, attempt, fence }`.

All child outputs, effect journal updates, and completion paths carry that fence. If a late or duplicated child reports results with a stale fence, the Engine discards them.

### 4. Step Event Sequences
**Pure Step**
```text
StepScheduled -> StepStarted -> StepCompleted | StepFailed
```

**Managed Effect Step**
```text
StepScheduled -> StepStarted -> EffectPrepared -> EffectCommitted -> StepCompleted | StepFailed
```

**Wait / Signal Step**
```text
StepScheduled -> TimerScheduled | SignalAwaiting -> TimerFired | SignalAccepted -> StepCompleted
```

### 5. Event Schema Requirements
The replay contract requires the following durable fields:

#### `WorkflowStarted`
- `binary_hash: String`
- `workflow_version_hash: String`
- `dedupe_key_hash: Option<String>`

#### `StepScheduled`
- `attempt: u32`
- `fence: u64`

#### `StepCompleted`
- `attempt: u32`
- `fence: u64`
- `routing_projection: serde_json::Value`
- `output_ref: Option<String>`
- `output_hash: Option<String>`

`output_ref` may only be published after the referenced canonical blob becomes durable per ADR-040.

#### `EffectPrepared`
- `effect_id: String`
- `sink_kind: String`
- `payload_hash: String`
- `fence: u64`

#### `EffectCommitted`
- `effect_id: String`
- `external_receipt: serde_json::Value`
- `fence: u64`

#### `StepFailed`
- `attempt: u32`
- `fence: u64`

### 6. Determinism Requirements for Engine Code
The following constraints apply to Engine traversal and replay logic:
1. **Deterministic iteration order.** Candidate node selection must use ordered data structures or explicitly sorted vectors.
2. **No wall-clock time in decisions.** Wall-clock time may trigger timers, but it may not decide branch routing or retry choice.
3. **Canonical workflow topology.** Replay uses the stored canonical `WorkflowSpec`, never a fresh `--graph` subprocess during recovery.
4. **Routing uses recorded projections.** Conditional branches evaluate against the recorded `routing_projection`, not against live re-computation.
5. **One logical managed effect per node in v1.** If a workflow needs two independent side effects, it models them as two nodes.
6. **Parallel fan-out ordering is read from the event log.** Replay never assumes an implicit completion order.
7. **Version normalization happens before replay.** Events and snapshots are upcast to the current logical schema before `apply()` runs (ADR-035).
8. **Signal wake-up matching is deterministic.** Signals resume only the lineage/epoch/wait-state defined by ADR-042.

### 7. Replay Path for Crash Recovery
On crash recovery, the Engine follows this sequence:
1. Scan the `instances` partition for non-terminal states.
2. Load the latest snapshot and replay post-snapshot events through `apply()`.
3. Recover the canonical `WorkflowSpec` from `workflow_versions` via the pinned binary hash.
4. Inspect the reconstructed state:
   - `StepScheduled` or `StepStarted` for a Pure Step with no `StepCompleted` -> rerun the child under a new fence. This is safe because the step is pure.
   - `EffectPrepared` with no `EffectCommitted` -> reconcile the sink using `effect_id`. If already committed, persist `EffectCommitted` and `StepCompleted`. If not committed, commit exactly once through the connector.
   - `WaitingForTimer` -> re-register the timer using the recorded timer event.
   - `WaitingForSignal` -> re-register or re-evaluate signal wait state using the deterministic wake-up matching rules in ADR-042.
   - `Compensating::*` -> replay compensation planning/execution state and reconcile any in-flight compensating effect through the same managed connector contract.
   - `RunningDecision` -> re-run deterministic DAG traversal.
5. Throttle recovery per ADR-013 and workload class budgets per ADR-033.

### 8. Exact-Once Contract
The exact-once contract is now explicit:
- Duplicate ingress with the same dedupe key returns the existing instance instead of creating a new one within the configured dedupe retention window.
- A step completion only wins if its fence is current.
- A managed effect only commits through a supported connector with reconciliation semantics.
- Pure Steps may be physically recomputed after a crash, but no duplicate externally visible effect is allowed.
- `Unsafe` nodes are excluded from exact workflows and remain at-least-once.
- Mutating commands carry durable command identity and causation metadata so duplicate operator or API requests do not create ambiguous histories (ADR-036).
- Connector execution and reconciliation semantics are governed by ADR-041.

## Consequences
- **Positive:** Crash recovery remains simple, auditable, and testable: replay events through a pure state machine and reconcile only the managed-effect edge.
- **Positive:** The Engine can now honestly claim exactly-once core semantics instead of hand-waving around idempotency.
- **Positive:** Canonical workflow versions remove the need to re-run discovery during recovery.
- **Negative:** Exact-once is a capability-based contract, not a blanket promise for arbitrary subprocesses.
- **Negative:** Connectors, fencing, and effect journaling add more Engine complexity than plain at-least-once retries.
