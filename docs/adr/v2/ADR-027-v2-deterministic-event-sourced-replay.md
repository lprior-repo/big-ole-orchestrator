# ADR 027 (v2): Deterministic Event-Sourced Replay and Idempotency

## Status
Proposed

## Context
When the engine crashes or restarts, in-flight workflow instances must resume exactly where they left off — no lost work, no duplicated decisions, no phantom state.

Temporal solves this by re-executing imperative workflow code inside an SDK sandbox that intercepts non-deterministic operations (time, random, UUID) and comparing generated Commands against recorded Events. Restate solves this by re-running handler code and short-circuiting at journal entries with previously recorded results (`ctx.run`).

Veloxide's architecture differs fundamentally from both: the engine's decision logic is NOT imperative code running inside an SDK sandbox. The engine IS the orchestrator. It traverses a declarative DAG (discovered via `--graph`, pinned to a binary hash per ADR-017) to decide which nodes to execute next. The subprocess IS the activity — opaque to the engine, communicating only via FD3/FD4 pipes.

This means Temporal/Restate-style replay (re-execute code, intercept non-determinism, compare commands) does not apply. But a simpler, equally correct approach does: **event-sourced state reconstruction through a pure state machine.**

The building blocks already exist in `vo-types`:
- `LifecycleState` enum with exhaustive transition rules (`state.rs`)
- `EventPayload` variants including `StepScheduled`, `StepStarted`, `StepCompleted`, `StepFailed` (`events.rs`)
- Pure `apply(current_state, transition_event) -> Result<LifecycleState, TransitionError>` function (`state.rs`)

This ADR defines the replay contract that binds these pieces together, specifies the event schema changes required, and establishes the idempotency guarantees the engine provides.

## Decision

### 1. Replay Strategy: Event-Sourced State Reconstruction
The engine does NOT re-execute imperative decision code during replay. Instead:
1. Read events for an instance from the `events` partition (bounded by the latest snapshot per ADR-016).
2. Apply each event through the pure `apply()` state machine to reconstruct the instance's current `LifecycleState`.
3. Using the reconstructed state + the DAG topology + recorded step outputs, determine what action the engine must take next.

This is deterministic because `apply()` is a pure function — same event sequence always produces the same state sequence. There is no non-determinism to intercept because the engine's decision logic has no time, random, or external I/O dependencies.

### 2. DAG Topology Persistence
The DAG topology is stored in the `WorkflowStarted` event as a serialized JSON value. During replay, the engine uses the stored topology — it does NOT re-run `--graph` against the pinned binary.

Rationale: Re-running `--graph` spawns a subprocess during recovery, adding latency and a failure mode. The pinned binary's graph cannot change (content-addressed by hash per ADR-012), so storing it once at instance creation is safe and sufficient.

### 3. The Intent-Before-Execution Contract
Before the engine spawns a subprocess, it MUST persist a `StepScheduled` event. This serves two purposes:
- **Replay correctness:** If the engine crashes during subprocess execution, the replay discovers a `StepScheduled` without a corresponding `StepCompleted` or `StepFailed`. The engine knows the step was in-flight and re-executes it.
- **Decision verification:** During replay, the engine re-derives "which node should execute next" from the DAG + completed set. If the engine decides node X but the next event in the log is `StepScheduled { step_id: "Y" }`, this indicates a non-determinism bug or DAG topology change.

The full event sequence for a step is:
```
StepScheduled → StepStarted → StepCompleted | StepFailed
```
Where:
- `StepScheduled` = engine decided to execute (persisted BEFORE subprocess spawn)
- `StepStarted` = subprocess began executing (persisted when FD4 handshake confirms process is alive)
- `StepCompleted` = subprocess returned a result via FD4 (includes step output)
- `StepFailed` = subprocess failed, timed out, or was killed (includes failure reason)

### 4. Deterministic Execution IDs (L2 Idempotency)
Every `StepScheduled` event carries a deterministic `execution_id` and `attempt` number:
```
execution_id = "{instance_id}::{step_id}::{attempt}"
```

The engine passes this `execution_id` to the subprocess as part of the FD3 input payload. The user's code can use this value as an idempotency key for external side effects (Stripe charges, database writes, API calls). On retry, the `attempt` number increments but the `instance_id` and `step_id` remain the same, giving the user a stable key for deduplication.

### 5. Step Output Caching (L3 Idempotency)
The `StepCompleted` event stores the step output (the FD4 payload) in the event log. On crash recovery, the replay path works as follows:

- If the engine finds `StepScheduled` WITH a corresponding `StepCompleted` → the step already completed. Read the cached output from the event. Skip re-execution. Move to `RunningDecision` to determine the next node.
- If the engine finds `StepScheduled` WITHOUT a corresponding `StepCompleted` or `StepFailed` → the step was in-flight at crash time. Re-execute the subprocess (at-least-once).

This narrows the at-least-once window to the milliseconds between reading the FD4 payload and committing the `fjall::Batch` containing the `StepCompleted` event. Once the batch commits, the result is durable and will never be re-executed.

### 6. Event Schema Changes

#### `EventPayload::WorkflowStarted`
Add two fields to the existing `WorkflowStarted` payload:
- `dag_topology: serde_json::Value` — the serialized DAG graph (nodes, edges, conditions) discovered via `--graph`. Stored once at instance creation. Used during replay to reconstruct the DAG without re-running `--graph`.
- `binary_hash: String` — the SHA-256 content hash of the pinned binary (per ADR-012). Enables the UI to display which version a workflow ran on, and enables recovery to verify the binary still exists at `/var/wtf/versions/<hash>/`.

