# ADR 005 (v2): Actor Hibernation and Timer Management

## Status
Accepted

## Context
A durable workflow engine must be able to support workflows that sleep for hours, days, or months. If every sleeping workflow remained in memory as an active `tokio` task or `ractor` actor, the engine would quickly run out of RAM (e.g., 1 million sleeping workflows).

We need a mechanism to suspend the execution of an actor, free its memory, and predictably wake it up when the delay has expired.

The older design assumed child processes could drive suspension by writing directives to `stdout`. That is no longer acceptable under the hardened IPC contract.

## Decision
We implement a **Suspend-to-Disk Hibernation Model** leveraging `ractor` and `fjall`.

### 1. The Suspension Trigger
Suspension is a first-class workflow transition, not a `stdout` side channel.

An actor initiates hibernation only when:
1. The canonical `WorkflowSpec` reaches a `Wait` node.
2. A Pure Step or Managed Effect Step returns an explicit typed suspension directive over the structured FD4 envelope.
3. Engine policy transitions the instance into `Suspended::WaitingForTimer` or `Suspended::WaitingForSignal` under the hierarchical lifecycle model (ADR-039).

### 2. The Persistence
Before terminating, the actor performs the following sequence atomically where applicable:
1. Appends `TimerScheduled` or `SignalAwaiting` to the `events` partition.
2. Writes a durable wake-up entry to `timers` if the wait is timer-based.
3. Updates the `instances` summary to the suspended state.
4. Persists any snapshot required by the current snapshot policy.
5. Calls `context.stop()` on itself only after the durable suspension boundary commits.

The canonical timer lifecycle is:
```text
TimerScheduled -> TimerPersisted -> ActorStopped -> TimerFired -> ActorResumed
```

`TimerPersisted` may be implicit in the same batch as `TimerScheduled`, but the Engine must reason about that boundary explicitly.

### 3. The Reanimator Loop
The Master Orchestrator spawns a single background `tokio` task on startup.
Every 1 second, this task performs a range scan on the `timers` partition from `0` up to `current_timestamp` using the canonical binary key encoding from ADR-020.
For every key it finds:
1. It atomically records `TimerFired` and deletes the wake-up key.
2. It enqueues resume work for the `instance_id` under the recovery/fairness budget rules.
3. The resumed actor replays from the latest snapshot and transitions to `ActorResumed` / the appropriate active substate.

Timer wake-up matching semantics are further defined in ADR-042.

## Consequences
- **Positive:** Infinite horizontal scaling of sleeping workflows. The engine can track millions of suspended instances using 0 bytes of RAM.
- **Positive:** Crash resilience. If the entire server loses power, the `timers` partition on disk is unaffected. Upon reboot, the Reanimator loop instantly finds any timers that popped while the server was offline and spawns them.
- **Positive:** Suspension is now explicit, replay-safe, and consistent with the hardened FD3/FD4 contract.
- **Negative:** Rehydrating an actor requires a disk read to replay the event log. This is mitigated by snapshots and lineage rollover for long-running workflows.
