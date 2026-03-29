# ADR 005 (v2): Actor Hibernation and Timer Management

## Status
Accepted

## Context
A durable workflow engine must be able to support workflows that sleep for hours, days, or months. If every sleeping workflow remained in memory as an active `tokio` task or `ractor` actor, the engine would quickly run out of RAM (e.g., 1 million sleeping workflows).

We need a mechanism to suspend the execution of an actor, free its memory, and predictably wake it up when the delay has expired.

## Decision
We implement a **Suspend-to-Disk Hibernation Model** leveraging `ractor` and `fjall`.

### 1. The Suspension Trigger
When an actor reaches a `Wait` node in the DAG, or when an executed binary outputs a `{"_vo_directive": "suspend"}` JSON payload via `stdout` (e.g., waiting for a human approval webhook), the Actor initiates hibernation.

### 2. The Persistence
Before terminating, the actor:
1. Appends a `TimerScheduled { fire_at_timestamp }` event to the `events` partition.
2. Writes a routing entry to the `timers` partition in `fjall`:
   `Key: <fire_at_timestamp>:<instance_id>`
   `Value: TimerData`
3. Calls `context.stop()` on itself. The `ractor` supervisor deregisters it, and the memory is freed by the Rust allocator.

### 3. The Reanimator Loop
The Master Orchestrator spawns a single background `tokio` task on startup. 
Every 1 second, this task performs a prefix/range scan on the `timers` partition from `0` up to `current_timestamp`.
For every key it finds:
1. It deletes the key from the `timers` partition.
2. It spawns a new `ractor` actor for that `instance_id`.
3. The new actor queries the `events` partition, replays its history to restore its exact state, and resumes execution seamlessly.

## Consequences
- **Positive:** Infinite horizontal scaling of sleeping workflows. The engine can track millions of suspended instances using 0 bytes of RAM.
- **Positive:** Crash resilience. If the entire server loses power, the `timers` partition on disk is unaffected. Upon reboot, the Reanimator loop instantly finds any timers that popped while the server was offline and spawns them.
- **Negative:** Rehydrating an actor requires a disk read to replay the event log. (Mitigated by the extreme read speed of LSM-Trees and the use of periodic snapshots for long-running workflows).