#### `EventPayload::StepScheduled`
Add two fields to the existing `StepScheduled` payload:
- `attempt: u32` — the attempt number for this step (1-indexed). First execution = 1, first retry = 2, etc.
- `execution_id: String` — deterministic identifier `{instance_id}::{step_id}::{attempt}`. Passed to the subprocess via FD3 for use as an idempotency key.

#### `EventPayload::StepCompleted`
Add one field to the existing `StepCompleted` payload:
- `output: serde_json::Value` — the step output returned by the subprocess via FD4. Bounded by `MAX_STEP_OUTPUT_BYTES` (5MB, per ADR-012). Used during replay to skip re-execution and by the UI to display step results.

#### `EventPayload::StepFailed`
Add one field to the existing `StepFailed` payload:
- `attempt: u32` — the attempt number that failed. Enables the UI to render a retry timeline showing which attempt failed and why.

### 7. Determinism Requirements for Engine Code
The following constraints apply to the engine's DAG traversal and routing logic:

1. **Deterministic iteration order.** Candidate node selection must use ordered data structures (`BTreeMap`, `BTreeSet`, or sorted `Vec`). `HashMap` iteration order is non-deterministic across runs. If two nodes are both eligible, the engine must select deterministically (e.g., alphabetical by `step_id`, or by DAG index).

2. **No wall-clock time in decisions.** The engine must not use `Utc::now()` or `Instant::now()` to decide which node to execute, which branch to take, or whether a step should be retried. Timers are the only time-dependent construct, and their fire/not-fire state is recorded in events (`TimerSet`, `TimerFired`).

3. **Step outputs drive routing.** Conditional branching in the DAG (e.g., "route to 'refund' if `output.status == 'failed'`") is evaluated against the step output recorded in the `StepCompleted` event. This is deterministic — the recorded output never changes.

4. **Parallel fan-out ordering.** When multiple independent nodes execute in parallel, the engine must not assume a specific completion order during replay. The event log records the actual order via `StepCompleted` timestamps. The engine reads this order from events, not from re-execution.

### 8. Replay Path for Crash Recovery
On crash recovery (ADR-013), the engine follows this sequence:

1. **Scan the `instances` partition** for instances in non-terminal states (`Running`, `Hibernated`). Skip `Completed`, `Failed`, `Cancelled` — these are cold data.

2. **For each recovered instance**, load the latest snapshot from the `snapshots` partition (ADR-016). Replay events with `sequence_number > snapshot.sequence_number` through `apply()`.

3. **Inspect the reconstructed state:**
   - `StepScheduled` with no `StepCompleted`/`StepFailed` → step was in-flight at crash time. Re-execute the subprocess (at-least-once). The user's binary must be idempotent.
   - `WaitingForTimer` → instance was hibernated. Re-register the timer with the Reanimator using the `fire_at` from the `TimerSet` event. Do NOT re-evaluate against wall-clock time.
   - `RunningDecision` → engine was mid-decision. Re-run the DAG traversal (pure function of DAG + completed set) and proceed.
   - `StepExecuting` with no `StepCompleted`/`StepFailed` → same as `StepScheduled` re-execution case.

4. **Throttle recovery** per ADR-013 batching (e.g., 50 instances at a time with inter-batch delay).

### 9. Idempotency Contract
The engine provides **at-least-once invocation with effectively-once result caching**:
- A step may be invoked more than once if the engine crashes between subprocess completion and `StepCompleted` persistence.
- If `StepCompleted` is in the event log, the engine reads the cached output and skips re-execution. The subprocess is NOT spawned again.
- If `StepCompleted` is absent, the engine re-spawns the subprocess regardless of whether the previous invocation completed externally.

The user's subprocess code MUST be idempotent for any operation with external side effects (payment charges, email sends, database writes). The `execution_id` provided via FD3 gives the user a free idempotency key for external systems that support deduplication (Stripe, most databases, most APIs).

This differs from Temporal (which provides effectively-once semantics via deterministic replay + activity result caching) and Restate (which provides exactly-once journaling semantics). Veloxide trades these stronger guarantees for the simplicity and performance of subprocess execution without SDK sandboxing.

## Consequences
- **Positive:** Crash recovery is simple, verifiable, and testable — replay a known event sequence through a pure function and check the result. No SDK interception, no command comparison, no non-determinism detection.
- **Positive:** The engine's decision logic is trivially auditable. Every decision the engine makes is recorded in the event log. The event log IS the execution trace.
- **Positive:** Step output caching means completed steps are never re-executed on recovery. The at-least-once window is bounded to the milliseconds between FD4 read and batch commit.
- **Positive:** Deterministic `execution_id` gives users a free idempotency key with zero SDK effort for external systems that support deduplication.
- **Positive:** DAG topology stored in `WorkflowStarted` eliminates `--graph` subprocess spawns during recovery, making crash recovery faster and more reliable.
- **Positive:** `attempt` fields on `StepScheduled` and `StepFailed` enable the UI to render a rich retry timeline — users can see exactly which attempt failed, why, and how many retries occurred.
- **Positive:** `output` field on `StepCompleted` enables the UI to display step results inline — users can inspect what each step returned without re-running anything.
- **Negative:** User code must be idempotent for external side effects. This is a real burden on the developer, though the `execution_id` key significantly reduces the effort.
- **Negative:** Event payloads are larger due to step output caching and DAG topology storage. Mitigated by `MAX_STEP_OUTPUT_BYTES` (5MB cap) and the fact that DAG topologies are typically small (< 10KB for most workflows).
- **Negative:** DAG topology changes between executions are not tolerated. ADR-017 version pinning mitigates this — active instances are pinned to their original binary hash.
- **Negative:** The engine must be audited for `HashMap` usage in traversal code and `Utc::now()` usage in decision paths. This is an ongoing code review constraint.
