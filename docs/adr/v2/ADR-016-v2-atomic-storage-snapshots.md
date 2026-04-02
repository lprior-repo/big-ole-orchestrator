# ADR 016 (v2): Atomic Storage and Replay Snapshots

## Status
Accepted

## Context
1. **Multi-Partition Corruption:** We are using `fjall` with multiple partitions (`events`, `instances`, `timers`, `dedupe`, `effects`, `leases`, `snapshots`). If the Engine writes an event but crashes before updating the related control-plane records, exact-once semantics collapse.
2. **The Replay Cliff:** An actor rehydrates its state by replaying its event log. If a workflow runs a massive Map loop, generating 20,000 events, rehydration could take seconds. If the Engine restarts and 100 actors need to rehydrate 20,000 events each, startup delay is catastrophic.
3. **Blob Publication Hazard:** If the Engine publishes a `StepCompleted.output_ref` before the referenced canonical blob is durable, replay can observe a pointer to missing truth.

## Decision

### 1. Atomic WriteBatches
The `DbWriterActor` is mandated to use `fjall::Batch` for every single control-plane transition.
- A transition must atomically update **every** touched control-plane partition in the same batch: `events`, `instances`, `timers`, `dedupe`, `effects`, `leases`, and `snapshots` as applicable.
- If the batch fails to commit, none of those writes become visible.
- Observability projections and cold blob writes may be deferred, but exact-once control records may not.

Canonical payload blob publication is governed by ADR-040. A control-plane record may only publish a blob reference once the blob has crossed its durability boundary.

### 2. Periodic State Snapshotting
To solve the replay cliff, the Engine implements a `snapshots` partition in `fjall`.
- Every $N$ events, the actor serializes its computed in-memory state, including the current routing state, outstanding timer state, current fence ownership, and in-flight managed-effect bookkeeping.
- It sends a `SnapshotTaken { sequence_number, state_bytes }` instruction to the `DbWriterActor`.
- The snapshot is written as part of the same atomic batch as the event that triggered it.
- On rehydration, the actor reads the latest snapshot and replays only events with `sequence_number > snapshot.sequence_number`.

Snapshots are an optimization, not a new source of truth. They carry their own schema version and may be discarded and rebuilt if incompatible with the current upcaster chain (ADR-035).

## Consequences
- **Positive:** Exact-once control-plane consistency is preserved across crashes.
- **Positive:** Rehydration time is strictly bounded to $O(N)$ post-snapshot events.
- **Negative:** Snapshots consume additional disk space and must be kept small enough to avoid hurting the hot path.
