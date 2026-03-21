# ADR-019: Snapshot Recovery Optimization (sled as Snapshot Store)

## Status

Accepted

## Context

Full log replay from seq=1 is always correct but becomes slow for long-lived workflows. A workflow that has processed 10,000 events would take measurable time to replay on crash recovery even at memory speeds. For the latency SLO of "recovery in under 5 seconds for instances with fewer than 1,000 events" to hold as instances grow beyond that, we need a mechanism to limit replay to the tail of the log.

sled (ADR-001) was demoted from primary storage to backup when NATS JetStream became the source of truth (ADR-013). It retains value as a **local snapshot store**: it is embedded (no network round-trip), fast for point reads, and gives sled a meaningful role in the architecture without adding a new dependency.

## Decision

The engine takes periodic snapshots of actor in-memory state and stores them in sled. On crash recovery, the engine loads the most recent valid snapshot from sled and replays only the JetStream events that occurred after the snapshot was taken.

### Snapshot Trigger

A snapshot is taken when either:
- The actor has processed `snapshot_interval` new events since the last snapshot (default: 100)
- The actor receives an explicit `TakeSnapshot` message (admin tooling)

### Snapshot Record

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// The JetStream sequence number of the last event applied before this snapshot
    pub seq: u64,
    /// msgpack-encoded actor state (FsmActorState | DagActorState | ProceduralActorState)
    pub state_bytes: Bytes,
    /// CRC32 checksum of state_bytes for corruption detection
    pub checksum: u32,
    /// Wall clock time (informational only, not used during replay)
    pub taken_at: DateTime<Utc>,
}
```

### Snapshot Write Sequence

Taking a snapshot is a two-step operation that preserves consistency:

```
1. Serialize current in-memory state to state_bytes
2. Compute CRC32 checksum
3. Write SnapshotRecord to sled tree "snapshots", key = instance_id
4. Publish WorkflowEvent::SnapshotTaken { seq, checksum } to JetStream
5. Await JetStream PublishAck
```

The `SnapshotTaken` event in JetStream is the marker. During recovery, the engine finds the most recent `SnapshotTaken` event in the log, loads the sled snapshot for that seq, and replays from `seq + 1`.

**Why write to sled before JetStream?** If sled write fails, the `SnapshotTaken` event is never published and the log is unchanged — no harm done. If sled write succeeds but the JetStream publish fails, the snapshot exists in sled but has no marker in the log — it will not be used for recovery, but it does not corrupt anything either. Retrying the publish is safe.

### Recovery Procedure with Snapshots

```
1. Stream JetStream log for instance from the tail backward,
   looking for most recent WorkflowEvent::SnapshotTaken { seq, checksum }

2. If SnapshotTaken found:
   a. Load sled snapshot for instance_id
   b. Verify checksum
   c. If valid: deserialize state, set replay cursor to seq + 1
   d. If invalid: log warning, fall back to full replay from seq=1

3. If no SnapshotTaken found (new instance or all snapshots predated):
   Full replay from seq=1

4. Replay JetStream events from replay cursor to stream tail
5. Enter Live Phase
```

### sled Schema (Amended from ADR-007)

sled retains a single tree:

```rust
const SNAPSHOTS: &[u8] = b"snapshots";

// Key:   instance_id (ULID bytes)
// Value: msgpack-encoded SnapshotRecord
```

All other sled trees from ADR-007 (journal, timers, signals, run_queue, activities, workflows) are replaced by NATS JetStream and KV.

### Performance Characteristics

| Scenario | Replay events | Expected recovery time |
|----------|--------------|----------------------|
| New instance, 0 events | 0 | <1ms |
| Active instance, 100 events, no snapshot | 100 | <10ms |
| Active instance, 1,000 events, snapshot at 900 | 100 | <10ms |
| Active instance, 10,000 events, snapshot at 9,900 | 100 | <10ms |
| Long-lived, snapshot interval 100 | Always ≤100 | <10ms |

### Snapshot Corruption Handling

If checksum validation fails on the loaded snapshot:
- Log `WARN snapshot_corrupted instance_id=<id> seq=<seq>`
- Fall back to full replay from seq=1
- Do not panic: full replay is always correct

After full replay completes, the engine immediately writes a new snapshot, overwriting the corrupted one.

## Consequences

### Positive

- Recovery time is bounded by `snapshot_interval` regardless of workflow age
- sled has a defined, meaningful role in the architecture
- No additional infrastructure: sled is embedded in the engine binary
- Snapshot corruption is survivable (full replay fallback)

### Negative

- Snapshots add write overhead every `snapshot_interval` events
- sled disk space grows proportionally to active workflow count
- Two-step snapshot write (sled then JetStream) is slightly more complex than one-step

### sled Lifecycle

sled entries for a workflow instance are deleted when the instance reaches a terminal state and its event log is archived to the JetStream archive stream. Old snapshots do not accumulate indefinitely.
