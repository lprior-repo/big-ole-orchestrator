# ADR 002 (v2): Storage Pivot to Fjall (LSM-Tree)

## Status
Accepted

## Context
v1 used NATS JetStream as the durable event log. To achieve a true "Single Binary" architecture without sacrificing event-sourced write throughput, we must embed the storage engine directly into the `vo-engine` process.

For v2, storage also carries more than just events. Exactly-once admission, execution fencing, effect journaling, snapshots, and workflow versioning all need durable atomic state.

## Decision
We will use **`fjall`**, a pure-Rust Log-Structured Merge-tree (LSM-tree), as the engine's durable storage substrate.

### The KV Schema Partitions
Because `fjall` is a key-value store, we isolate data into explicit partitions:
1. **`events`**: Minimal replay events and state transitions.
2. **`instances`**: Materialized instance summaries for dashboard and operator queries.
3. **`timers`**: Durable wake-up schedule for hibernated workflows.
4. **`snapshots`**: Periodic replay acceleration checkpoints.
5. **`dedupe`**: Exactly-once ingress and signal deduplication records.
6. **`effects`**: `EffectPrepared` and `EffectCommitted` journal entries for managed effects.
7. **`leases`**: Monotonic fence tokens for step execution ownership.
8. **`workflow_versions`**: Canonical `WorkflowSpec` and metadata keyed by pinned binary hash.
9. **`payload_blobs`**: Encrypted canonical payload blobs, optional large outputs, and bounded observability artifacts that do not belong on the hot control-plane path.

### Hot Path vs Cold Path
The storage model is intentionally split:
1. **Hot control-plane partitions** hold small, bounded values required for exact-once execution and replay.
2. **Cold blob storage** holds large canonical payloads and logs by reference and hash.

Replay and scheduling must never depend on scanning huge blob values on the hot path.

### High-Throughput Batching (`DbWriterActor`)
To maximize NVMe IOPS, individual `ractor` actors will **not** call `fsync` directly. All actors send transition requests to a central `DbWriterActor`, which:
1. Uses `fjall::Batch` for every control-plane transition.
2. Group-commits small hot records frequently.
3. Treats large blob persistence and operator projections as lower-priority work than exact-once control records.

## Consequences
- **Positive:** High write throughput remains possible in a pure Rust, single-binary deployment.
- **Positive:** Fjall can now serve as the durable backbone for admission dedupe, replay, effect journaling, and workflow versioning.
- **Positive:** The hot/cold split prevents large payloads and logs from dominating the exact-once write path.
- **Negative:** We lose SQL queryability. The UI and CLI rely on custom indexes and projections.
- **Negative:** Schema migrations require careful `serde` versioning and partition-aware migrations.
- **Negative:** Blob garbage collection, compaction pressure, and storage QoS become first-class operational concerns.
